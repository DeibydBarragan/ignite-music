use medios::{Limitador, Permiso, Prioridad};
use songbird::input::{AudioStream, AudioStreamError, Compose};
use std::io::{self, Read, Seek, SeekFrom};
use std::process::{Child, Command, Stdio};
use symphonia::core::io::{MediaSource, ReadOnlySource};

struct ProcesoAudio(Vec<Child>, Option<Permiso>);

impl Read for ProcesoAudio {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0
            .last_mut()
            .and_then(|child| child.stdout.as_mut())
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "ffmpeg stdout closed"))?
            .read(buf)
    }
}

impl Seek for ProcesoAudio {
    fn seek(&mut self, _: SeekFrom) -> io::Result<u64> {
        Err(io::ErrorKind::Unsupported.into())
    }
}

impl MediaSource for ProcesoAudio {
    fn is_seekable(&self) -> bool {
        false
    }

    fn byte_len(&self) -> Option<u64> {
        None
    }
}

impl Drop for ProcesoAudio {
    fn drop(&mut self) {
        let mut children = std::mem::take(&mut self.0);
        let permiso = self.1.take();
        if children.is_empty() {
            drop(permiso);
            return;
        }
        let cleanup = move || {
            for child in &mut children {
                let _ = child.kill();
            }
            for child in children.iter_mut().rev() {
                let _ = child.wait();
            }
            drop(permiso);
        };
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn_blocking(cleanup);
        } else {
            cleanup();
        }
    }
}

pub(crate) struct PipelineAudio {
    pub ytdlp: String,
    pub ytdlp_args: Vec<String>,
    pub ffmpeg: String,
    pub query: String,
    pub filtro: Option<String>,
    pub seek: Option<u64>,
    pub limiter: Option<Limitador>,
    pub limiter_timeout_ms: u64,
}

impl PipelineAudio {
    fn crear(
        &self,
        permiso: Option<Permiso>,
    ) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        let mut extractor = Command::new(&self.ytdlp);
        extractor
            .args(&self.ytdlp_args)
            .args([
                "--output",
                "-",
                "--format",
                "bestaudio/best",
                "--no-playlist",
            ])
            .arg(&self.query)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut ytdlp = extractor
            .spawn()
            .map_err(|e| AudioStreamError::Fail(Box::new(e)))?;
        let input = ytdlp
            .stdout
            .take()
            .ok_or_else(|| AudioStreamError::Fail("yt-dlp stdout unavailable".into()))?;
        let mut ffmpeg = Command::new(&self.ffmpeg);
        ffmpeg
            .args(crate::filtros_ffmpeg::ffmpeg_pipe_args(
                self.filtro.as_deref(),
                self.seek,
            ))
            .stdin(Stdio::from(input))
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        match ffmpeg.spawn() {
            Ok(mut ffmpeg) => {
                if ffmpeg.stdout.is_none() {
                    let _ = ffmpeg.kill();
                    let _ = ytdlp.kill();
                    return Err(AudioStreamError::Fail("ffmpeg stdout unavailable".into()));
                }
                let source = ProcesoAudio(vec![ytdlp, ffmpeg], permiso);
                Ok(AudioStream {
                    input: Box::new(ReadOnlySource::new(source)),
                })
            }
            Err(error) => {
                let _ = ytdlp.kill();
                let _ = ytdlp.wait();
                Err(AudioStreamError::Fail(Box::new(error)))
            }
        }
    }
}

#[async_trait::async_trait]
impl Compose for PipelineAudio {
    fn create(&mut self) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        if self.limiter.is_some() {
            return Err(AudioStreamError::Fail(
                "playback pipeline requires async initialization".into(),
            ));
        }
        self.crear(None)
    }

    async fn create_async(
        &mut self,
    ) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        let permiso = if let Some(limiter) = &self.limiter {
            Some(
                limiter
                    .acquire(
                        Prioridad::Baja,
                        Some(std::time::Duration::from_millis(
                            self.limiter_timeout_ms.max(1),
                        )),
                    )
                    .await
                    .map_err(|error| AudioStreamError::Fail(Box::new(error)))?,
            )
        } else {
            None
        };
        let pipeline = Self {
            ytdlp: self.ytdlp.clone(),
            ytdlp_args: self.ytdlp_args.clone(),
            ffmpeg: self.ffmpeg.clone(),
            query: self.query.clone(),
            filtro: self.filtro.clone(),
            seek: self.seek,
            limiter: None,
            limiter_timeout_ms: self.limiter_timeout_ms,
        };
        tokio::task::spawn_blocking(move || pipeline.crear(permiso))
            .await
            .map_err(|e| AudioStreamError::Fail(Box::new(e)))?
    }

    fn should_create_async(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::ProcesoAudio;
    use medios::{Limitador, Prioridad};
    use std::time::Duration;

    #[tokio::test]
    async fn el_permiso_cubre_la_vida_de_la_fuente_de_audio() {
        let limiter = Limitador::new(1);
        let permiso = limiter.acquire(Prioridad::Baja, None).await.unwrap();
        let fuente = ProcesoAudio(Vec::new(), Some(permiso));
        assert_eq!(limiter.active(), 1);
        assert!(limiter
            .acquire(Prioridad::Alta, Some(Duration::from_millis(5)))
            .await
            .is_err());
        drop(fuente);
        assert_eq!(limiter.active(), 0);
        assert!(limiter
            .acquire(Prioridad::Alta, Some(Duration::from_millis(5)))
            .await
            .is_ok());
    }
}
