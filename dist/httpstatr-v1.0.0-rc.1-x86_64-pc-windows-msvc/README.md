# httpstatr

[![CI](https://github.com/spanforge/httpstatr/actions/workflows/ci.yml/badge.svg)](https://github.com/spanforge/httpstatr/actions/workflows/ci.yml)

`httpstatr` is a command-line HTTP timing and policy tool powered by curl. It
shows where a request spends its time, validates the response, repeats requests
for stable statistics, compares results with a baseline, and produces reports
for people, CI systems, and monitoring tools.

It is a Rust rewrite of [reorx/httpstat](https://github.com/reorx/httpstat).
The familiar single-request workflow remains, with additional assertions,
percentiles, regression gates, endpoint suites, history, diagnostics, HAR,
OpenMetrics, JUnit, CSV, and Markdown output.

## Highlights

- Visual timing breakdown for DNS, TCP connection, TLS, server processing,
  content transfer, and total time.
- Arbitrary curl option forwarding for authentication, proxies, client
  certificates, HTTP versions, request methods, and request bodies.
- Status, header, body-size, body-text, and body-regex assertions.
- Latency SLOs with process exit codes suitable for CI.
- Repeated measurements with warmups, delays, percentiles, standard deviation,
  pass rate, and throughput.
- Saved baselines and absolute or relative performance-regression rules.
- Concurrent, versioned TOML suites for testing multiple endpoints.
- Local JSONL history with filters, trend charts, import, export, and pruning.
- Pretty, JSON, JSONL, CSV, JUnit XML, Markdown, OpenMetrics, and HAR output.
- Redaction of credentials, cookies, tokens, sensitive headers, and common
  secret-bearing query parameters.
- Shell completion and man-page generation.

## Value in software development

`httpstatr` turns an HTTP timing check into evidence that developers can use
throughout the delivery cycle:

| Development task | How `httpstatr` helps | Practical value |
| --- | --- | --- |
| Debug a slow endpoint | Separates DNS, connection, TLS, server, and transfer time | Narrows the first investigation to networking, application work, or payload transfer. |
| Verify an API change | Checks status, headers, body shape, size, and latency together | Catches functional and performance failures with one reproducible command. |
| Review a pull request | Compares repeated measurements with a saved baseline | Makes a latency regression visible before merge. |
| Run integration smoke tests | Executes named endpoint suites with shared policies | Validates several service paths without building a custom test runner. |
| Enforce CI quality gates | Returns distinct exit codes and writes JUnit or JSON results | Fails the pipeline for transport, contract, SLO, regression, or suite-policy problems. |
| Validate a release | Records repeatable pre- and post-deployment measurements | Provides an artifact that explains whether behavior or latency changed. |
| Investigate an incident | Captures connection details, redirects, percentiles, failures, and diagnostics | Preserves evidence that can be shared without exposing common secrets. |
| Watch engineering trends | Stores redacted local history with names, tags, and commit identifiers | Connects latency changes to builds, environments, or source revisions. |

It is especially useful between a one-off curl command and a full load-testing
or observability platform. It makes developer checks repeatable and
machine-readable while keeping curl's request behavior. It does not generate
realistic concurrent load, trace server internals, or prove a root cause.

See [Software development workflows](docs/development-workflows.md) for local,
CI, release, integration, and incident-response examples.

## Compared with the original httpstat

`httpstatr` preserves the original project's curl-based request model and
timing visualization, then extends that workflow for repeatable testing and CI.

| Area | Original Python `httpstat` | `httpstatr` |
| --- | --- | --- |
| Runtime | Single Python 3 script plus curl | Compiled Rust executable plus curl |
| Core timing view | DNS, TCP, TLS, server, transfer, and total | Compatible timing phases plus aggregate statistics and diagnostics |
| Curl forwarding | Yes, except reserved output controls | Yes, with the same reserved output controls |
| Structured results | JSON/JSONL schema v1 | Compatible schema v1 plus aggregate schema v3 |
| Correctness checks | Latency SLOs | SLOs plus status, header, size, text, and regex assertions |
| Repeated analysis | One request per invocation | Warmups, repeats, delays, percentiles, deviation, and throughput |
| Automation | Saved JSON and SLO exit code | Baselines, regression gates, suites, history, and CI reports |
| Export formats | Pretty, JSON, and JSONL | Pretty, JSON, JSONL, CSV, JUnit, Markdown, OpenMetrics, and HAR |
| Sensitive output | Standard curl/debug behavior | Automatic redaction with an explicit local override |

The original remains attractive when a small, directly inspectable Python
script and its existing pip/Homebrew workflow are the priority. `httpstatr` is
aimed at users who also need statistical sampling, response policies, release
binaries, and CI artifacts. It is a functional extension rather than a
byte-for-byte drop-in replacement, and both tools continue to require curl.

Read the [detailed comparison](docs/comparison-with-httpstat.md) for retained
compatibility, added features, intentional differences, and migration examples.

## Requirements

The executable invokes **curl 7.50.0 or newer** and verifies that the selected
curl supports HTTP before making a request. Curl must be available on `PATH`,
or supplied with `--curl-bin` or `HTTPSTAT_CURL_BIN`. Windows 10 and 11 normally
include `curl.exe`.

Rust **1.85 or newer** is required only when building from source.

## Installation

Install the release candidate with Cargo:

```console
cargo install httpstatr --version 1.0.0-rc.1 --locked
```

Or build the repository:

```console
cargo build --release --locked
```

The executable is written to `target/release/httpstatr` on Linux and macOS, or
`target/release/httpstatr.exe` on Windows.

Tagged releases contain archives for Linux x86-64, Windows x86-64, macOS
x86-64, and Apple Silicon. Download an archive and `SHA256SUMS` from the
[GitHub releases page](https://github.com/spanforge/httpstatr/releases), verify
the checksum, and put the executable on `PATH`.

```console
# Linux or macOS
sha256sum -c SHA256SUMS --ignore-missing
tar -xzf httpstatr-v1.0.0-rc.1-TARGET.tar.gz
install httpstatr-v1.0.0-rc.1-TARGET/httpstatr ~/.local/bin/httpstatr
```

```powershell
# Windows: compare this hash with SHA256SUMS before extracting
Get-FileHash .\httpstatr-v1.0.0-rc.1-x86_64-pc-windows-msvc.zip -Algorithm SHA256
Expand-Archive .\httpstatr-v1.0.0-rc.1-x86_64-pc-windows-msvc.zip
```

Verify the installation:

```console
httpstatr --version
curl --version
```

## Quick start

Measure one request:

```console
httpstatr https://httpbin.org/get
```

Forward curl options after the URL:

```console
httpstatr https://httpbin.org/post \
  -X POST \
  -H "Content-Type: application/json" \
  --data '{"name":"example"}'
```

`httpstatr` options can appear before or after the URL. Options it does not
recognize are passed to curl. The curl output-control options `-w`/`--write-out`,
`-D`/`--dump-header`, `-o`/`--output`, `-s`/`--silent`, and
`-S`/`--show-error` are reserved because `httpstatr` uses them to collect the
response and timing data.

Validate an API response:

```console
httpstatr https://api.example.com/health \
  --expect-status 200-299 \
  --expect-header "content-type:application/json" \
  --expect-body-contains '"status":"healthy"' \
  --slo total=500,ttfb=300
```

Collect a stable sample and save it as a baseline:

```console
httpstatr https://api.example.com/health \
  --warmup 3 --repeat 30 --delay 0.1 \
  --save baseline.json
```

Fail a later run when latency or errors regress:

```console
httpstatr https://api.example.com/health \
  --repeat 30 \
  --compare baseline.json \
  --fail-if "p95.total > baseline*1.10" \
  --fail-if "error_rate > 1%"
```

Run a multi-endpoint suite:

```console
httpstatr --file examples/httpstatr-suite.toml --concurrency 4
```

## Timing model

The visual phases are derived from curl's cumulative timestamps:

| Phase | Meaning |
| --- | --- |
| `dns` | Start until name lookup completes. |
| `connect` | DNS completion until the TCP connection completes. |
| `tls` | TCP connection completion until pre-transfer setup completes. For plain HTTP this is normally zero or near zero. |
| `server` | Pre-transfer completion until the first response byte. |
| `transfer` | First response byte until the transfer completes. |
| `total` | Complete request duration. |

Structured results also retain curl's cumulative `namelookup`,
`initial_connect`, `pretransfer`, and `starttransfer` values. Redirect metadata,
the effective URL, HTTP version, local and remote addresses, and connection
reuse information are included when curl exposes them.

## Assertions and policies

Response assertions can be repeated:

```console
httpstatr https://api.example.com/items \
  --expect-status 200,201,204 \
  --expect-header content-type \
  --expect-header "cache-control:no-cache" \
  --min-body-bytes 10 \
  --max-body-bytes 1048576 \
  --expect-body-contains '"items"' \
  --expect-body-regex '"count"\s*:\s*[0-9]+'
```

SLO checks accept `total`, `connect`, `ttfb`, `dns`, and `tls`, with integer
thresholds in milliseconds:

```console
httpstatr https://api.example.com --slo total=500,connect=100,ttfb=250
```

Assertions return exit code `5`; SLO violations return `4`. Body-content
assertions read at most 16 MiB into memory. See
[Requests and assertions](docs/requests-and-assertions.md) for exact matching
rules, timeout behavior, curl forwarding, and secret handling.

## Repeated measurements and regressions

```console
httpstatr https://api.example.com --warmup 5 --repeat 100 --delay 0.25
```

Aggregate output reports minimum, maximum, mean, median, p90, p95, p99, and
population standard deviation for each timing phase. Warmups are excluded from
statistics. Transport failures remain visible but do not contribute timing
values. Each run allows at most 10,000 measured requests and 10,000 warmups;
`httpstatr` is a diagnostic and policy tool, not a load generator.

Regression rules support timing metrics such as `p95.total`, rate subjects
such as `error_rate`, the operators `>`, `>=`, `<`, and `<=`, absolute
thresholds, and `baseline*FACTOR` thresholds.

See [Measurements and regression gates](docs/measurements.md) for calculation
details, compatible baselines, diagnostics, and CI examples.

## Output formats

Select output with `--format` or `-f`:

```console
httpstatr https://api.example.com --repeat 20 --format json
httpstatr https://api.example.com --repeat 20 --format jsonl
httpstatr https://api.example.com --repeat 20 --format csv > samples.csv
httpstatr https://api.example.com --repeat 20 --format junit > results.xml
httpstatr https://api.example.com --repeat 20 --format markdown > results.md
httpstatr https://api.example.com --repeat 20 --format openmetrics > metrics.txt
httpstatr https://api.example.com --repeat 20 --format har > results.har
```

`--save result.json` always writes canonical versioned JSON independently of
the display format. Single ordinary requests use JSON schema v1. Features that
need aggregate data use schema v3. Schema v2 remains accepted as a comparison
baseline. See [Output formats](docs/output-formats.md) and the
[schema v3 reference](docs/result-schema-v3.md).

## Endpoint suites

Suites are versioned TOML files with shared defaults, named requests, endpoint
overrides, assertions, SLOs, baselines, regression rules, and suite-wide pass
policies. Endpoints run concurrently while requests within one endpoint remain
sequential. Results stay in declaration order.

```toml
schema_version = 1
concurrency = 4

[defaults]
repeat = 5
warmup = 1
timeout = 10
expect_status = ["200-299"]

[policies]
min_success_rate = 100
max_failures = 0

[[requests]]
name = "health"
url = "https://api.example.com/health"
headers = ["Accept: application/json"]
expect_body_contains = ["healthy"]
slo = "total=500,ttfb=300"
```

See [Endpoint suites](docs/suites.md) for the full schema, inheritance rules,
concurrency, cancellation behavior, and output support.

## History and trends

History is opt-in and stores redacted schema v3 records in an append-only JSONL
file:

```console
httpstatr https://api.example.com/health --repeat 20 \
  --history .httpstatr/history.jsonl \
  --history-name health --tag ci --commit abc123

httpstatr history --history .httpstatr/history.jsonl list \
  --name health --tag ci --last 30 --chart
```

History supports filters by name, URL, tag, commit, and Unix time range, plus
import, export, and atomic pruning. See [History and trends](docs/history.md).

## Environment variables

| Variable | Default | Effect |
| --- | --- | --- |
| `HTTPSTAT_SHOW_BODY` | `false` | Show up to 1024 bytes of the response body in pretty output. |
| `HTTPSTAT_SHOW_IP` | `true` | Show local and remote addresses and ports. |
| `HTTPSTAT_SHOW_SPEED` | `false` | Show upload and download speed. |
| `HTTPSTAT_SAVE_BODY` | `true` | Retain the temporary response body in pretty mode. |
| `HTTPSTAT_CURL_BIN` | `curl` or `curl.exe` | Select the curl executable. |
| `HTTPSTAT_METRICS_ONLY` | `false` | Change default pretty output to JSON for httpstat compatibility. |
| `HTTPSTAT_DEBUG` | `false` | Print configuration and the redacted curl command. |
| `NO_COLOR` | unset | Disable ANSI color when present. |

Boolean variables accept `1`, `true`, `yes`, or `on`, and `0`, `false`, `no`,
or `off`, case-insensitively. CLI `--curl-bin` takes precedence over
`HTTPSTAT_CURL_BIN`.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Requests and configured policies passed. |
| `1` | Configuration, file, or local processing error. |
| `2` | Invalid command-line usage. |
| `4` | At least one latency SLO failed. |
| `5` | At least one response assertion failed. |
| `6` | A measured request had a curl transport failure. |
| `7` | A regression rule was triggered. |
| `8` | An endpoint suite failed its suite-level policy. |
| `130` | An endpoint suite was interrupted with Ctrl-C. |
| other | A single request failed and curl's exit code was preserved. |

For aggregate runs, precedence is transport failure, assertion failure, SLO
violation, regression, then success. Structured results retain all failure
details even when a higher-priority category determines the process exit code.

## Completions and man page

```console
httpstatr --generate-completion bash > httpstatr.bash
httpstatr --generate-completion zsh > _httpstatr
httpstatr --generate-completion fish > httpstatr.fish
httpstatr --generate-completion powershell > _httpstatr.ps1
httpstatr --generate-man > httpstatr.1
```

Installation examples are in the [CLI reference](docs/cli-reference.md).

## Documentation

The [documentation index](docs/README.md) links every user and maintainer
guide. Start with:

- [Getting started](docs/getting-started.md)
- [Software development workflows](docs/development-workflows.md)
- [CLI reference](docs/cli-reference.md)
- [Comparison with the original httpstat](docs/comparison-with-httpstat.md)
- [Requests and assertions](docs/requests-and-assertions.md)
- [Measurements and regression gates](docs/measurements.md)
- [Output formats](docs/output-formats.md)
- [Endpoint suites](docs/suites.md)
- [History and trends](docs/history.md)
- [Troubleshooting](docs/troubleshooting.md)

Sensitive values are redacted by default. `--show-secrets` should be used only
for intentional local debugging and cannot be combined with history. Report
security issues through the process in [SECURITY.md](SECURITY.md).

## Development and releases

```console
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
cargo build --release --locked
```

Maintainers should follow [the release process](docs/releasing.md). User-visible
changes are recorded in [CHANGELOG.md](CHANGELOG.md). The transport and
telemetry decisions are documented in
[Architecture decisions](docs/version-3-decisions.md).

This port is based on the behavior and documentation of `reorx/httpstat`
2.0.0. The upstream project is MIT licensed; see [NOTICE](NOTICE).
