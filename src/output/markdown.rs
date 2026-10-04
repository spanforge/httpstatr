use crate::model::{BatchResult, Statistics};

pub fn render(result: &BatchResult) -> String {
    let status = if result.ok { "PASS" } else { "FAIL" };
    let mut markdown = format!(
        "# httpstatr report\n\n**{status}** — `{}`\n\n- Requests: {}\n- Passed: {}\n- Failed: {}\n- Success rate: {:.1}%\n- Duration: {} ms\n- Throughput: {:.2} req/s\n\n",
        result.url,
        result.summary.requested,
        result.summary.passed,
        result.summary.failed,
        result.summary.success_rate,
        result.summary.duration_ms,
        result.summary.requests_per_second
    );
    markdown.push_str("## Timing summary\n\n| Phase | Mean | Median | p90 | p95 | p99 | Min | Max | Stddev |\n| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n");
    for (name, statistics) in [
        ("DNS", &result.aggregate.dns),
        ("Connect", &result.aggregate.connect),
        ("TLS", &result.aggregate.tls),
        ("Server", &result.aggregate.server),
        ("Transfer", &result.aggregate.transfer),
        ("Total", &result.aggregate.total),
    ] {
        push_statistics(&mut markdown, name, statistics);
    }

    if let Some(comparison) = &result.comparison {
        markdown.push_str("\n## Baseline comparison\n\n| Metric | Baseline | Current | Change | Change % |\n| --- | ---: | ---: | ---: | ---: |\n");
        for phase in ["dns", "connect", "tls", "server", "transfer", "total"] {
            let key = format!("p95.{phase}");
            if let Some(delta) = comparison.metrics.get(&key) {
                let percent = delta
                    .percent_change
                    .map_or_else(|| "n/a".into(), |value| format!("{value:+.2}%"));
                markdown.push_str(&format!(
                    "| `{key}` | {:.2} ms | {:.2} ms | {:+.2} ms | {percent} |\n",
                    delta.baseline, delta.current, delta.absolute_change
                ));
            }
        }
    }

    if let Some(regression) = &result.regression {
        markdown.push_str("\n## Regression policies\n\n| Result | Rule | Actual | Threshold |\n| --- | --- | ---: | ---: |\n");
        for rule in &regression.rules {
            let status = if rule.pass { "PASS" } else { "FAIL" };
            markdown.push_str(&format!(
                "| {status} | `{}` | {:.2} | {} {:.2} |\n",
                escape_cell(&rule.rule),
                rule.actual,
                rule.operator,
                rule.threshold
            ));
        }
    }

    let failures: Vec<_> = result.samples.iter().filter(|sample| !sample.ok).collect();
    if !failures.is_empty() {
        markdown.push_str("\n## Failed samples\n\n");
        for sample in failures {
            let reason = if let Some(error) = &sample.error {
                error.clone()
            } else {
                let mut parts = Vec::new();
                if let Some(slo) = &sample.slo {
                    parts.extend(slo.violations.iter().map(|violation| {
                        format!(
                            "SLO {}={}ms > {}ms",
                            violation.key, violation.actual_ms, violation.threshold_ms
                        )
                    }));
                }
                if let Some(assertions) = &sample.assertions {
                    parts.extend(assertions.failures.iter().map(|failure| {
                        format!(
                            "{} expected {}, got {}",
                            failure.kind, failure.expected, failure.actual
                        )
                    }));
                }
                parts.join("; ")
            };
            markdown.push_str(&format!("- Sample {}: {}\n", sample.index, reason));
        }
    }
    markdown
}

fn push_statistics(markdown: &mut String, name: &str, value: &Statistics) {
    markdown.push_str(&format!(
        "| {name} | {:.2} ms | {:.2} ms | {:.2} ms | {:.2} ms | {:.2} ms | {:.2} ms | {:.2} ms | {:.2} ms |\n",
        value.mean,
        value.median,
        value.p90,
        value.p95,
        value.p99,
        value.min,
        value.max,
        value.stddev
    ));
}

fn escape_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_markdown_table_cells() {
        assert_eq!(escape_cell("a|b\nc"), "a\\|b c");
    }
}
