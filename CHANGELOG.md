# Changelog

All notable changes to this project are documented here. The project follows
[Semantic Versioning](https://semver.org/).

## [1.0.0] - 2026-10-05

First stable release for Windows x86-64. Includes the release-candidate feature
set and the following production-readiness improvements.

### Added

- `doctor` checks curl, temporary storage, proxy configuration, and certificate
  path configuration without sending requests or displaying proxy credentials.
- JSON Pointer equality and type assertions through `--expect-json`.
- `--validate` checks every suite endpoint, baseline, assertion, and regression
  rule before execution, without requiring curl or network access.
- Suite environment profiles and explicit `${env:VARIABLE}` references.
- `--run-timeout` bounds the whole run; `--max-download-bytes` bounds response
  downloads. Default request and connect timeouts are 60 and 10 seconds.
- Measurement environment metadata and small-sample tail-percentile guidance.

### Fixed

- Attached credential arguments, attached authorization headers, encoded query
  parameter names, and proxy headers are redacted consistently.
- Active requests and delays respond to cancellation; batch reports retain
  completed samples and use exit code 130 on interruption.
- TLS phase timing uses curl's handshake metric, excluding protocol preparation.
- Repeated header values are preserved and relative redirect URLs are resolved.
- History uses cross-process locks, bounded streaming reads, atomic prune/export,
  interrupted-replacement recovery, and safe import/export source handling.
- Saved result files are replaced atomically; input and diagnostic sizes are bounded.
- HAR preserves repeated headers, identifies HTTP versions and common request
  methods, and accounts for protocol preparation in its timing totals.
- Release automation validates source and dependencies, tests Windows builds,
  smoke-tests archives, publishes checksums and provenance,
  and distinguishes stable releases from prereleases.

### Compatibility

- Implicit curlrc files, curl config files, multiple-transfer modes, and curl
  output/limit overrides are disabled. Use the tool's limit flags and one URL
  per request. Value-taking short options must be passed separately.
- Suite files are limited to 4 MiB, 256 endpoints, and 100,000 total measured
  plus warmup requests. Baselines and history records are limited to 64 MiB.
- Repeated values in response header maps are separated by newlines; header
  assertions match any individual value. Existing JSON v1 fields are retained.
- TLS phase values are corrected. The legacy cumulative `tls` SLO still uses
  the pretransfer milestone; use aggregate `tls` regression rules for handshake cost.

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

[1.0.0]: https://github.com/spanforge/httpstatr/releases/tag/v1.0.0
[1.0.0-rc.1]: https://github.com/spanforge/httpstatr/releases/tag/1.0.0.rc1
