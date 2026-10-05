use std::collections::BTreeMap;

use crate::model::AssertionSummary;

const SENSITIVE_HEADERS: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "cookie",
    "set-cookie",
    "x-api-key",
    "api-key",
    "apikey",
    "x-auth-token",
];

pub fn is_sensitive_header(name: &str) -> bool {
    let name = name.trim().to_ascii_lowercase();
    SENSITIVE_HEADERS.contains(&name.as_str())
        || name.contains("token")
        || name.contains("secret")
        || name.contains("password")
        || name.contains("api-key")
        || name.contains("apikey")
}

pub fn redact_url(url: &str) -> String {
    let mut redacted = url.to_string();
    if let Some(scheme_end) = redacted.find("://") {
        let authority_start = scheme_end + 3;
        let authority_end = redacted[authority_start..]
            .find(['/', '?', '#'])
            .map_or(redacted.len(), |offset| authority_start + offset);
        let authority = &redacted[authority_start..authority_end];
        if let Some(at) = authority.rfind('@') {
            redacted = format!(
                "{}<redacted>@{}{}",
                &redacted[..authority_start],
                &authority[at + 1..],
                &redacted[authority_end..]
            );
        }
    }
    redact_query(&redacted)
}

fn redact_query(url: &str) -> String {
    let Some(query_start) = url.find('?') else {
        return url.to_string();
    };
    let query_end = url[query_start..]
        .find('#')
        .map_or(url.len(), |offset| query_start + offset);
    let query = &url[query_start + 1..query_end];
    let redacted = query
        .split('&')
        .map(|parameter| {
            let Some((name, value)) = parameter.split_once('=') else {
                return parameter.to_string();
            };
            if is_sensitive_query_parameter(name) && !value.is_empty() {
                format!("{name}=<redacted>")
            } else {
                parameter.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("&");
    format!("{}?{}{}", &url[..query_start], redacted, &url[query_end..])
}

fn is_sensitive_query_parameter(name: &str) -> bool {
    let decoded = url::form_urlencoded::parse(format!("{name}=").as_bytes())
        .next()
        .map(|(name, _)| name.into_owned())
        .unwrap_or_else(|| name.to_string());
    let normalized = decoded.trim().to_ascii_lowercase().replace('-', "_");
    normalized.contains("token")
        || normalized.contains("secret")
        || normalized.contains("password")
        || matches!(
            normalized.as_str(),
            "api_key" | "apikey" | "key" | "auth" | "signature" | "sig"
        )
}

pub fn redact_header_line(value: &str) -> String {
    let Some((name, header_value)) = value.split_once(':') else {
        return value.to_string();
    };
    let normalized_name = name.trim_start_matches(['>', '<', ' ']).trim();
    if is_sensitive_header(normalized_name) {
        format!("{name}: <redacted>")
    } else if normalized_name.eq_ignore_ascii_case("location") {
        format!("{name}: {}", redact_url(header_value.trim()))
    } else {
        value.to_string()
    }
}

pub fn redact_headers(fields: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    fields
        .iter()
        .map(|(name, value)| {
            let value = if is_sensitive_header(name) {
                "<redacted>".to_string()
            } else if name.eq_ignore_ascii_case("location") {
                redact_url(value)
            } else {
                value.clone()
            };
            (name.clone(), value)
        })
        .collect()
}

pub fn redact_display_headers(text: &str) -> String {
    text.lines()
        .map(redact_header_line)
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn redact_assertions(summary: &AssertionSummary) -> AssertionSummary {
    let mut redacted = summary.clone();
    for failure in &mut redacted.failures {
        if matches!(failure.kind.as_str(), "body_contains" | "body_regex") {
            failure.expected = "<redacted>".into();
        }
        if failure.kind != "header" {
            continue;
        }
        failure.expected = redact_header_line(&failure.expected);
        if failure.actual != "<missing>" {
            failure.actual = redact_header_line(&failure.actual);
        }
    }
    redacted
}

pub fn redact_command(url: &str, args: &[String]) -> Vec<String> {
    let mut output = Vec::with_capacity(args.len() + 1);
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if matches!(arg.as_str(), "-H" | "--header" | "--proxy-header") && index + 1 < args.len() {
            output.push(arg.clone());
            output.push(redact_header_line(&args[index + 1]));
            index += 2;
            continue;
        }
        if matches!(
            arg.as_str(),
            "-u" | "--user"
                | "--proxy-user"
                | "-U"
                | "--oauth2-bearer"
                | "-b"
                | "--cookie"
                | "-d"
                | "--data"
                | "--data-raw"
                | "--data-binary"
                | "--data-urlencode"
                | "--json"
        ) && index + 1 < args.len()
        {
            output.push(arg.clone());
            output.push("<redacted>".to_string());
            index += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--proxy-header=") {
            output.push(format!("--proxy-header={}", redact_header_line(value)));
        } else if let Some(value) = arg.strip_prefix("--header=") {
            output.push(format!("--header={}", redact_header_line(value)));
        } else if let Some(value) = arg.strip_prefix("-H") {
            output.push(format!("-H{}", redact_header_line(value)));
        } else if arg.starts_with("-d") && arg.len() > 2 {
            output.push("-d<redacted>".to_string());
        } else if arg.starts_with("-b") && arg.len() > 2 {
            output.push("-b<redacted>".to_string());
        } else if arg.starts_with("-u") && arg.len() > 2 {
            output.push("-u<redacted>".to_string());
        } else if arg.starts_with("-U") && arg.len() > 2 {
            output.push("-U<redacted>".to_string());
        } else if [
            "--user=",
            "--proxy-user=",
            "--oauth2-bearer=",
            "--cookie=",
            "--data=",
            "--data-raw=",
            "--data-binary=",
            "--data-urlencode=",
            "--json=",
        ]
        .iter()
        .any(|prefix| arg.starts_with(prefix))
        {
            let name = arg.split('=').next().unwrap_or(arg);
            output.push(format!("{name}=<redacted>"));
        } else {
            output.push(redact_url(arg));
        }
        index += 1;
    }
    output.push(redact_url(url));
    output
}

pub fn redact_curl_text(text: &str, url: &str, args: &[String]) -> String {
    let mut secrets = collect_secret_values(url, args);
    secrets.sort_by_key(|value| std::cmp::Reverse(value.len()));
    let mut output = text
        .lines()
        .map(redact_header_line)
        .collect::<Vec<_>>()
        .join("\n");
    for secret in secrets {
        if !secret.is_empty() {
            output = output.replace(&secret, "<redacted>");
        }
    }
    output
}

fn collect_secret_values(url: &str, args: &[String]) -> Vec<String> {
    let mut values = Vec::new();
    if let Some(scheme_end) = url.find("://") {
        let rest = &url[scheme_end + 3..];
        if let Some(at) = rest.find('@') {
            values.push(rest[..at].to_string());
        }
    }
    if let Some(query_start) = url.find('?') {
        let query_end = url[query_start..]
            .find('#')
            .map_or(url.len(), |offset| query_start + offset);
        for parameter in url[query_start + 1..query_end].split('&') {
            if let Some((name, value)) = parameter.split_once('=') {
                if is_sensitive_query_parameter(name) && !value.is_empty() {
                    values.push(value.to_string());
                }
            }
        }
    }
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if matches!(arg.as_str(), "-H" | "--header" | "--proxy-header") && index + 1 < args.len() {
            if let Some((name, value)) = args[index + 1].split_once(':') {
                if is_sensitive_header(name) {
                    values.push(value.trim().to_string());
                }
            }
            index += 2;
            continue;
        }
        if matches!(
            arg.as_str(),
            "-u" | "--user"
                | "--proxy-user"
                | "-U"
                | "--oauth2-bearer"
                | "-b"
                | "--cookie"
                | "-d"
                | "--data"
                | "--data-raw"
                | "--data-binary"
                | "--data-urlencode"
                | "--json"
        ) && index + 1 < args.len()
        {
            values.push(args[index + 1].clone());
            index += 2;
            continue;
        }
        let sensitive_assignment = arg.split_once('=').filter(|(name, _)| {
            matches!(
                *name,
                "--user"
                    | "--proxy-user"
                    | "--oauth2-bearer"
                    | "--cookie"
                    | "--data"
                    | "--data-raw"
                    | "--data-binary"
                    | "--data-urlencode"
                    | "--json"
            )
        });
        if let Some(header) = arg
            .strip_prefix("--header=")
            .or_else(|| arg.strip_prefix("--proxy-header="))
            .or_else(|| arg.strip_prefix("-H").filter(|value| !value.is_empty()))
        {
            if let Some((name, value)) = header.split_once(':') {
                if is_sensitive_header(name) {
                    values.push(value.trim().to_string());
                }
            }
        } else if let Some((_, value)) = sensitive_assignment {
            values.push(value.to_string());
        } else if (arg.starts_with("-d")
            || arg.starts_with("-b")
            || arg.starts_with("-u")
            || arg.starts_with("-U"))
            && arg.len() > 2
        {
            values.push(arg[2..].to_string());
        }
        index += 1;
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_attached_credentials_and_header_echoes() {
        let args = vec![
            "-uuser:attached-secret".into(),
            "-Uproxy:proxy-secret".into(),
            "-HAuthorization: Bearer header-secret".into(),
            "--json={\"key\":\"json-secret\"}".into(),
        ];
        let command =
            redact_command("https://example.test/?api%5Fkey=query-secret", &args).join(" ");
        let text = redact_curl_text(
            "user:attached-secret proxy:proxy-secret Bearer header-secret {\"key\":\"json-secret\"}",
            "https://example.test",
            &args,
        );
        for secret in [
            "attached-secret",
            "proxy-secret",
            "header-secret",
            "json-secret",
            "query-secret",
        ] {
            assert!(!command.contains(secret));
            assert!(!text.contains(secret));
        }
    }

    #[test]
    fn redacts_url_credentials() {
        assert_eq!(
            redact_url("https://user:secret@example.com/path"),
            "https://<redacted>@example.com/path"
        );
    }

    #[test]
    fn redacts_sensitive_url_query_parameters() {
        assert_eq!(
            redact_url("https://example.test/path?api_key=secret&page=2&access-token=abc#section"),
            "https://example.test/path?api_key=<redacted>&page=2&access-token=<redacted>#section"
        );
        assert_eq!(
            redact_url("example.test?token=abc"),
            "example.test?token=<redacted>"
        );
    }

    #[test]
    fn redacts_mixed_case_headers() {
        assert_eq!(
            redact_header_line("Authorization: Bearer secret"),
            "Authorization: <redacted>"
        );
        assert_eq!(
            redact_header_line("> X-Api-Key: secret"),
            "> X-Api-Key: <redacted>"
        );
        assert_eq!(
            redact_header_line("Accept: text/plain"),
            "Accept: text/plain"
        );
        assert_eq!(
            redact_header_line("Location: /login?token=secret&next=home"),
            "Location: /login?token=<redacted>&next=home"
        );
    }

    #[test]
    fn redacts_short_and_long_curl_arguments() {
        let args = vec![
            "-H".into(),
            "Cookie: session=abc".into(),
            "--user=bob:secret".into(),
            "-dpayload-secret".into(),
        ];
        let redacted = redact_command("https://example.com", &args).join(" ");
        assert!(!redacted.contains("session=abc"));
        assert!(!redacted.contains("bob:secret"));
        assert!(!redacted.contains("payload-secret"));
    }

    #[test]
    fn redacts_query_secrets_from_verbose_curl_text() {
        let text = "> GET /health?access_token=verbose-secret HTTP/1.1\n";
        let redacted = redact_curl_text(
            text,
            "https://example.test/health?access_token=verbose-secret",
            &[],
        );
        assert!(!redacted.contains("verbose-secret"));
        assert!(redacted.contains("<redacted>"));
    }

    #[test]
    fn redacts_sensitive_assertion_failures() {
        let summary = AssertionSummary {
            pass: false,
            failures: vec![crate::model::AssertionFailure {
                kind: "header".into(),
                expected: "Authorization: expected-secret".into(),
                actual: "Authorization: actual-secret".into(),
            }],
        };
        let redacted = redact_assertions(&summary);
        assert!(!format!("{redacted:?}").contains("secret"));
    }
}
