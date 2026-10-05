use crate::error::AppError;
use crate::model::{AssertionFailure, AssertionSummary, Metrics, ResponseHeaders, Violation};
use regex::Regex;

pub const ASSERTION_EXIT_CODE: u8 = 5;

pub fn parse_slo(spec: &str) -> Result<Vec<(String, u64)>, AppError> {
    let mut values = Vec::new();
    for part in spec.split(',') {
        let (key, value) = part.trim().split_once('=').ok_or_else(|| {
            AppError::local(format!(
                "invalid SLO specification {part:?}; expected key=value"
            ))
        })?;
        let key = key.trim();
        if !matches!(key, "total" | "connect" | "ttfb" | "dns" | "tls") {
            return Err(AppError::local(format!(
                "unknown SLO key {key:?}; valid keys: total, connect, ttfb, dns, tls"
            )));
        }
        let threshold = value.trim().parse::<u64>().map_err(|_| {
            AppError::local(format!("SLO value for {key:?} must be a positive integer"))
        })?;
        if threshold == 0 {
            return Err(AppError::local(format!(
                "SLO value for {key:?} must be positive"
            )));
        }
        values.push((key.to_string(), threshold));
    }
    if values.is_empty() {
        return Err(AppError::local("empty SLO specification"));
    }
    Ok(values)
}

pub fn check_slo(slo: Option<&[(String, u64)]>, metrics: &Metrics) -> Vec<Violation> {
    let mut violations = Vec::new();
    for (key, threshold) in slo.unwrap_or(&[]) {
        let actual = match key.as_str() {
            "total" => metrics.time_total,
            "connect" => metrics.time_connect,
            "ttfb" => metrics.time_starttransfer,
            "dns" => metrics.time_namelookup,
            "tls" => metrics.time_pretransfer,
            _ => continue,
        };
        if actual > *threshold {
            violations.push(Violation {
                key: key.clone(),
                threshold_ms: *threshold,
                actual_ms: actual,
            });
        }
    }
    violations
}

#[derive(Debug)]
pub struct Expectations {
    statuses: Vec<StatusExpectation>,
    headers: Vec<HeaderExpectation>,
    min_body_bytes: Option<u64>,
    max_body_bytes: Option<u64>,
    body_contains: Vec<String>,
    body_regexes: Vec<(String, Regex)>,
    json: Vec<(String, JsonExpectation)>,
}

#[derive(Debug)]
enum JsonExpectation {
    Equal(serde_json::Value),
    Type(String),
}

#[derive(Debug)]
struct StatusExpectation {
    source: String,
    start: u16,
    end: u16,
}

#[derive(Debug)]
struct HeaderExpectation {
    name: String,
    value: Option<String>,
}

impl Expectations {
    pub fn parse(
        statuses: &[String],
        headers: &[String],
        min_body_bytes: Option<u64>,
        max_body_bytes: Option<u64>,
        body_contains: &[String],
        body_regexes: &[String],
    ) -> Result<Option<Self>, AppError> {
        if min_body_bytes
            .zip(max_body_bytes)
            .is_some_and(|(minimum, maximum)| minimum > maximum)
        {
            return Err(AppError::local(
                "--min-body-bytes cannot exceed --max-body-bytes",
            ));
        }
        if statuses.is_empty()
            && headers.is_empty()
            && min_body_bytes.is_none()
            && max_body_bytes.is_none()
            && body_contains.is_empty()
            && body_regexes.is_empty()
        {
            return Ok(None);
        }
        let mut parsed_statuses = Vec::new();
        for source in statuses.iter().flat_map(|value| value.split(',')) {
            let source = source.trim();
            let (start, end) = if let Some((start, end)) = source.split_once('-') {
                (parse_status(start)?, parse_status(end)?)
            } else {
                let status = parse_status(source)?;
                (status, status)
            };
            if start > end {
                return Err(AppError::local(format!(
                    "invalid status range {source:?}: start exceeds end"
                )));
            }
            parsed_statuses.push(StatusExpectation {
                source: source.to_string(),
                start,
                end,
            });
        }
        let parsed_headers = headers
            .iter()
            .map(|source| {
                let (name, value) = source
                    .split_once(':')
                    .map_or((source.as_str(), None), |(name, value)| {
                        (name, Some(value.trim().to_string()))
                    });
                if name.trim().is_empty() {
                    return Err(AppError::local("expected header name cannot be empty"));
                }
                Ok(HeaderExpectation {
                    name: name.trim().to_string(),
                    value,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let parsed_regexes = body_regexes
            .iter()
            .map(|source| {
                Regex::new(source)
                    .map(|regex| (source.clone(), regex))
                    .map_err(|error| {
                        AppError::local(format!("invalid --expect-body-regex {source:?}: {error}"))
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(Self {
            statuses: parsed_statuses,
            headers: parsed_headers,
            min_body_bytes,
            max_body_bytes,
            body_contains: body_contains.to_vec(),
            body_regexes: parsed_regexes,
            json: Vec::new(),
        }))
    }

    pub fn parse_with_json(
        statuses: &[String],
        headers: &[String],
        min_body_bytes: Option<u64>,
        max_body_bytes: Option<u64>,
        body_contains: &[String],
        body_regexes: &[String],
        json: &[String],
    ) -> Result<Option<Self>, AppError> {
        let existing = Self::parse(
            statuses,
            headers,
            min_body_bytes,
            max_body_bytes,
            body_contains,
            body_regexes,
        )?;
        if json.is_empty() {
            return Ok(existing);
        }
        let mut expectations = existing.unwrap_or(Self {
            statuses: Vec::new(),
            headers: Vec::new(),
            min_body_bytes,
            max_body_bytes,
            body_contains: Vec::new(),
            body_regexes: Vec::new(),
            json: Vec::new(),
        });
        for spec in json {
            let (pointer, expected) = spec.split_once('=').ok_or_else(|| {
                AppError::local("JSON assertion requires POINTER=JSON or POINTER:type=TYPE")
            })?;
            let (pointer, expectation) = if let Some(pointer) = pointer.strip_suffix(":type") {
                if !["null", "boolean", "number", "string", "array", "object"].contains(&expected) {
                    return Err(AppError::local(
                        "JSON type must be null, boolean, number, string, array, or object",
                    ));
                }
                (pointer, JsonExpectation::Type(expected.into()))
            } else {
                (
                    pointer,
                    JsonExpectation::Equal(serde_json::from_str(expected).map_err(|_| {
                        AppError::local("JSON assertion expected value must be valid JSON")
                    })?),
                )
            };
            if !pointer.is_empty() && !pointer.starts_with('/') {
                return Err(AppError::local(
                    "JSON Pointer must be empty or start with /",
                ));
            }
            let mut chars = pointer.chars();
            while let Some(character) = chars.next() {
                if character == '~' && !matches!(chars.next(), Some('0' | '1')) {
                    return Err(AppError::local("JSON Pointer escapes must be ~0 or ~1"));
                }
            }
            expectations.json.push((pointer.into(), expectation));
        }
        Ok(Some(expectations))
    }

    pub fn requires_body_content(&self) -> bool {
        !self.body_contains.is_empty() || !self.body_regexes.is_empty() || !self.json.is_empty()
    }

    pub fn check(
        &self,
        response: &ResponseHeaders,
        body_size: u64,
        body: Option<&str>,
    ) -> AssertionSummary {
        let mut failures = Vec::new();
        if !self.statuses.is_empty()
            && !self
                .statuses
                .iter()
                .any(|expected| (expected.start..=expected.end).contains(&response.status_code))
        {
            failures.push(AssertionFailure {
                kind: "status".into(),
                expected: self
                    .statuses
                    .iter()
                    .map(|value| value.source.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                actual: response.status_code.to_string(),
            });
        }
        for expected in &self.headers {
            let actual = response
                .fields
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(&expected.name))
                .map(|(_, value)| value);
            match (actual, &expected.value) {
                (None, _) => failures.push(AssertionFailure {
                    kind: "header".into(),
                    expected: expected.name.clone(),
                    actual: "<missing>".into(),
                }),
                (Some(actual), Some(value)) if !actual.lines().any(|line| line == value) => {
                    failures.push(AssertionFailure {
                        kind: "header".into(),
                        expected: format!("{}: {value}", expected.name),
                        actual: format!("{}: {actual}", expected.name),
                    });
                }
                _ => {}
            }
        }
        if let Some(minimum) = self.min_body_bytes {
            if body_size < minimum {
                failures.push(AssertionFailure {
                    kind: "body_size".into(),
                    expected: format!(">= {minimum} bytes"),
                    actual: format!("{body_size} bytes"),
                });
            }
        }
        if let Some(maximum) = self.max_body_bytes {
            if body_size > maximum {
                failures.push(AssertionFailure {
                    kind: "body_size".into(),
                    expected: format!("<= {maximum} bytes"),
                    actual: format!("{body_size} bytes"),
                });
            }
        }
        let body = body.unwrap_or("");
        for expected in &self.body_contains {
            if !body.contains(expected) {
                failures.push(AssertionFailure {
                    kind: "body_contains".into(),
                    expected: expected.clone(),
                    actual: "<not found>".into(),
                });
            }
        }
        for (source, regex) in &self.body_regexes {
            if !regex.is_match(body) {
                failures.push(AssertionFailure {
                    kind: "body_regex".into(),
                    expected: source.clone(),
                    actual: "<no match>".into(),
                });
            }
        }
        if !self.json.is_empty() {
            let document = serde_json::from_str::<serde_json::Value>(body);
            for (pointer, expected) in &self.json {
                let actual = document
                    .as_ref()
                    .ok()
                    .and_then(|value| value.pointer(pointer));
                let pass = actual.is_some_and(|actual| match expected {
                    JsonExpectation::Equal(expected) => actual == expected,
                    JsonExpectation::Type(kind) => match kind.as_str() {
                        "null" => actual.is_null(),
                        "boolean" => actual.is_boolean(),
                        "number" => actual.is_number(),
                        "string" => actual.is_string(),
                        "array" => actual.is_array(),
                        "object" => actual.is_object(),
                        _ => false,
                    },
                });
                if !pass {
                    failures.push(AssertionFailure {
                        kind: "json".into(),
                        expected: format!("JSON assertion at pointer {pointer:?}"),
                        actual: if document.is_err() {
                            "<invalid JSON>"
                        } else if actual.is_none() {
                            "<missing>"
                        } else {
                            "<value or type mismatch>"
                        }
                        .into(),
                    });
                }
            }
        }
        AssertionSummary {
            pass: failures.is_empty(),
            failures,
        }
    }
}

fn parse_status(value: &str) -> Result<u16, AppError> {
    let parsed = value
        .trim()
        .parse::<u16>()
        .map_err(|_| AppError::local(format!("invalid expected HTTP status {value:?}")))?;
    if (100..=599).contains(&parsed) {
        Ok(parsed)
    } else {
        Err(AppError::local(format!(
            "expected HTTP status must be between 100 and 599, got {parsed}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn json_pointers_types_and_invalid_documents_are_checked() {
        let expectations = Expectations::parse_with_json(
            &[],
            &[],
            None,
            None,
            &[],
            &[],
            &[
                "/a~1b/~0key=2".into(),
                "/items:type=array".into(),
                "/nothing:type=null".into(),
            ],
        )
        .unwrap()
        .unwrap();
        assert!(
            expectations
                .check(
                    &response(200),
                    0,
                    Some(r#"{"a/b":{"~key":2},"items":[],"nothing":null}"#)
                )
                .pass
        );
        assert!(
            !expectations
                .check(&response(200), 0, Some("invalid-json"))
                .pass
        );
        assert!(!expectations.check(&response(200), 0, Some("{}")).pass);
        for assertion in ["path=2", "/a~2b=2", "/a:type=invalid", "/a=not-json"] {
            assert!(
                Expectations::parse_with_json(&[], &[], None, None, &[], &[], &[assertion.into()])
                    .is_err()
            );
        }
        let mut headers = response(200);
        headers
            .fields
            .insert("X-Test".into(), "first\nsecond".into());
        let expectations =
            Expectations::parse(&[], &["x-test:second".into()], None, None, &[], &[])
                .unwrap()
                .unwrap();
        assert!(expectations.check(&headers, 0, None).pass);
    }

    fn response(status: u16) -> ResponseHeaders {
        ResponseHeaders {
            status_line: format!("HTTP/1.1 {status}"),
            status_code: status,
            fields: BTreeMap::from([("Content-Type".into(), "application/json".into())]),
            display_text: String::new(),
        }
    }

    #[test]
    fn parses_slo_values() {
        assert_eq!(
            parse_slo("total=500, connect=100").unwrap(),
            vec![("total".into(), 500), ("connect".into(), 100)]
        );
        assert!(parse_slo("wat=1").is_err());
        assert!(parse_slo("total=0").is_err());
    }

    #[test]
    fn checks_status_ranges_and_headers() {
        let expectations = Expectations::parse(
            &["200-299".into()],
            &["content-type:application/json".into()],
            Some(2),
            Some(100),
            &["ok".into()],
            &[r#"\{"#.into()],
        )
        .unwrap()
        .unwrap();
        assert!(
            expectations
                .check(&response(204), 11, Some(r#"{"ok":true}"#))
                .pass
        );
        assert!(
            !expectations
                .check(&response(500), 11, Some(r#"{"ok":true}"#))
                .pass
        );
    }
}
