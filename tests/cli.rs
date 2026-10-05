use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn server(delay: Duration) -> (String, JoinHandle<()>) {
    server_n(delay, 1)
}

fn server_n(delay: Duration, requests: usize) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        for _ in 0..requests {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request);
            thread::sleep(delay);
            let response = concat!(
                "HTTP/1.1 200 OK\r\n",
                "Content-Type: application/json\r\n",
                "X-Test: yes\r\n",
                "Set-Cookie: session=server-secret\r\n",
                "Content-Length: 11\r\n",
                "Connection: close\r\n",
                "\r\n",
                "{\"ok\":true}"
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (format!("http://{address}/"), handle)
}

fn run(url: &str, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_httpstatr"));
    command
        .arg(url)
        .args(args)
        .env("HTTPSTAT_SAVE_BODY", "false");
    command.output().unwrap()
}

fn temporary_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "httpstatr-test-{name}-{}-{nanos}",
        std::process::id()
    ))
}

fn run_suite_file(path: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_httpstatr"))
        .arg("--file")
        .arg(path)
        .args(args)
        .env("HTTPSTAT_SAVE_BODY", "false")
        .output()
        .unwrap()
}

#[test]
fn download_limit_stops_oversized_response() {
    let (url, handle) = server(Duration::ZERO);
    let output = run(&url, &["--max-download-bytes", "5", "--format", "json"]);
    handle.join().unwrap();
    assert_eq!(output.status.code(), Some(63));
}

#[test]
fn json_assertions_check_values_types_and_missing_fields() {
    let (url, handle) = server(Duration::ZERO);
    let output = run(
        &url,
        &[
            "--expect-json",
            "/ok=true",
            "--expect-json",
            "/ok:type=boolean",
            "--format",
            "json",
        ],
    );
    handle.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let (url, handle) = server(Duration::ZERO);
    let output = run(
        &url,
        &[
            "--expect-json",
            "/missing=\"assertion-secret\"",
            "--format",
            "json",
        ],
    );
    handle.join().unwrap();
    assert_eq!(output.status.code(), Some(5));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("assertion-secret"));
}

#[test]
fn doctor_reports_local_capabilities_without_printing_proxy_credentials() {
    let output = Command::new(env!("CARGO_BIN_EXE_httpstatr"))
        .arg("doctor")
        .env("HTTPS_PROXY", "http://user:doctor-secret@127.0.0.1:1")
        .env_remove("CURL_CA_BUNDLE")
        .env_remove("SSL_CERT_FILE")
        .env_remove("SSL_CERT_DIR")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("HTTP supported"));
    assert!(!text.contains("doctor-secret"));
}

fn tls_server(cert_path: &std::path::Path) -> (String, JoinHandle<()>) {
    use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
    let certificate = include_bytes!("fixtures/localhost-cert.pem");
    fs::write(cert_path, certificate).unwrap();
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from_pem_slice(certificate).unwrap()],
            PrivateKeyDer::from_pem_slice(include_bytes!("fixtures/localhost-key.pem")).unwrap(),
        )
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let connection = rustls::ServerConnection::new(std::sync::Arc::new(config)).unwrap();
        let mut stream = rustls::StreamOwned::new(connection, socket);
        let mut request = [0; 4096];
        if stream.read(&mut request).is_ok() {
            let _ = stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            let _ = stream.flush();
        }
    });
    (format!("https://localhost:{}/", address.port()), handle)
}

#[test]
fn https_validates_certificates_and_exports_tls_timings() {
    let directory = tempfile::tempdir().unwrap();
    let certificate = directory.path().join("server.pem");
    let (url, handle) = tls_server(&certificate);
    let output = run(
        &url,
        &[
            "--cacert",
            certificate.to_str().unwrap(),
            "--noproxy",
            "*",
            "--format",
            "json",
        ],
    );
    handle.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["response"]["status_code"], 200);
    assert!(
        result["timings_ms"]["tls"].as_u64().unwrap()
            <= result["timings_ms"]["pretransfer"].as_u64().unwrap()
    );
    let (url, handle) = tls_server(&certificate);
    let output = run(&url, &["--noproxy", "*", "--format", "json"]);
    handle.join().unwrap();
    assert_eq!(output.status.code(), Some(60));
}

#[test]
fn explicit_http_proxy_is_forwarded_and_headers_are_measured() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let proxy = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0; 4096];
        let size = stream.read(&mut request).unwrap();
        assert!(
            String::from_utf8_lossy(&request[..size])
                .starts_with("GET http://example.test/health HTTP/1.1")
        );
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
    });
    let output = run(
        "http://example.test/health",
        &[
            "--proxy",
            &proxy,
            "--noproxy",
            "",
            "--expect-status",
            "200",
            "--format",
            "json",
        ],
    );
    handle.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn exported_junit_is_well_formed_and_har_timings_are_consistent() {
    let (url, handle) = server(Duration::ZERO);
    let output = run(&url, &["--format", "junit"]);
    handle.join().unwrap();
    assert!(output.status.success());
    let mut reader = quick_xml::Reader::from_reader(output.stdout.as_slice());
    let mut depth = 0;
    loop {
        match reader.read_event().unwrap() {
            quick_xml::events::Event::Start(element) => {
                for attribute in element.attributes() {
                    attribute.unwrap();
                }
                depth += 1;
            }
            quick_xml::events::Event::End(_) => depth -= 1,
            quick_xml::events::Event::Eof => break,
            _ => {}
        }
    }
    assert_eq!(depth, 0);
    let (url, handle) = server(Duration::ZERO);
    let output = run(&url, &["--format", "har"]);
    handle.join().unwrap();
    assert!(output.status.success());
    let har: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let entry = &har["log"]["entries"][0];
    assert_eq!(entry["request"]["httpVersion"], "HTTP/1.1");
    let times = &entry["timings"];
    let sum: u64 = ["blocked", "dns", "connect", "send", "wait", "receive"]
        .iter()
        .map(|key| times[key].as_u64().unwrap())
        .sum();
    assert_eq!(sum, entry["time"].as_u64().unwrap());
}

#[test]
fn suite_profiles_resolve_urls_and_environment_secrets() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("suite.toml");
    let (url, handle) = server(Duration::ZERO);
    fs::write(&path, format!("schema_version=1\n[profiles.staging]\nBASE_URL='{url}'\n[[requests]]\nname='health'\nurl='${{BASE_URL}}'\nheaders=['Authorization: Bearer ${{env:HTTPSTATR_TEST_TOKEN}}']\nexpect_json=['/ok=true']\n")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_httpstatr"))
        .args([
            "--file",
            path.to_str().unwrap(),
            "--profile",
            "staging",
            "--format",
            "json",
        ])
        .env("HTTPSTATR_TEST_TOKEN", "profile-secret")
        .output()
        .unwrap();
    handle.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("profile-secret"));
    let output = Command::new(env!("CARGO_BIN_EXE_httpstatr"))
        .args([
            "--file",
            path.to_str().unwrap(),
            "--profile",
            "staging",
            "--validate",
        ])
        .env_remove("HTTPSTATR_TEST_TOKEN")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("HTTPSTATR_TEST_TOKEN"));
}

#[test]
fn whole_run_deadline_preserves_completed_samples_and_skips_delay() {
    let (url, handle) = server(Duration::ZERO);
    let started = std::time::Instant::now();
    let output = run(
        &url,
        &[
            "--repeat",
            "3",
            "--delay",
            "10",
            "--run-timeout",
            "0.5",
            "--format",
            "json",
        ],
    );
    handle.join().unwrap();
    assert!(started.elapsed() < Duration::from_secs(3));
    assert_eq!(output.status.code(), Some(6));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["summary"]["transport_successful"], 1);
    assert_eq!(result["samples"][1]["exit_code"], 28);
}

#[test]
fn suite_validation_needs_no_curl_or_network_and_checks_all_policies() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("suite.toml");
    fs::write(&path, "schema_version=1\n[[requests]]\nname='health'\nurl='http://127.0.0.1:1/'\nexpect_status=['200']\n").unwrap();
    let output = run_suite_file(&path, &["--validate", "--curl-bin", "does-not-exist"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::write(&path, "schema_version=1\n[[requests]]\nname='health'\nurl='http://127.0.0.1:1/'\nexpect_body_regex=['[']\n").unwrap();
    assert!(!run_suite_file(&path, &["--validate"]).status.success());
}

#[test]
fn concurrent_history_writes_and_interrupted_replacement_are_safe() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("history.jsonl");
    let (url, handle) = server_n(Duration::ZERO, 6);
    let mut children = Vec::new();
    for _ in 0..6 {
        children.push(
            Command::new(env!("CARGO_BIN_EXE_httpstatr"))
                .arg(&url)
                .args(["--history", path.to_str().unwrap(), "--format", "json"])
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
    }
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }
    handle.join().unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert_eq!(text.lines().count(), 6);
    for line in text.lines() {
        assert!(serde_json::from_str::<serde_json::Value>(line).is_ok());
    }
    let backup = directory.path().join("history.jsonl.bak");
    fs::rename(&path, &backup).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_httpstatr"))
        .args([
            "history",
            "--history",
            path.to_str().unwrap(),
            "prune",
            "--keep",
            "2",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!backup.exists());
    assert_eq!(fs::read_to_string(&path).unwrap().lines().count(), 2);
    let output = Command::new(env!("CARGO_BIN_EXE_httpstatr"))
        .args([
            "history",
            "--history",
            path.to_str().unwrap(),
            "export",
            "--output",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(fs::read_to_string(&path).unwrap().lines().count(), 2);
}

#[test]
fn history_import_redacts_credentials_and_rejects_invalid_sources_before_writing() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.jsonl");
    let destination = directory.path().join("destination.jsonl");
    let (url, handle) = server(Duration::ZERO);
    assert!(
        run(
            &url,
            &["--history", source.to_str().unwrap(), "--format", "json"]
        )
        .status
        .success()
    );
    handle.join().unwrap();
    let mut record: serde_json::Value =
        serde_json::from_str(fs::read_to_string(&source).unwrap().trim()).unwrap();
    record["result"]["url"] = serde_json::json!("https://example.test/?token=import-secret");
    record["result"]["configuration"]["curl_args"] = serde_json::json!(["-uuser:import-password"]);
    record["result"]["samples"][0]["response"]["headers"]["Authorization"] =
        serde_json::json!("Bearer import-header");
    let text = format!("{}\n", serde_json::to_string(&record).unwrap());
    fs::write(&source, format!("{text}invalid-record\n")).unwrap();
    let import = || {
        Command::new(env!("CARGO_BIN_EXE_httpstatr"))
            .args([
                "history",
                "--history",
                destination.to_str().unwrap(),
                "import",
                source.to_str().unwrap(),
            ])
            .output()
            .unwrap()
    };
    assert!(!import().status.success());
    assert!(!destination.exists());
    fs::write(&source, &text).unwrap();
    assert!(import().status.success());
    let result = fs::read_to_string(&destination).unwrap();
    for secret in ["import-secret", "import-password", "import-header"] {
        assert!(!result.contains(secret));
    }
    fs::write(&destination, "incomplete").unwrap();
    assert!(!import().status.success());
    assert_eq!(fs::read_to_string(&destination).unwrap(), "incomplete");
}

#[test]
fn emits_json_and_passes_assertions() {
    let (url, handle) = server(Duration::ZERO);
    let output = run(
        &url,
        &[
            "--format",
            "json",
            "--expect-status",
            "200-299",
            "--expect-header",
            "content-type:application/json",
        ],
    );
    handle.join().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["schema_version"], 1);
    assert_eq!(result["assertions"]["pass"], true);
    assert_eq!(result["response"]["headers"]["Set-Cookie"], "<redacted>");
}

#[test]
fn failed_assertion_uses_exit_code_five() {
    let (url, handle) = server(Duration::ZERO);
    let output = run(&url, &["--format", "jsonl", "--expect-status", "404"]);
    handle.join().unwrap();
    assert_eq!(output.status.code(), Some(5));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["assertions"]["pass"], false);
    assert_eq!(result["assertions"]["failures"][0]["actual"], "200");
}

#[test]
fn timeout_is_forwarded_to_curl() {
    let (url, handle) = server(Duration::from_millis(250));
    let output = run(&url, &["--timeout", "0.05", "--format", "json"]);
    handle.join().unwrap();
    assert_eq!(output.status.code(), Some(28));
}

#[test]
fn missing_curl_reports_installation_guidance() {
    let output = run(
        "http://127.0.0.1:1/",
        &["--curl-bin", "httpstatr-curl-does-not-exist"],
    );
    assert_eq!(output.status.code(), Some(1));
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("Install curl 7.50.0 or newer"));
    assert!(error.contains("--curl-bin"));
}

#[test]
fn debug_and_verbose_output_redact_secrets() {
    let (url, handle) = server(Duration::ZERO);
    let mut command = Command::new(env!("CARGO_BIN_EXE_httpstatr"));
    let output = command
        .arg(&url)
        .args([
            "-H",
            "Authorization: Bearer request-secret",
            "-v",
            "--format",
            "json",
        ])
        .env("HTTPSTAT_DEBUG", "true")
        .env("HTTPSTAT_SAVE_BODY", "false")
        .output()
        .unwrap();
    handle.join().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!combined.contains("request-secret"));
    assert!(!combined.contains("server-secret"));
    assert!(combined.contains("<redacted>"));
}

#[test]
fn repeated_run_reports_warmups_samples_and_aggregates() {
    let (url, handle) = server_n(Duration::ZERO, 4);
    let output = run(
        &url,
        &[
            "--repeat",
            "3",
            "--warmup",
            "1",
            "--format",
            "json",
            "--expect-body-contains",
            "\"ok\":true",
            "--expect-body-regex",
            r#"\{"ok":true\}"#,
            "--min-body-bytes",
            "10",
            "--max-body-bytes",
            "11",
            "-H",
            "Authorization: Bearer batch-secret",
        ],
    );
    handle.join().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["schema_version"], 3);
    assert_eq!(result["warmup_count"], 1);
    assert_eq!(result["summary"]["requested"], 3);
    assert_eq!(result["summary"]["passed"], 3);
    assert_eq!(result["aggregate"]["total"]["count"], 3);
    assert!(result["configuration"]["curl_version"].is_string());
    assert_eq!(result["samples"].as_array().unwrap().len(), 3);
    assert_eq!(result["samples"][0]["response_size_bytes"], 11);
    assert_eq!(
        result["configuration"]["curl_args"][1],
        "Authorization: <redacted>"
    );
    assert!(
        !String::from_utf8(output.stdout)
            .unwrap()
            .contains("batch-secret")
    );
}

#[test]
fn csv_contains_one_row_per_sample() {
    let (url, handle) = server_n(Duration::ZERO, 2);
    let output = run(&url, &["--repeat", "2", "--format", "csv"]);
    handle.join().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let csv = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<_> = csv.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].starts_with("sample,url,ok,exit_code"));
}

#[test]
fn body_assertion_failure_uses_exit_code_five() {
    let (url, handle) = server(Duration::ZERO);
    let output = run(&url, &["--max-body-bytes", "5", "--format", "json"]);
    handle.join().unwrap();
    assert_eq!(output.status.code(), Some(5));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["assertions"]["failures"][0]["kind"], "body_size");
}

#[test]
fn repeated_run_preserves_partial_transport_failures() {
    let (url, handle) = server_n(Duration::ZERO, 2);
    let output = run(&url, &["--repeat", "3", "--format", "json"]);
    handle.join().unwrap();
    assert_eq!(output.status.code(), Some(6));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["summary"]["transport_successful"], 2);
    assert_eq!(result["summary"]["failed"], 1);
    assert!(result["samples"][2]["error"].is_string());
}

#[test]
fn repeated_run_applies_delay_between_samples() {
    let (url, handle) = server_n(Duration::ZERO, 2);
    let output = run(
        &url,
        &["--repeat", "2", "--delay", "0.05", "--format", "json"],
    );
    handle.join().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(result["summary"]["duration_ms"].as_u64().unwrap() >= 45);
}

#[test]
fn compares_baseline_and_enforces_regression_rule() {
    let (url, handle) = server_n(Duration::ZERO, 6);
    let baseline_path = temporary_path("baseline.json");
    let baseline_path_text = baseline_path.to_string_lossy().into_owned();
    let baseline = run(
        &url,
        &[
            "--repeat",
            "3",
            "--format",
            "json",
            "--save",
            &baseline_path_text,
        ],
    );
    assert_eq!(baseline.status.code(), Some(0));
    let current = run(
        &url,
        &[
            "--repeat",
            "3",
            "--format",
            "json",
            "--compare",
            &baseline_path_text,
            "--fail-if",
            "p95.total >= baseline*0",
        ],
    );
    handle.join().unwrap();
    let _ = fs::remove_file(&baseline_path);
    assert_eq!(current.status.code(), Some(7));
    let result: serde_json::Value = serde_json::from_slice(&current.stdout).unwrap();
    assert!(result["comparison"]["metrics"]["p95.total"].is_object());
    assert_eq!(result["regression"]["pass"], false);
    assert_eq!(result["regression"]["rules"][0]["pass"], false);
}

#[test]
fn absolute_regression_rule_works_without_baseline() {
    let (url, handle) = server(Duration::ZERO);
    let output = run(
        &url,
        &["--fail-if", "success_rate < 101%", "--format", "json"],
    );
    handle.join().unwrap();
    assert_eq!(output.status.code(), Some(7));
}

#[test]
fn junit_and_markdown_reports_render() {
    let (junit_url, junit_handle) = server(Duration::ZERO);
    let junit = run(&junit_url, &["--format", "junit", "--expect-status", "404"]);
    junit_handle.join().unwrap();
    assert_eq!(junit.status.code(), Some(5));
    let junit_text = String::from_utf8(junit.stdout).unwrap();
    assert!(junit_text.starts_with("<?xml version=\"1.0\""));
    assert!(junit_text.contains("<failure type=\"httpstatr.sample\""));

    let (markdown_url, markdown_handle) = server(Duration::ZERO);
    let markdown = run(&markdown_url, &["--format", "markdown"]);
    markdown_handle.join().unwrap();
    assert_eq!(markdown.status.code(), Some(0));
    let markdown_text = String::from_utf8(markdown.stdout).unwrap();
    assert!(markdown_text.starts_with("# httpstatr report"));
    assert!(markdown_text.contains("## Timing summary"));
}

#[test]
fn rejects_schema_v1_as_aggregate_baseline() {
    let baseline_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("json_v1.json");
    let baseline_text = baseline_path.to_string_lossy().into_owned();
    let output = run(
        "http://127.0.0.1:1/",
        &["--compare", &baseline_text, "--format", "json"],
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("schema version 1")
    );
}

#[test]
fn assertion_precedes_slo_and_regression_exit_codes() {
    let (url, handle) = server(Duration::from_millis(15));
    let output = run(
        &url,
        &[
            "--expect-status",
            "404",
            "--slo",
            "total=1",
            "--fail-if",
            "success_rate < 101%",
            "--format",
            "json",
        ],
    );
    handle.join().unwrap();
    assert_eq!(output.status.code(), Some(5));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["samples"][0]["assertions"]["pass"], false);
    assert_eq!(result["samples"][0]["slo"]["pass"], false);
    assert_eq!(result["regression"]["pass"], false);
}

#[test]
fn suite_runs_concurrently_in_declaration_order_and_enforces_policies() {
    let (slow_url, slow_handle) = server(Duration::from_millis(250));
    let (fast_url, fast_handle) = server(Duration::from_millis(250));
    let suite_path = temporary_path("suite.toml");
    fs::write(
        &suite_path,
        format!(
            r#"schema_version = 1
concurrency = 2

[policies]
min_success_rate = 100
max_failures = 0

[[requests]]
name = "slow-pass"
url = "{slow_url}"
expect_status = ["200"]

[[requests]]
name = "fast-fail"
url = "{fast_url}"
expect_status = ["404"]
"#
        ),
    )
    .unwrap();
    let output = run_suite_file(&suite_path, &["--format", "json"]);
    slow_handle.join().unwrap();
    fast_handle.join().unwrap();
    let _ = fs::remove_file(&suite_path);
    assert_eq!(output.status.code(), Some(8));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["schema_version"], 1);
    assert_eq!(result["concurrency"], 2);
    assert_eq!(result["endpoints"][0]["name"], "slow-pass");
    assert_eq!(result["endpoints"][1]["name"], "fast-fail");
    assert_eq!(result["summary"]["passed"], 1);
    assert_eq!(result["policies"]["pass"], false);
    assert!(result["summary"]["duration_ms"].as_u64().unwrap() < 430);
}

#[test]
fn cli_can_relax_suite_failure_policy() {
    let (pass_url, pass_handle) = server(Duration::ZERO);
    let (fail_url, fail_handle) = server(Duration::ZERO);
    let suite_path = temporary_path("relaxed-suite.toml");
    fs::write(
        &suite_path,
        format!(
            r#"schema_version = 1

[[requests]]
name = "pass"
url = "{pass_url}"
expect_status = ["200"]

[[requests]]
name = "allowed-failure"
url = "{fail_url}"
expect_status = ["404"]
"#
        ),
    )
    .unwrap();
    let output = run_suite_file(
        &suite_path,
        &[
            "--format",
            "json",
            "--suite-min-success-rate",
            "50",
            "--suite-max-failures",
            "1",
        ],
    );
    pass_handle.join().unwrap();
    fail_handle.join().unwrap();
    let _ = fs::remove_file(&suite_path);
    assert_eq!(output.status.code(), Some(0));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["policies"]["pass"], true);
    assert_eq!(result["summary"]["failed"], 1);
}

#[test]
fn generates_shell_completion_and_man_page() {
    let completion = Command::new(env!("CARGO_BIN_EXE_httpstatr"))
        .args(["--generate-completion", "powershell"])
        .output()
        .unwrap();
    assert_eq!(completion.status.code(), Some(0));
    assert!(
        String::from_utf8(completion.stdout)
            .unwrap()
            .contains("Register-ArgumentCompleter")
    );

    let man = Command::new(env!("CARGO_BIN_EXE_httpstatr"))
        .arg("--generate-man")
        .output()
        .unwrap();
    assert_eq!(man.status.code(), Some(0));
    assert!(
        String::from_utf8(man.stdout)
            .unwrap()
            .contains(".TH httpstatr")
    );
}

#[test]
fn stores_queries_exports_and_prunes_history() {
    let (url, handle) = server(Duration::ZERO);
    let url = format!("{url}?api_key=history-secret&page=1");
    let path = temporary_path("history.jsonl");
    let path_text = path.to_string_lossy().into_owned();
    let output = run(
        &url,
        &[
            "--history",
            &path_text,
            "--history-name",
            "health",
            "--tag",
            "ci",
            "--commit",
            "abc123",
            "--format",
            "json",
        ],
    );
    handle.join().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stored = fs::read_to_string(&path).unwrap();
    let record: serde_json::Value = serde_json::from_str(stored.trim()).unwrap();
    assert_eq!(record["history_schema_version"], 1);
    assert_eq!(record["result"]["schema_version"], 3);
    assert!(!stored.contains("history-secret"));
    assert!(stored.contains("api_key=<redacted>"));

    let listed = Command::new(env!("CARGO_BIN_EXE_httpstatr"))
        .args([
            "history",
            "--history",
            &path_text,
            "list",
            "--tag",
            "ci",
            "--chart",
        ])
        .output()
        .unwrap();
    assert_eq!(listed.status.code(), Some(0));
    let text = String::from_utf8(listed.stdout).unwrap();
    assert!(text.contains("health"));
    assert!(text.contains("p95 |"));

    let exported = Command::new(env!("CARGO_BIN_EXE_httpstatr"))
        .args(["history", "--history", &path_text, "export"])
        .output()
        .unwrap();
    assert_eq!(exported.status.code(), Some(0));
    assert!(
        String::from_utf8(exported.stdout)
            .unwrap()
            .contains("abc123")
    );

    let pruned = Command::new(env!("CARGO_BIN_EXE_httpstatr"))
        .args(["history", "--history", &path_text, "prune", "--keep", "0"])
        .output()
        .unwrap();
    assert_eq!(pruned.status.code(), Some(0));
    assert_eq!(fs::read_to_string(&path).unwrap(), "");
    let _ = fs::remove_file(path);
}

#[test]
fn renders_openmetrics_and_har() {
    let (metrics_url, metrics_handle) = server(Duration::ZERO);
    let metrics = run(&metrics_url, &["--format", "openmetrics"]);
    metrics_handle.join().unwrap();
    assert_eq!(metrics.status.code(), Some(0));
    let metrics_text = String::from_utf8(metrics.stdout).unwrap();
    assert!(metrics_text.contains("httpstatr_request_duration_milliseconds"));
    assert!(metrics_text.ends_with("# EOF\n"));

    let (har_url, har_handle) = server(Duration::ZERO);
    let har = run(&har_url, &["--format", "har"]);
    har_handle.join().unwrap();
    assert_eq!(har.status.code(), Some(0));
    let document: serde_json::Value = serde_json::from_slice(&har.stdout).unwrap();
    assert_eq!(document["log"]["version"], "1.2");
    assert_eq!(document["log"]["entries"][0]["response"]["status"], 200);
    assert!(
        document["log"]["entries"][0]["startedDateTime"]
            .as_str()
            .is_some_and(|value| value.ends_with('Z'))
    );
}

#[test]
fn rejects_unbounded_repeat_and_warmup_counts() {
    let repeat = run("http://127.0.0.1:1/", &["--repeat", "10001"]);
    assert_eq!(repeat.status.code(), Some(2));
    assert!(
        String::from_utf8(repeat.stderr)
            .unwrap()
            .contains("between 1 and 10000")
    );

    let warmup = run("http://127.0.0.1:1/", &["--warmup", "10001"]);
    assert_eq!(warmup.status.code(), Some(2));
    assert!(
        String::from_utf8(warmup.stderr)
            .unwrap()
            .contains("between 0 and 10000")
    );
}

#[test]
fn captures_redirect_and_connection_details() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        for index in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request);
            let response = if index == 0 {
                "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            } else {
                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok"
            };
            let _ = stream.write_all(response.as_bytes());
        }
    });
    let url = format!("http://{address}/start");
    let output = run(
        &url,
        &["-L", "--format", "json", "--fail-if", "success_rate < 0%"],
    );
    handle.join().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["schema_version"], 3);
    assert_eq!(result["samples"][0]["redirects"][0]["status_code"], 302);
    assert_eq!(result["samples"][0]["connection"]["redirect_count"], 1);
}
