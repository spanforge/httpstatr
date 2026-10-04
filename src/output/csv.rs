use crate::model::BatchResult;

pub fn render(result: &BatchResult) -> String {
    let mut output = String::from(
        "sample,url,ok,exit_code,status_code,dns_ms,connect_ms,tls_ms,server_ms,transfer_ms,total_ms,response_size_bytes,error\n",
    );
    for sample in &result.samples {
        let timings = sample.timings_ms.as_ref();
        let values = [
            sample.index.to_string(),
            result.url.clone(),
            sample.ok.to_string(),
            sample.exit_code.to_string(),
            sample
                .response
                .as_ref()
                .map(|response| response.status_code.to_string())
                .unwrap_or_default(),
            timings
                .map(|value| value.dns.to_string())
                .unwrap_or_default(),
            timings
                .map(|value| value.connect.to_string())
                .unwrap_or_default(),
            timings
                .map(|value| value.tls.to_string())
                .unwrap_or_default(),
            timings
                .map(|value| value.server.to_string())
                .unwrap_or_default(),
            timings
                .map(|value| value.transfer.to_string())
                .unwrap_or_default(),
            timings
                .map(|value| value.total.to_string())
                .unwrap_or_default(),
            sample
                .response_size_bytes
                .map(|value| value.to_string())
                .unwrap_or_default(),
            sample_error(sample),
        ];
        output.push_str(
            &values
                .iter()
                .map(|value| escape(value))
                .collect::<Vec<_>>()
                .join(","),
        );
        output.push('\n');
    }
    output
}

fn sample_error(sample: &crate::model::BatchSample) -> String {
    if let Some(error) = &sample.error {
        return error.clone();
    }
    let mut messages = Vec::new();
    if let Some(slo) = &sample.slo {
        messages.extend(slo.violations.iter().map(|violation| {
            format!(
                "SLO {}={}ms exceeds {}ms",
                violation.key, violation.actual_ms, violation.threshold_ms
            )
        }));
    }
    if let Some(assertions) = &sample.assertions {
        messages.extend(assertions.failures.iter().map(|failure| {
            format!(
                "{} expected {}, got {}",
                failure.kind, failure.expected, failure.actual
            )
        }));
    }
    messages.join("; ")
}

fn escape(value: &str) -> String {
    if value.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_csv_fields() {
        assert_eq!(escape("plain"), "plain");
        assert_eq!(escape("a,b"), "\"a,b\"");
        assert_eq!(escape("a\"b"), "\"a\"\"b\"");
    }
}
