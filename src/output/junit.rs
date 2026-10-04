use crate::model::{BatchResult, BatchSample};

pub fn render(result: &BatchResult) -> String {
    let has_regression = result.regression.is_some();
    let tests = result.samples.len() + usize::from(has_regression);
    let failures = result.samples.iter().filter(|sample| !sample.ok).count()
        + usize::from(
            result
                .regression
                .as_ref()
                .is_some_and(|regression| !regression.pass),
        );
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuite name=\"httpstatr\" tests=\"{tests}\" failures=\"{failures}\" time=\"{:.3}\">\n  <properties>\n    <property name=\"url\" value=\"{}\"/>\n    <property name=\"schema_version\" value=\"{}\"/>\n  </properties>\n",
        result.summary.duration_ms as f64 / 1000.0,
        escape(&result.url),
        result.schema_version
    );
    for sample in &result.samples {
        let time = sample
            .timings_ms
            .as_ref()
            .map_or(0.0, |timings| timings.total as f64 / 1000.0);
        xml.push_str(&format!(
            "  <testcase name=\"sample {}\" classname=\"httpstatr.request\" time=\"{time:.3}\">",
            sample.index
        ));
        if sample.ok {
            xml.push_str("</testcase>\n");
        } else {
            let message = sample_failure(sample);
            xml.push_str(&format!(
                "\n    <failure type=\"httpstatr.sample\" message=\"{}\">{}</failure>\n  </testcase>\n",
                escape(&message),
                escape(&message)
            ));
        }
    }
    if let Some(regression) = &result.regression {
        xml.push_str(
            "  <testcase name=\"regression policies\" classname=\"httpstatr.regression\" time=\"0.000\">",
        );
        if regression.pass {
            xml.push_str("</testcase>\n");
        } else {
            let message = regression
                .rules
                .iter()
                .filter(|rule| !rule.pass)
                .map(|rule| {
                    format!(
                        "{} (actual {:.2}, threshold {:.2})",
                        rule.rule, rule.actual, rule.threshold
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");
            xml.push_str(&format!(
                "\n    <failure type=\"httpstatr.regression\" message=\"{}\">{}</failure>\n  </testcase>\n",
                escape(&message),
                escape(&message)
            ));
        }
    }
    xml.push_str("</testsuite>\n");
    xml
}

fn sample_failure(sample: &BatchSample) -> String {
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
    if messages.is_empty() {
        format!("sample failed with exit code {}", sample.exit_code)
    } else {
        messages.join("; ")
    }
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_xml_values() {
        assert_eq!(escape("<&\"'>"), "&lt;&amp;&quot;&apos;&gt;");
    }
}
