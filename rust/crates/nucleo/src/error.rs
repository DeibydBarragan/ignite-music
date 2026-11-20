use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArranqueError {
    pub problemas: usize,
}

impl ArranqueError {
    pub fn new(problemas: usize) -> Self {
        Self { problemas }
    }
}

impl fmt::Display for ArranqueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "The bot was not started: {} problem(s) above must be fixed first.",
            self.problemas
        )
    }
}

impl std::error::Error for ArranqueError {}
