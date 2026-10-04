use std::fmt;

#[derive(Debug)]
pub struct AppError {
    pub message: String,
    pub code: u8,
}

impl AppError {
    pub fn new(message: impl Into<String>, code: u8) -> Self {
        Self {
            message: message.into(),
            code,
        }
    }

    pub fn local(message: impl Into<String>) -> Self {
        Self::new(message, 1)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for AppError {}
