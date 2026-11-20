//! Sonda de ejecución de binarios externos (Spec S-32).

use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

pub const TIEMPO_SONDA_DEFECTO: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultadoSonda {
    pub ok: bool,
    pub salida: String,
    pub error: String,
}

/// Ejecuta `binario` con `args` y tiempo límite sin lanzar pánico.
pub async fn sondear(binario: &str, args: &[&str], timeout: Duration) -> ResultadoSonda {
    let mut cmd = Command::new(binario);
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());

    #[cfg(windows)]
    {
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let error = if e.kind() == std::io::ErrorKind::NotFound {
                "not found".to_string()
            } else {
                e.to_string()
            };
            return ResultadoSonda {
                ok: false,
                salida: String::new(),
                error,
            };
        }
    };

    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let mut out_bytes = Vec::new();
    let mut err_bytes = Vec::new();

    let lectura = async {
        let l_out = async {
            if let Some(mut so) = stdout.take() {
                let _ = so.read_to_end(&mut out_bytes).await;
            }
        };
        let l_err = async {
            if let Some(mut se) = stderr.take() {
                let _ = se.read_to_end(&mut err_bytes).await;
            }
        };
        tokio::join!(l_out, l_err);
        child.wait().await
    };

    match tokio::time::timeout(timeout, lectura).await {
        Ok(Ok(status)) => {
            let out_str = String::from_utf8_lossy(&out_bytes).trim().to_string();
            let err_str = String::from_utf8_lossy(&err_bytes).trim().to_string();

            if status.success() {
                ResultadoSonda {
                    ok: true,
                    salida: out_str,
                    error: String::new(),
                }
            } else {
                let code = status.code().unwrap_or(-1);
                let err_source = if !err_str.is_empty() {
                    err_str
                } else if !out_str.is_empty() {
                    out_str
                } else {
                    format!("exit code {}", code)
                };
                let last_line = err_source
                    .lines()
                    .last()
                    .unwrap_or(&err_source)
                    .trim()
                    .to_string();

                ResultadoSonda {
                    ok: false,
                    salida: String::new(),
                    error: last_line,
                }
            }
        }
        Ok(Err(e)) => ResultadoSonda {
            ok: false,
            salida: String::new(),
            error: e.to_string(),
        },
        Err(_) => {
            let _ = child.kill().await;
            ResultadoSonda {
                ok: false,
                salida: String::new(),
                error: "timed out".to_string(),
            }
        }
    }
}
