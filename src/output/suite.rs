use crate::suite::{SuiteEndpointResult, SuiteResult};

pub fn pretty(result: &SuiteResult) {
    println!(
        "Suite completed: {} endpoints, {} passed, {} failed ({:.1}% success)",
        result.summary.total_endpoints,
        result.summary.passed,
        result.summary.failed,
        result.summary.success_rate
    );
    println!(
        "Concurrency: {}   Samples: {}   Duration: {}ms",
        result.concurrency, result.summary.total_samples, result.summary.duration_ms
    );
    if result.canceled {
        println!("Suite was canceled; completed endpoint results were preserved.");
    }
    println!();
    println!("Result  Endpoint                         Samples  Exit  URL");
    for endpoint in &result.endpoints {
        let status = if endpoint.ok {
            "PASS"
        } else if result.policies.pass {
            "ALLOW"
        } else {
            "FAIL"
        };
        let samples = endpoint
            .result
            .as_ref()
            .map_or(0, |value| value.summary.requested);
        println!(
            "{status:<6}  {:<31} {:>7}  {:>4}  {}",
            truncate(&endpoint.name, 31),
            samples,
            endpoint.exit_code,
            endpoint.url
        );
        if let Some(error) = &endpoint.error {
            println!("        {error}");
        }
    }
    if !result.policies.violations.is_empty() {
        println!();
        println!("Suite policy violations:");
        for violation in &result.policies.violations {
            println!("  - {violation}");
        }
    }
}

pub fn csv(result: &SuiteResult) -> String {
    let mut output = String::from("endpoint,url,ok,exit_code,samples,passed,failed,error\n");
    for endpoint in &result.endpoints {
        let (samples, passed, failed) = endpoint.result.as_ref().map_or((0, 0, 0), |value| {
            (
                value.summary.requested,
                value.summary.passed,
                value.summary.failed,
            )
        });
        let fields = [
            endpoint.name.clone(),
            endpoint.url.clone(),
            endpoint.ok.to_string(),
            endpoint.exit_code.to_string(),
            samples.to_string(),
            passed.to_string(),
            failed.to_string(),
            endpoint.error.clone().unwrap_or_default(),
        ];
        output.push_str(
            &fields
                .iter()
                .map(|value| escape_csv(value))
                .collect::<Vec<_>>()
                .join(","),
        );
        output.push('\n');
    }
    output
}

pub fn junit(result: &SuiteResult) -> String {
    let failed_endpoints = result
        .endpoints
        .iter()
        .filter(|endpoint| !endpoint.ok)
        .count();
    let failures = if result.policies.pass {
        0
    } else {
        failed_endpoints + 1
    };
    let skipped = if result.policies.pass {
        failed_endpoints
    } else {
        0
    };
    let tests = result.endpoints.len() + 1;
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuite name=\"httpstatr suite\" tests=\"{tests}\" failures=\"{failures}\" skipped=\"{skipped}\" time=\"{:.3}\">\n",
        result.summary.duration_ms as f64 / 1000.0
    );
    for endpoint in &result.endpoints {
        xml.push_str(&format!(
            "  <testcase name=\"{}\" classname=\"httpstatr.suite\">",
            escape_xml(&endpoint.name)
        ));
        if endpoint.ok {
            xml.push_str("</testcase>\n");
        } else if result.policies.pass {
            xml.push_str(
                "\n    <skipped message=\"failure allowed by suite policy\"/>\n  </testcase>\n",
            );
        } else {
            let reason = endpoint_failure(endpoint);
            xml.push_str(&format!(
                "\n    <failure type=\"httpstatr.endpoint\" message=\"{}\">{}</failure>\n  </testcase>\n",
                escape_xml(&reason),
                escape_xml(&reason)
            ));
        }
    }
    xml.push_str("  <testcase name=\"suite policies\" classname=\"httpstatr.suite\">");
    if result.policies.pass {
        xml.push_str("</testcase>\n");
    } else {
        let reason = result.policies.violations.join("; ");
        xml.push_str(&format!(
            "\n    <failure type=\"httpstatr.suite_policy\" message=\"{}\">{}</failure>\n  </testcase>\n",
            escape_xml(&reason),
            escape_xml(&reason)
        ));
    }
    xml.push_str("</testsuite>\n");
    xml
}

pub fn markdown(result: &SuiteResult) -> String {
    let status = if result.ok { "PASS" } else { "FAIL" };
    let mut output = format!(
        "# httpstatr suite report\n\n**{status}** — {} endpoints, {} passed, {} failed, {:.1}% success{}\n\n| Result | Endpoint | Samples | Exit | URL |\n| --- | --- | ---: | ---: | --- |\n",
        result.summary.total_endpoints,
        result.summary.passed,
        result.summary.failed,
        result.summary.success_rate,
        if result.canceled { " (canceled)" } else { "" }
    );
    for endpoint in &result.endpoints {
        let status = if endpoint.ok {
            "PASS"
        } else if result.policies.pass {
            "ALLOWED"
        } else {
            "FAIL"
        };
        let samples = endpoint
            .result
            .as_ref()
            .map_or(0, |value| value.summary.requested);
        output.push_str(&format!(
            "| {status} | {} | {samples} | {} | `{}` |\n",
            escape_markdown(&endpoint.name),
            endpoint.exit_code,
            escape_markdown(&endpoint.url)
        ));
    }
    if !result.policies.violations.is_empty() {
        output.push_str("\n## Suite policy violations\n\n");
        for violation in &result.policies.violations {
            output.push_str(&format!("- {violation}\n"));
        }
    }
    output
}

fn endpoint_failure(endpoint: &SuiteEndpointResult) -> String {
    endpoint
        .error
        .clone()
        .unwrap_or_else(|| format!("endpoint failed with exit code {}", endpoint.exit_code))
}

fn truncate(value: &str, length: usize) -> String {
    if value.chars().count() <= length {
        value.to_string()
    } else {
        value
            .chars()
            .take(length.saturating_sub(1))
            .collect::<String>()
            + "…"
    }
}

fn escape_csv(value: &str) -> String {
    if value.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn escape_markdown(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_unicode_safely() {
        assert_eq!(truncate("hello", 5), "hello");
        assert_eq!(truncate("abcdef", 5), "abcd…");
    }
}
