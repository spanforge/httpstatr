use std::collections::BTreeMap;
use std::path::Path;

use crate::error::AppError;
use crate::model::{
    AggregateTimings, BatchResult, ComparisonResult, MetricDelta, RegressionResult,
    RegressionRuleResult, Statistics,
};

pub const REGRESSION_EXIT_CODE: u8 = 7;

const PHASES: &[&str] = &[
    "dns",
    "connect",
    "tls",
    "server",
    "transfer",
    "total",
    "namelookup",
    "initial_connect",
    "pretransfer",
    "starttransfer",
];
const STATISTICS: &[&str] = &[
    "min", "max", "mean", "median", "p90", "p95", "p99", "stddev",
];

#[derive(Clone, Debug)]
pub struct Rule {
    source: String,
    subject: Subject,
    operator: Operator,
    target: Target,
}

#[derive(Clone, Debug)]
enum Subject {
    Metric { statistic: String, phase: String },
    SuccessRate,
    ErrorRate,
}

#[derive(Clone, Copy, Debug)]
enum Operator {
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
}

#[derive(Clone, Copy, Debug)]
enum Target {
    Absolute(f64),
    Baseline(f64),
}

pub fn load_baseline(path: &Path) -> Result<BatchResult, AppError> {
    let text = crate::storage::read_text(path, 67_108_864)?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|error| {
        AppError::local(format!(
            "baseline {} is not valid JSON: {error}",
            path.display()
        ))
    })?;
    let version = value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if version != 2 && version != 3 {
        return Err(AppError::local(format!(
            "baseline {} uses schema version {version}; repeated-run schema version 2 or 3 is required",
            path.display()
        )));
    }
    serde_json::from_value(value).map_err(|error| {
        AppError::local(format!(
            "baseline {} is not a compatible aggregate result: {error}",
            path.display()
        ))
    })
}

pub fn compare(
    current: &BatchResult,
    baseline: &BatchResult,
) -> Result<ComparisonResult, AppError> {
    validate_compatible(current, baseline)?;
    let mut metrics = BTreeMap::new();
    for phase in PHASES {
        for statistic in STATISTICS {
            let key = format!("{statistic}.{phase}");
            let baseline_value = metric(&baseline.aggregate, &key)?;
            let current_value = metric(&current.aggregate, &key)?;
            let raw_change = current_value - baseline_value;
            let absolute_change = round_two(raw_change);
            let percent_change = if baseline_value == 0.0 {
                None
            } else {
                Some(round_two(raw_change / baseline_value * 100.0))
            };
            metrics.insert(
                key,
                MetricDelta {
                    baseline: baseline_value,
                    current: current_value,
                    absolute_change,
                    percent_change,
                },
            );
        }
    }
    Ok(ComparisonResult {
        baseline_url: baseline.url.clone(),
        metrics,
    })
}

pub fn parse_rules(specifications: &[String], has_baseline: bool) -> Result<Vec<Rule>, AppError> {
    specifications
        .iter()
        .map(|source| parse_rule(source, has_baseline))
        .collect()
}

pub fn evaluate(
    rules: &[Rule],
    current: &BatchResult,
    baseline: Option<&BatchResult>,
) -> Result<RegressionResult, AppError> {
    let mut results = Vec::with_capacity(rules.len());
    for rule in rules {
        let actual = subject_value(&rule.subject, current)?;
        let (threshold, baseline_value) = match rule.target {
            Target::Absolute(value) => (value, None),
            Target::Baseline(factor) => {
                let baseline = baseline.ok_or_else(|| {
                    AppError::local(format!(
                        "regression rule {:?} requires --compare BASELINE",
                        rule.source
                    ))
                })?;
                let value = subject_value(&rule.subject, baseline)?;
                (value * factor, Some(value))
            }
        };
        let triggered = rule.operator.compare(actual, threshold);
        results.push(RegressionRuleResult {
            rule: rule.source.clone(),
            pass: !triggered,
            actual: round_two(actual),
            operator: rule.operator.as_str().to_string(),
            threshold: round_two(threshold),
            baseline: baseline_value.map(round_two),
        });
    }
    Ok(RegressionResult {
        pass: results.iter().all(|result| result.pass),
        rules: results,
    })
}

fn parse_rule(source: &str, has_baseline: bool) -> Result<Rule, AppError> {
    let (lhs, operator, rhs) = split_expression(source)?;
    let subject = match lhs {
        "success_rate" => Subject::SuccessRate,
        "error_rate" => Subject::ErrorRate,
        metric_name => {
            validate_metric(metric_name)?;
            let (statistic, phase) = metric_name.split_once('.').unwrap();
            Subject::Metric {
                statistic: statistic.to_string(),
                phase: phase.to_string(),
            }
        }
    };
    let target = if let Some(rest) = rhs.strip_prefix("baseline") {
        if !has_baseline {
            return Err(AppError::local(format!(
                "regression rule {source:?} requires --compare BASELINE"
            )));
        }
        let factor = if rest.trim().is_empty() {
            1.0
        } else {
            let value = rest.trim().strip_prefix('*').ok_or_else(|| {
                AppError::local(format!(
                    "invalid regression threshold {rhs:?}; expected baseline or baseline*FACTOR"
                ))
            })?;
            parse_nonnegative(value.trim(), "baseline multiplier")?
        };
        Target::Baseline(factor)
    } else {
        let suffix = if matches!(subject, Subject::SuccessRate | Subject::ErrorRate) {
            "%"
        } else {
            "ms"
        };
        let value = rhs.trim().strip_suffix(suffix).unwrap_or(rhs.trim());
        Target::Absolute(parse_nonnegative(value.trim(), "rule threshold")?)
    };
    Ok(Rule {
        source: source.to_string(),
        subject,
        operator,
        target,
    })
}

fn split_expression(source: &str) -> Result<(&str, Operator, &str), AppError> {
    for (token, operator) in [
        (">=", Operator::GreaterEqual),
        ("<=", Operator::LessEqual),
        (">", Operator::Greater),
        ("<", Operator::Less),
    ] {
        if let Some((lhs, rhs)) = source.split_once(token) {
            let lhs = lhs.trim();
            let rhs = rhs.trim();
            if lhs.is_empty() || rhs.is_empty() {
                break;
            }
            return Ok((lhs, operator, rhs));
        }
    }
    Err(AppError::local(format!(
        "invalid regression rule {source:?}; expected SUBJECT OPERATOR THRESHOLD"
    )))
}

fn validate_metric(name: &str) -> Result<(), AppError> {
    let Some((statistic, phase)) = name.split_once('.') else {
        return Err(AppError::local(format!(
            "invalid regression metric {name:?}; expected STATISTIC.PHASE such as p95.total"
        )));
    };
    if !STATISTICS.contains(&statistic) || !PHASES.contains(&phase) {
        return Err(AppError::local(format!(
            "unknown regression metric {name:?}; statistics: {}, phases: {}",
            STATISTICS.join(", "),
            PHASES.join(", ")
        )));
    }
    Ok(())
}

fn parse_nonnegative(value: &str, label: &str) -> Result<f64, AppError> {
    let value = value
        .parse::<f64>()
        .map_err(|_| AppError::local(format!("{label} must be a nonnegative number")))?;
    if value.is_finite() && value >= 0.0 {
        Ok(value)
    } else {
        Err(AppError::local(format!(
            "{label} must be a nonnegative number"
        )))
    }
}

fn validate_compatible(current: &BatchResult, baseline: &BatchResult) -> Result<(), AppError> {
    if baseline.aggregate.total.count == 0 {
        return Err(AppError::local(
            "baseline has no successful timing samples to compare",
        ));
    }
    if current.url != baseline.url {
        return Err(AppError::local(format!(
            "baseline URL {:?} does not match current URL {:?}",
            baseline.url, current.url
        )));
    }
    if current.configuration.curl_args != baseline.configuration.curl_args {
        return Err(AppError::local(
            "baseline curl arguments do not match the current request",
        ));
    }
    Ok(())
}

fn subject_value(subject: &Subject, result: &BatchResult) -> Result<f64, AppError> {
    match subject {
        Subject::Metric { statistic, phase } => {
            metric(&result.aggregate, &format!("{statistic}.{phase}"))
        }
        Subject::SuccessRate => Ok(result.summary.success_rate),
        Subject::ErrorRate => Ok(100.0 - result.summary.success_rate),
    }
}

fn metric(aggregate: &AggregateTimings, name: &str) -> Result<f64, AppError> {
    let (statistic, phase) = name
        .split_once('.')
        .ok_or_else(|| AppError::local(format!("invalid aggregate metric {name:?}")))?;
    let stats = phase_statistics(aggregate, phase)
        .ok_or_else(|| AppError::local(format!("unknown timing phase {phase:?}")))?;
    statistic_value(stats, statistic)
        .ok_or_else(|| AppError::local(format!("unknown timing statistic {statistic:?}")))
}

fn phase_statistics<'a>(aggregate: &'a AggregateTimings, phase: &str) -> Option<&'a Statistics> {
    match phase {
        "dns" => Some(&aggregate.dns),
        "connect" => Some(&aggregate.connect),
        "tls" => Some(&aggregate.tls),
        "server" => Some(&aggregate.server),
        "transfer" => Some(&aggregate.transfer),
        "total" => Some(&aggregate.total),
        "namelookup" => Some(&aggregate.namelookup),
        "initial_connect" => Some(&aggregate.initial_connect),
        "pretransfer" => Some(&aggregate.pretransfer),
        "starttransfer" => Some(&aggregate.starttransfer),
        _ => None,
    }
}

fn statistic_value(statistics: &Statistics, name: &str) -> Option<f64> {
    match name {
        "min" => Some(statistics.min),
        "max" => Some(statistics.max),
        "mean" => Some(statistics.mean),
        "median" => Some(statistics.median),
        "p90" => Some(statistics.p90),
        "p95" => Some(statistics.p95),
        "p99" => Some(statistics.p99),
        "stddev" => Some(statistics.stddev),
        _ => None,
    }
}

impl Operator {
    fn compare(self, left: f64, right: f64) -> bool {
        match self {
            Self::Greater => left > right,
            Self::GreaterEqual => left >= right,
            Self::Less => left < right,
            Self::LessEqual => left <= right,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Greater => ">",
            Self::GreaterEqual => ">=",
            Self::Less => "<",
            Self::LessEqual => "<=",
        }
    }
}

fn round_two(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_absolute_and_baseline_rules() {
        let rules = parse_rules(
            &[
                "p95.total > baseline*1.10".into(),
                "error_rate > 1%".into(),
                "mean.server >= 250ms".into(),
            ],
            true,
        )
        .unwrap();
        assert_eq!(rules.len(), 3);
        assert!(parse_rules(&["p95.total > baseline".into()], false).is_err());
        assert!(parse_rules(&["wat.total > 1".into()], false).is_err());
    }

    #[test]
    fn operators_detect_failure_conditions() {
        assert!(Operator::Greater.compare(11.0, 10.0));
        assert!(!Operator::LessEqual.compare(11.0, 10.0));
    }

    #[test]
    fn rate_subjects_are_percentages() {
        let summary = crate::model::BatchSummary {
            requested: 10,
            transport_successful: 9,
            passed: 8,
            failed: 2,
            success_rate: 80.0,
            duration_ms: 1,
            requests_per_second: 1.0,
        };
        assert_eq!(
            subject_value(&Subject::ErrorRate, &batch_with_summary(summary)).unwrap(),
            20.0
        );
    }

    #[test]
    fn comparison_rejects_different_requests() {
        let summary = crate::model::BatchSummary {
            requested: 1,
            transport_successful: 1,
            passed: 1,
            failed: 0,
            success_rate: 100.0,
            duration_ms: 1,
            requests_per_second: 1.0,
        };
        let mut baseline = batch_with_summary(summary.clone());
        baseline.aggregate.total.count = 1;
        let mut current = batch_with_summary(summary);
        current.aggregate.total.count = 1;
        current.url = "https://different.example.test".into();
        assert!(compare(&current, &baseline).is_err());
    }

    fn batch_with_summary(summary: crate::model::BatchSummary) -> BatchResult {
        BatchResult {
            schema_version: 2,
            url: "https://example.test".into(),
            ok: true,
            exit_code: 0,
            warmup_count: 0,
            warmup_failures: 0,
            configuration: crate::model::BatchConfiguration {
                repeat: 10,
                warmup: 0,
                delay_seconds: 0.0,
                connect_timeout_seconds: None,
                timeout_seconds: None,
                curl_args: Vec::new(),
                curl_version: None,
                slo: None,
                expect_status: Vec::new(),
                expect_header: Vec::new(),
                min_body_bytes: None,
                max_body_bytes: None,
                expect_body_contains: Vec::new(),
                expect_body_regex: Vec::new(),
                expect_json: Vec::new(),
                context: None,
                comparison_baseline: None,
                regression_rules: Vec::new(),
            },
            summary,
            aggregate: AggregateTimings::default(),
            samples: Vec::new(),
            comparison: None,
            regression: None,
            diagnostics: Vec::new(),
        }
    }
}
