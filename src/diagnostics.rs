use crate::model::{BatchResult, Diagnostic, Statistics};

pub fn analyze(result: &BatchResult) -> Vec<Diagnostic> {
    let mut found = Vec::new();
    if result.aggregate.total.count > 0 && result.aggregate.total.count < 100 {
        found.push(Diagnostic {
            code: "small_tail_sample".into(), level: "info".into(),
            observation: "Tail percentiles are based on fewer than 100 successful samples".into(),
            evidence: format!("{} successful timing samples; p99 may equal the maximum", result.aggregate.total.count),
            suggestion: "Collect more samples under comparable conditions; this warning does not change policy results.".into(),
        });
    }
    let total = result.aggregate.total.median;
    if total <= 0.0 {
        return found;
    }
    share(
        &mut found,
        "dns_share",
        "DNS is a large part of median latency",
        result.aggregate.dns.median,
        total,
        (0.25, 20.0),
        "Check resolver latency, DNS caching, and whether the hostname lookup is repeated unnecessarily.",
    );
    let average_size = {
        let sizes: Vec<u64> = result
            .samples
            .iter()
            .filter_map(|sample| sample.response_size_bytes)
            .collect();
        if sizes.is_empty() {
            0.0
        } else {
            sizes.iter().sum::<u64>() as f64 / sizes.len() as f64
        }
    };
    if result.aggregate.transfer.median >= 100.0 && average_size <= 100_000.0 {
        found.push(Diagnostic {
            code: "slow_transfer".into(),
            level: "investigate".into(),
            observation: "Content transfer is slow relative to the response size".into(),
            evidence: format!("median transfer={:.1}ms, average body={average_size:.0} bytes", result.aggregate.transfer.median),
            suggestion: "Check packet loss, bandwidth limits, proxy buffering, and response streaming behavior.".into(),
        });
    }
    share(
        &mut found,
        "tls_share",
        "TLS setup is a large part of median latency",
        result.aggregate.tls.median,
        total,
        (0.30, 20.0),
        "Inspect connection reuse, session resumption, certificate-chain size, and network distance.",
    );
    share(
        &mut found,
        "server_share",
        "Server processing dominates median latency",
        result.aggregate.server.median,
        total,
        (0.60, 50.0),
        "Correlate the request with server traces and profile application and dependency time.",
    );
    let stats = &result.aggregate.total;
    if stats.count >= 5 && high_variance(stats) {
        found.push(Diagnostic {
            code: "tail_variance".into(),
            level: "investigate".into(),
            observation: "Total latency has a wide tail".into(),
            evidence: format!(
                "median={:.1}ms, p95={:.1}ms, stddev={:.1}ms across {} samples",
                stats.median, stats.p95, stats.stddev, stats.count
            ),
            suggestion: "Compare slow samples with server traces and check queueing, retries, and shared-resource contention.".into(),
        });
    }
    let redirect_count = result
        .samples
        .iter()
        .filter_map(|sample| sample.connection.as_ref())
        .map(|connection| connection.redirect_count)
        .max()
        .unwrap_or(0);
    if redirect_count > 0 {
        found.push(Diagnostic {
            code: "redirect_chain".into(),
            level: "info".into(),
            observation: "The request followed redirects".into(),
            evidence: format!("up to {redirect_count} redirect(s) per sample"),
            suggestion: "Call the final URL directly where possible and verify each redirect is intentional.".into(),
        });
    }
    found
}

fn share(
    found: &mut Vec<Diagnostic>,
    code: &str,
    observation: &str,
    phase: f64,
    total: f64,
    thresholds: (f64, f64),
    suggestion: &str,
) {
    let ratio = phase / total;
    if phase >= thresholds.1 && ratio >= thresholds.0 {
        found.push(Diagnostic {
            code: code.into(),
            level: "investigate".into(),
            observation: observation.into(),
            evidence: format!(
                "median={phase:.1}ms ({:.1}% of {:.1}ms total)",
                ratio * 100.0,
                total
            ),
            suggestion: suggestion.into(),
        });
    }
}

fn high_variance(stats: &Statistics) -> bool {
    stats.p95 >= stats.median * 1.5 || (stats.mean > 0.0 && stats.stddev / stats.mean >= 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variance_rule_is_deterministic() {
        let stats = Statistics {
            count: 10,
            mean: 100.0,
            median: 80.0,
            p95: 150.0,
            stddev: 10.0,
            ..Statistics::default()
        };
        assert!(high_variance(&stats));
    }
}
