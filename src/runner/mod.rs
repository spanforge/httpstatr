mod curl;

use std::path::PathBuf;

use crate::error::AppError;
use crate::model::{Metrics, RedirectHop, ResponseHeaders};

pub use curl::{CurlRunner, validate_installation, validate_installation_bounded};

pub struct Request<'a> {
    pub url: &'a str,
    pub curl_args: &'a [String],
    pub curl_bin: &'a str,
    pub connect_timeout: Option<f64>,
    pub timeout: Option<f64>,
    pub canceled: &'a std::sync::atomic::AtomicBool,
    pub deadline: Option<std::time::Instant>,
    pub max_download_bytes: u64,
    pub debug: bool,
    pub show_secrets: bool,
}

pub struct RunOutput {
    pub metrics: Metrics,
    pub headers: ResponseHeaders,
    pub body_path: PathBuf,
    pub redirects: Vec<RedirectHop>,
}

pub trait Runner {
    fn execute(&self, request: &Request<'_>) -> Result<RunOutput, AppError>;
}
