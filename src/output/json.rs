use crate::error::AppError;
use serde::Serialize;

pub fn render(result: &impl Serialize, pretty: bool) -> Result<String, AppError> {
    if pretty {
        serde_json::to_string_pretty(result)
    } else {
        serde_json::to_string(result)
    }
    .map_err(|error| AppError::local(format!("could not serialize result: {error}")))
}
