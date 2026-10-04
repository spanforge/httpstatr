use crate::model::{AggregateTimings, Metrics, Statistics};

pub fn aggregate(samples: &[Metrics]) -> AggregateTimings {
    AggregateTimings {
        dns: statistics(samples.iter().map(Metrics::dns)),
        connect: statistics(samples.iter().map(Metrics::connect)),
        tls: statistics(samples.iter().map(Metrics::tls)),
        server: statistics(samples.iter().map(Metrics::server)),
        transfer: statistics(samples.iter().map(Metrics::transfer)),
        total: statistics(samples.iter().map(|sample| sample.time_total)),
        namelookup: statistics(samples.iter().map(|sample| sample.time_namelookup)),
        initial_connect: statistics(samples.iter().map(|sample| sample.time_connect)),
        pretransfer: statistics(samples.iter().map(|sample| sample.time_pretransfer)),
        starttransfer: statistics(samples.iter().map(|sample| sample.time_starttransfer)),
    }
}

fn statistics(values: impl Iterator<Item = u64>) -> Statistics {
    let mut values: Vec<f64> = values.map(|value| value as f64).collect();
    if values.is_empty() {
        return Statistics::default();
    }
    values.sort_by(f64::total_cmp);
    let count = values.len();
    let mean = values.iter().sum::<f64>() / count as f64;
    let variance = values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / count as f64;
    Statistics {
        count,
        min: values[0],
        max: values[count - 1],
        mean: round_two(mean),
        median: percentile(&values, 0.50),
        p90: percentile(&values, 0.90),
        p95: percentile(&values, 0.95),
        p99: percentile(&values, 0.99),
        stddev: round_two(variance.sqrt()),
    }
}

// Nearest-rank percentile: rank = ceil(p * N), using one-based ranks.
fn percentile(sorted: &[f64], percentile: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (percentile * sorted.len() as f64).ceil() as usize;
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

fn round_two(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculates_deterministic_nearest_rank_statistics() {
        let samples: Vec<Metrics> = (1..=100)
            .map(|value| Metrics {
                time_total: value,
                ..Metrics::default()
            })
            .collect();
        let stats = aggregate(&samples).total;
        assert_eq!(stats.count, 100);
        assert_eq!(stats.min, 1.0);
        assert_eq!(stats.max, 100.0);
        assert_eq!(stats.mean, 50.5);
        assert_eq!(stats.median, 50.0);
        assert_eq!(stats.p90, 90.0);
        assert_eq!(stats.p95, 95.0);
        assert_eq!(stats.p99, 99.0);
    }

    #[test]
    fn empty_statistics_are_zeroed() {
        let stats = aggregate(&[]).total;
        assert_eq!(stats.count, 0);
        assert_eq!(stats.p95, 0.0);
    }
}
