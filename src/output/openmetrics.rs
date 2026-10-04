use crate::model::{BatchResult, Statistics};

pub fn render(result: &BatchResult) -> String {
    let mut output = String::from(
        "# HELP httpstatr_request_duration_milliseconds HTTP request phase duration.\n\
# TYPE httpstatr_request_duration_milliseconds gauge\n",
    );
    let url = escape_label(&result.url);
    for (phase, stats) in [
        ("dns", &result.aggregate.dns),
        ("connect", &result.aggregate.connect),
        ("tls", &result.aggregate.tls),
        ("server", &result.aggregate.server),
        ("transfer", &result.aggregate.transfer),
        ("total", &result.aggregate.total),
    ] {
        statistics(&mut output, &url, phase, stats);
    }
    output.push_str("# HELP httpstatr_success_ratio Fraction of samples that passed.\n# TYPE httpstatr_success_ratio gauge\n");
    output.push_str(&format!(
        "httpstatr_success_ratio{{url=\"{url}\"}} {}\n",
        result.summary.success_rate / 100.0
    ));
    output.push_str("# HELP httpstatr_samples_total Number of measured samples.\n# TYPE httpstatr_samples_total gauge\n");
    output.push_str(&format!(
        "httpstatr_samples_total{{url=\"{url}\",outcome=\"passed\"}} {}\n",
        result.summary.passed
    ));
    output.push_str(&format!(
        "httpstatr_samples_total{{url=\"{url}\",outcome=\"failed\"}} {}\n",
        result.summary.failed
    ));
    output.push_str("# EOF\n");
    output
}

fn statistics(output: &mut String, url: &str, phase: &str, stats: &Statistics) {
    for (statistic, value) in [
        ("min", stats.min),
        ("mean", stats.mean),
        ("median", stats.median),
        ("p90", stats.p90),
        ("p95", stats.p95),
        ("p99", stats.p99),
        ("max", stats.max),
        ("stddev", stats.stddev),
    ] {
        output.push_str(&format!("httpstatr_request_duration_milliseconds{{url=\"{url}\",phase=\"{phase}\",statistic=\"{statistic}\"}} {value}\n"));
    }
}

fn escape_label(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::escape_label;
    #[test]
    fn escapes_openmetrics_labels() {
        assert_eq!(escape_label("a\"b\\c"), "a\\\"b\\\\c");
    }
}
