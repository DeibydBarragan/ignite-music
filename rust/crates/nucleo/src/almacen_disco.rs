use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaveInvalidaError(pub String);

impl fmt::Display for ClaveInvalidaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid store key: {}", self.0)
    }
}

impl std::error::Error for ClaveInvalidaError {}

#[derive(Debug)]
pub enum AlmacenError {
    ClaveInvalida(ClaveInvalidaError),
    Escritura(std::io::Error),
}

impl fmt::Display for AlmacenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ClaveInvalida(e) => write!(f, "{e}"),
            Self::Escritura(e) => write!(f, "Escritura error: {e}"),
        }
    }
}

impl std::error::Error for AlmacenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ClaveInvalida(e) => Some(e),
            Self::Escritura(e) => Some(e),
        }
    }
}

impl From<ClaveInvalidaError> for AlmacenError {
    fn from(err: ClaveInvalidaError) -> Self {
        Self::ClaveInvalida(err)
    }
}

impl From<std::io::Error> for AlmacenError {
    fn from(err: std::io::Error) -> Self {
        Self::Escritura(err)
    }
}

pub fn validate_key(key: &str) -> Result<(), ClaveInvalidaError> {
    if key.is_empty() || key.len() > 64 {
        return Err(ClaveInvalidaError(key.to_string()));
    }
    if !key
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(ClaveInvalidaError(key.to_string()));
    }
    Ok(())
}

pub fn write_atomic_file(
    dir: &Path,
    file_path: &Path,
    json_bytes: &[u8],
) -> Result<(), std::io::Error> {
    use std::io::Write;
    let pid = std::process::id();
    let rnd: u32 = rand::random();
    let tmp_name = format!(
        "{}.{}.{:08x}.tmp",
        file_path.file_name().unwrap().to_string_lossy(),
        pid,
        rnd
    );
    let tmp_path = dir.join(tmp_name);

    let res = (|| {
        let mut file = std::fs::File::create(&tmp_path)?;
        file.write_all(json_bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp_path, file_path)?;
        Ok(())
    })();

    if res.is_err() {
        let _ = std::fs::remove_file(&tmp_path);
    }
    res
}
