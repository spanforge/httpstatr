# Changelog

All notable changes to this project are documented here. The project follows
[Semantic Versioning](https://semver.org/).

## [1.0.0-rc.1] - 2026-10-04

First public release candidate.

### Added

- Curl-compatible single-request HTTP timing visualization.
- Repeated measurements, warmups, latency distributions, and response checks.
- Baseline comparison and deterministic performance-regression policies.
- Versioned TOML endpoint suites with bounded concurrency and suite policies.
- JSON, JSONL, CSV, JUnit, Markdown, OpenMetrics, and HAR output.
- Redacted JSONL history with filters, trend charts, import, export, and pruning.
- Redirect, connection, and evidence-based diagnostic information.
- Shell-completion and man-page generation.

### Security and reliability

- Automatic redaction for credentials, sensitive headers, request data, and
  common secret-bearing URL query parameters.
- Limits of 10,000 measured requests and 10,000 warmups per endpoint.
- A 16 MiB in-memory limit for response-body content assertions.
- Curl 7.50.0 capability validation before executing requests.

[1.0.0-rc.1]: https://github.com/spanforge/httpstatr/releases/tag/v1.0.0-rc.1
