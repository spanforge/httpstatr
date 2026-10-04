use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::Path;

use crate::cli::Config;
use crate::error::AppError;
use crate::model::{
    AssertionSummary, BatchResult, Metrics, ResponseHeaders, Statistics, Violation,
};

pub fn render(
    config: &Config,
    metrics: &Metrics,
    headers: &ResponseHeaders,
    body_path: &Path,
    violations: &[Violation],
    assertions: Option<&AssertionSummary>,
) -> Result<(), AppError> {
    let color = io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    if config.show_ip {
        println!(
            "Connected to {}:{} from {}:{}\n",
            paint(&metrics.remote_ip, "36", color),
            paint(&metrics.remote_port, "36", color),
            metrics.local_ip,
            metrics.local_port
        );
    }
    for (index, line) in headers.display_text.lines().enumerate() {
        if index == 0 {
            println!("{}", paint(line, "32", color));
        } else if let Some((key, value)) = line.split_once(':') {
            println!(
                "{}{}",
                paint(&format!("{key}:"), "38;5;246", color),
                paint(value, "36", color)
            );
        } else {
            println!("{line}");
        }
    }
    println!();
    if config.show_body {
        let mut file = fs::File::open(body_path)
            .map_err(|error| AppError::local(format!("could not read response body: {error}")))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|error| AppError::local(format!("could not read response body: {error}")))?;
        let text = String::from_utf8_lossy(&bytes);
        let mut limit = 1024.min(text.len());
        while !text.is_char_boundary(limit) {
            limit -= 1;
        }
        print!("{}", &text[..limit]);
        if text.len() > limit {
            println!("{}\n", paint("...", "36", color));
            print!(
                "{} is truncated (1024 out of {})",
                paint("Body", "32", color),
                text.len()
            );
            if config.save_body {
                print!(", stored in: {}", body_path.display());
            }
            println!();
        } else if !text.ends_with('\n') {
            println!();
        }
    } else if config.save_body {
        println!(
            "{} stored in: {}",
            paint("Body", "32", color),
            body_path.display()
        );
    }

    println!();
    if config.url.starts_with("https://") {
        println!(
            "  DNS Lookup   TCP Connection   TLS Handshake   Server Processing   Content Transfer"
        );
        println!(
            "[ {:^9} | {:^13} | {:^13} | {:^17} | {:^18} ]",
            ms(metrics.dns()),
            ms(metrics.connect()),
            ms(metrics.tls()),
            ms(metrics.server()),
            ms(metrics.transfer())
        );
        println!(
            "    namelookup:{:<8}        connect:{:<8}       pretransfer:{:<8}      starttransfer:{:<8}  total:{}",
            ms(metrics.time_namelookup),
            ms(metrics.time_connect),
            ms(metrics.time_pretransfer),
            ms(metrics.time_starttransfer),
            ms(metrics.time_total)
        );
    } else {
        println!("  DNS Lookup   TCP Connection   Server Processing   Content Transfer");
        println!(
            "[ {:^9} | {:^13} | {:^17} | {:^18} ]",
            ms(metrics.dns()),
            ms(metrics.connect()),
            ms(metrics.server()),
            ms(metrics.transfer())
        );
        println!(
            "    namelookup:{:<8}        connect:{:<8}        starttransfer:{:<8}  total:{}",
            ms(metrics.time_namelookup),
            ms(metrics.time_connect),
            ms(metrics.time_starttransfer),
            ms(metrics.time_total)
        );
    }
    if config.show_speed {
        println!(
            "speed_download: {:.1} KiB/s, speed_upload: {:.1} KiB/s",
            metrics.speed_download / 1024.0,
            metrics.speed_upload / 1024.0
        );
    }
    println!(
        "HTTP: {}   Connections: {}   Redirects: {} ({}ms)   Effective URL: {}",
        metrics.http_version,
        metrics.connection_count,
        metrics.redirect_count,
        metrics.redirect_time,
        metrics.effective_url
    );
    for violation in violations {
        println!(
            "{}",
            paint(
                &format!(
                    "SLO VIOLATION: {} = {}ms (threshold: {}ms)",
                    violation.key, violation.actual_ms, violation.threshold_ms
                ),
                "31",
                color
            )
        );
    }
    if let Some(summary) = assertions {
        for failure in &summary.failures {
            println!(
                "{}",
                paint(
                    &format!(
                        "ASSERTION FAILED: {} expected {}, got {}",
                        failure.kind, failure.expected, failure.actual
                    ),
                    "31",
                    color
                )
            );
        }
    }
    Ok(())
}

pub fn render_batch(result: &BatchResult) {
    println!(
        "Completed {} requests ({} passed, {} failed; {} received responses) after {} warmups",
        result.summary.requested,
        result.summary.passed,
        result.summary.failed,
        result.summary.transport_successful,
        result.warmup_count
    );
    if result.warmup_failures > 0 {
        println!("Warmup failures: {}", result.warmup_failures);
    }
    println!(
        "Success rate: {:.1}%   Duration: {}ms   Throughput: {:.2} req/s",
        result.summary.success_rate, result.summary.duration_ms, result.summary.requests_per_second
    );
    println!();
    println!("Timing       min     mean   median      p90      p95      p99      max   stddev");
    print_stats("DNS", &result.aggregate.dns);
    print_stats("Connect", &result.aggregate.connect);
    print_stats("TLS", &result.aggregate.tls);
    print_stats("Server", &result.aggregate.server);
    print_stats("Transfer", &result.aggregate.transfer);
    print_stats("Total", &result.aggregate.total);

    if let Some(comparison) = &result.comparison {
        println!();
        println!("Baseline comparison (p95):");
        println!("Phase       baseline   current    change   change %");
        for phase in ["dns", "connect", "tls", "server", "transfer", "total"] {
            let key = format!("p95.{phase}");
            if let Some(delta) = comparison.metrics.get(&key) {
                let percent = delta
                    .percent_change
                    .map_or_else(|| "n/a".into(), |value| format!("{value:+.1}%"));
                println!(
                    "{phase:<10} {:>7.1}ms {:>8.1}ms {:>+8.1}ms {percent:>10}",
                    delta.baseline, delta.current, delta.absolute_change
                );
            }
        }
    }
    if let Some(regression) = &result.regression {
        println!();
        println!("Regression policies:");
        for rule in &regression.rules {
            let status = if rule.pass { "PASS" } else { "FAIL" };
            println!(
                "  {status}: {} (actual {:.2}, threshold {} {:.2})",
                rule.rule, rule.actual, rule.operator, rule.threshold
            );
        }
    }
    if !result.diagnostics.is_empty() {
        println!();
        println!("Diagnostics (observations, not confirmed causes):");
        for diagnostic in &result.diagnostics {
            println!("  [{}] {}", diagnostic.level, diagnostic.observation);
            println!("      Evidence: {}", diagnostic.evidence);
            println!("      Next step: {}", diagnostic.suggestion);
        }
    }

    let failed: Vec<_> = result.samples.iter().filter(|sample| !sample.ok).collect();
    if !failed.is_empty() {
        println!();
        println!("Failed samples:");
        for sample in failed {
            if let Some(error) = &sample.error {
                println!("  #{}: {error}", sample.index);
            } else {
                if let Some(slo) = &sample.slo {
                    for violation in &slo.violations {
                        println!(
                            "  #{}: SLO {}={}ms exceeds {}ms",
                            sample.index,
                            violation.key,
                            violation.actual_ms,
                            violation.threshold_ms
                        );
                    }
                }
                if let Some(assertions) = &sample.assertions {
                    for failure in &assertions.failures {
                        println!(
                            "  #{}: {} expected {}, got {}",
                            sample.index, failure.kind, failure.expected, failure.actual
                        );
                    }
                }
            }
        }
    }
}

fn print_stats(label: &str, stats: &Statistics) {
    println!(
        "{label:<9} {:>6.0}ms {:>6.1}ms {:>6.0}ms {:>6.0}ms {:>6.0}ms {:>6.0}ms {:>6.0}ms {:>7.1}ms",
        stats.min,
        stats.mean,
        stats.median,
        stats.p90,
        stats.p95,
        stats.p99,
        stats.max,
        stats.stddev
    );
}

fn paint(text: &str, code: &str, enabled: bool) -> String {
    if enabled {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

fn ms(value: u64) -> String {
    format!("{value}ms")
}
