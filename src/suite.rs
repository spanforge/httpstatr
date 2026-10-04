use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cli::{Config, MAX_REPEAT, MAX_WARMUPS, OutputFormat};
use crate::error::AppError;
use crate::model::BatchResult;

pub const SUITE_SCHEMA_VERSION: u8 = 1;
pub const SUITE_FAILURE_EXIT_CODE: u8 = 8;
pub const MAX_CONCURRENCY: usize = 64;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RequestSettings {
    pub repeat: Option<usize>,
    pub warmup: Option<usize>,
    pub delay: Option<f64>,
    pub connect_timeout: Option<f64>,
    pub timeout: Option<f64>,
    pub slo: Option<String>,
    pub expect_status: Vec<String>,
    pub expect_header: Vec<String>,
    pub min_body_bytes: Option<u64>,
    pub max_body_bytes: Option<u64>,
    pub expect_body_contains: Vec<String>,
    pub expect_body_regex: Vec<String>,
    pub compare: Option<PathBuf>,
    pub fail_if: Vec<String>,
    pub curl_args: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndpointDefinition {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub headers: Vec<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(flatten)]
    pub settings: RequestSettings,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SuitePolicies {
    pub min_success_rate: Option<f64>,
    pub max_failures: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuiteDefinition {
    pub schema_version: u8,
    #[serde(default)]
    pub concurrency: Option<usize>,
    #[serde(default)]
    pub defaults: RequestSettings,
    #[serde(default)]
    pub policies: SuitePolicies,
    pub requests: Vec<EndpointDefinition>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SuiteResult {
    pub schema_version: u8,
    pub ok: bool,
    pub exit_code: u8,
    pub source: String,
    pub concurrency: usize,
    pub canceled: bool,
    pub summary: SuiteSummary,
    pub policies: SuitePolicyResult,
    pub endpoints: Vec<SuiteEndpointResult>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SuiteSummary {
    pub total_endpoints: usize,
    pub passed: usize,
    pub failed: usize,
    pub success_rate: f64,
    pub total_samples: usize,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SuitePolicyResult {
    pub pass: bool,
    pub min_success_rate: f64,
    pub max_failures: usize,
    pub violations: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SuiteEndpointResult {
    pub index: usize,
    pub name: String,
    pub url: String,
    pub ok: bool,
    pub exit_code: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<BatchResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn load(path: &Path) -> Result<SuiteDefinition, AppError> {
    let text = fs::read_to_string(path).map_err(|error| {
        AppError::local(format!("could not read suite {}: {error}", path.display()))
    })?;
    let suite: SuiteDefinition = toml::from_str(&text)
        .map_err(|error| AppError::local(format!("invalid suite {}: {error}", path.display())))?;
    validate(&suite)?;
    Ok(suite)
}

pub fn endpoint_config(
    base: &Config,
    suite_path: &Path,
    defaults: &RequestSettings,
    endpoint: &EndpointDefinition,
) -> Config {
    let settings = &endpoint.settings;
    let mut curl_args = defaults.curl_args.clone();
    curl_args.extend(settings.curl_args.clone());
    if let Some(method) = &endpoint.method {
        curl_args.extend(["-X".to_string(), method.clone()]);
    }
    for header in &endpoint.headers {
        curl_args.extend(["-H".to_string(), header.clone()]);
    }
    if let Some(body) = &endpoint.body {
        curl_args.extend(["--data-binary".to_string(), body.clone()]);
    }
    let compare = choose(&settings.compare, &defaults.compare)
        .cloned()
        .map(|path| resolve_relative(suite_path, path));
    Config {
        url: endpoint.url.clone(),
        curl_args,
        format: OutputFormat::Json,
        slo: choose(&settings.slo, &defaults.slo).cloned(),
        save: None,
        history: None,
        history_name: None,
        history_tags: Vec::new(),
        commit: None,
        show_body: false,
        show_ip: false,
        show_speed: false,
        save_body: false,
        curl_bin: base.curl_bin.clone(),
        curl_version: base.curl_version.clone(),
        debug: base.debug,
        connect_timeout: choose(&settings.connect_timeout, &defaults.connect_timeout).copied(),
        timeout: choose(&settings.timeout, &defaults.timeout).copied(),
        expect_status: merge(&defaults.expect_status, &settings.expect_status),
        expect_header: merge(&defaults.expect_header, &settings.expect_header),
        show_secrets: base.show_secrets,
        repeat: settings.repeat.or(defaults.repeat).unwrap_or(1),
        warmup: settings.warmup.or(defaults.warmup).unwrap_or(0),
        delay: settings.delay.or(defaults.delay).unwrap_or(0.0),
        min_body_bytes: choose(&settings.min_body_bytes, &defaults.min_body_bytes).copied(),
        max_body_bytes: choose(&settings.max_body_bytes, &defaults.max_body_bytes).copied(),
        expect_body_contains: merge(
            &defaults.expect_body_contains,
            &settings.expect_body_contains,
        ),
        expect_body_regex: merge(&defaults.expect_body_regex, &settings.expect_body_regex),
        compare,
        fail_if: merge(&defaults.fail_if, &settings.fail_if),
        suite_file: None,
        concurrency: None,
        suite_min_success_rate: None,
        suite_max_failures: None,
    }
}

pub fn effective_concurrency(cli: Option<usize>, suite: Option<usize>) -> Result<usize, AppError> {
    let value = cli.or(suite).unwrap_or(4);
    if (1..=MAX_CONCURRENCY).contains(&value) {
        Ok(value)
    } else {
        Err(AppError::local(format!(
            "concurrency must be between 1 and {MAX_CONCURRENCY}, got {value}"
        )))
    }
}

pub fn evaluate_policies(
    policies: &SuitePolicies,
    cli_min_success_rate: Option<f64>,
    cli_max_failures: Option<usize>,
    passed: usize,
    total: usize,
) -> SuitePolicyResult {
    let min_success_rate = cli_min_success_rate
        .or(policies.min_success_rate)
        .unwrap_or(100.0);
    let max_failures = cli_max_failures.or(policies.max_failures).unwrap_or(0);
    let failed = total.saturating_sub(passed);
    let success_rate = if total == 0 {
        0.0
    } else {
        passed as f64 / total as f64 * 100.0
    };
    let mut violations = Vec::new();
    if success_rate < min_success_rate {
        violations.push(format!(
            "endpoint success rate {:.2}% is below {:.2}%",
            success_rate, min_success_rate
        ));
    }
    if failed > max_failures {
        violations.push(format!(
            "failed endpoint count {failed} exceeds {max_failures}"
        ));
    }
    SuitePolicyResult {
        pass: violations.is_empty(),
        min_success_rate,
        max_failures,
        violations,
    }
}

fn validate(suite: &SuiteDefinition) -> Result<(), AppError> {
    if suite.schema_version != SUITE_SCHEMA_VERSION {
        return Err(AppError::local(format!(
            "unsupported suite schema version {}; expected {}",
            suite.schema_version, SUITE_SCHEMA_VERSION
        )));
    }
    if suite.requests.is_empty() {
        return Err(AppError::local(
            "suite must contain at least one [[requests]] entry",
        ));
    }
    validate_settings("defaults", &suite.defaults)?;
    if let Some(value) = suite.concurrency {
        effective_concurrency(None, Some(value))?;
    }
    if suite
        .policies
        .min_success_rate
        .is_some_and(|value| !(0.0..=100.0).contains(&value))
    {
        return Err(AppError::local(
            "policies.min_success_rate must be between 0 and 100",
        ));
    }
    let mut names = HashSet::new();
    for request in &suite.requests {
        if request.name.trim().is_empty() {
            return Err(AppError::local("suite request name cannot be empty"));
        }
        if request.url.trim().is_empty() {
            return Err(AppError::local(format!(
                "suite request {:?} has an empty URL",
                request.name
            )));
        }
        if !names.insert(request.name.clone()) {
            return Err(AppError::local(format!(
                "duplicate suite request name {:?}",
                request.name
            )));
        }
        validate_settings(&format!("request {:?}", request.name), &request.settings)?;
    }
    Ok(())
}

fn validate_settings(label: &str, settings: &RequestSettings) -> Result<(), AppError> {
    if settings
        .repeat
        .is_some_and(|value| !(1..=MAX_REPEAT).contains(&value))
    {
        return Err(AppError::local(format!(
            "{label} repeat must be between 1 and {MAX_REPEAT}"
        )));
    }
    if settings.warmup.is_some_and(|value| value > MAX_WARMUPS) {
        return Err(AppError::local(format!(
            "{label} warmup must not exceed {MAX_WARMUPS}"
        )));
    }
    if settings
        .delay
        .is_some_and(|value| !value.is_finite() || value < 0.0)
    {
        return Err(AppError::local(format!(
            "{label} delay must be a nonnegative number"
        )));
    }
    for (name, value) in [
        ("connect_timeout", settings.connect_timeout),
        ("timeout", settings.timeout),
    ] {
        if value.is_some_and(|value| !value.is_finite() || value <= 0.0) {
            return Err(AppError::local(format!(
                "{label} {name} must be a positive number"
            )));
        }
    }
    if settings
        .min_body_bytes
        .zip(settings.max_body_bytes)
        .is_some_and(|(minimum, maximum)| minimum > maximum)
    {
        return Err(AppError::local(format!(
            "{label} min_body_bytes cannot exceed max_body_bytes"
        )));
    }
    Ok(())
}

fn choose<'a, T>(endpoint: &'a Option<T>, default: &'a Option<T>) -> Option<&'a T> {
    endpoint.as_ref().or(default.as_ref())
}

fn merge<T: Clone>(default: &[T], endpoint: &[T]) -> Vec<T> {
    default.iter().chain(endpoint).cloned().collect()
}

fn resolve_relative(suite_path: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        suite_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versioned_suite() {
        let suite: SuiteDefinition = toml::from_str(
            r#"
schema_version = 1
concurrency = 2

[defaults]
repeat = 3
expect_status = ["200-299"]

[[requests]]
name = "health"
url = "https://example.test/health"
method = "GET"
"#,
        )
        .unwrap();
        validate(&suite).unwrap();
        assert_eq!(suite.requests.len(), 1);
        assert_eq!(suite.defaults.repeat, Some(3));
    }

    #[test]
    fn policies_can_allow_some_endpoint_failures() {
        let result = evaluate_policies(
            &SuitePolicies {
                min_success_rate: Some(75.0),
                max_failures: Some(1),
            },
            None,
            None,
            3,
            4,
        );
        assert!(result.pass);
    }

    #[test]
    fn documented_example_is_valid() {
        let suite: SuiteDefinition =
            toml::from_str(include_str!("../examples/httpstatr-suite.toml")).unwrap();
        validate(&suite).unwrap();
    }
}
