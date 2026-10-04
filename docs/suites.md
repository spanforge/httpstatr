# Endpoint suites

`httpstatr --file FILE` runs multiple named endpoints from a versioned TOML
document. Suite schema version `1` supports shared request defaults, endpoint
overrides, bounded concurrency, endpoint-level checks, and suite-level pass
policies.

Suites are useful for integration smoke tests, deployment verification, and
service-level contract checks. They keep request definitions and pass criteria
in version control, while JUnit, Markdown, and JSON output make the same suite
usable locally and in CI. See
[Software development workflows](development-workflows.md) for lifecycle
examples.

## Complete example

```toml
schema_version = 1
concurrency = 4

[defaults]
repeat = 5
warmup = 1
delay = 0.1
connect_timeout = 2
timeout = 10
slo = "total=750"
expect_status = ["200-299"]
expect_header = ["content-type"]
curl_args = ["--http2"]

[policies]
min_success_rate = 100
max_failures = 0

[[requests]]
name = "health"
url = "https://api.example.com/health"
method = "GET"
headers = ["Accept: application/json"]
expect_body_contains = ["healthy"]
slo = "total=500,ttfb=300"

[[requests]]
name = "create-item"
url = "https://api.example.com/items"
method = "POST"
headers = ["Content-Type: application/json"]
body = '{"name":"example"}'
expect_status = ["201"]
min_body_bytes = 10
repeat = 3
fail_if = ["p95.total > 750ms"]
```

The repository includes this starting point at
[`examples/httpstatr-suite.toml`](../examples/httpstatr-suite.toml).

Run a suite:

```console
httpstatr --file examples/httpstatr-suite.toml
httpstatr --file suite.toml --concurrency 8 --format json
```

The URL positional argument and `--file` are mutually exclusive.

## Top-level schema

| Key | Required | Meaning |
| --- | --- | --- |
| `schema_version` | Yes | Must be `1`. |
| `concurrency` | No | Concurrent endpoint workers, `1..64`; default `4`. |
| `[defaults]` | No | Request settings inherited by every endpoint. |
| `[policies]` | No | Suite-wide endpoint pass policy. |
| `[[requests]]` | Yes | One or more uniquely named endpoints. |

Unknown keys are rejected. Request names must be nonempty and unique, and URLs
must be nonempty.

## Shared and endpoint settings

The `[defaults]` table and each `[[requests]]` entry accept:

| Key | Type | Meaning |
| --- | --- | --- |
| `repeat` | integer `1..10000` | Measured requests. |
| `warmup` | integer `0..10000` | Unmeasured warmups. |
| `delay` | nonnegative number | Seconds between requests. |
| `connect_timeout` | positive number | Curl connection timeout in seconds. |
| `timeout` | positive number | Curl total timeout in seconds. |
| `slo` | string | Comma-separated latency SLOs. |
| `expect_status` | string array | Statuses, ranges, or comma lists. |
| `expect_header` | string array | Required names or exact `NAME:VALUE` pairs. |
| `min_body_bytes` | integer | Minimum response size. |
| `max_body_bytes` | integer | Maximum response size. |
| `expect_body_contains` | string array | Required body fragments. |
| `expect_body_regex` | string array | Required regular-expression matches. |
| `compare` | path | Schema v2/v3 baseline. |
| `fail_if` | string array | Regression expressions. |
| `curl_args` | string array | Additional curl arguments. |

An endpoint also accepts:

| Key | Required | Meaning |
| --- | --- | --- |
| `name` | Yes | Unique result identifier. |
| `url` | Yes | Request URL. |
| `method` | No | Appended as curl `-X METHOD`. |
| `headers` | No | Appended as repeated curl `-H` arguments. |
| `body` | No | Inline body appended as curl `--data-binary`. |

Scalar endpoint settings override defaults. Array settings from defaults and
the endpoint are concatenated, with defaults first. Consequently, assertions,
regression rules, and curl arguments can be extended per endpoint.

`method`, `headers`, and `body` exist only on endpoint entries. Relative
`compare` paths are resolved relative to the suite file, allowing a portable
suite and baseline directory.

## Requests and bodies

For a JSON request:

```toml
[[requests]]
name = "create"
url = "https://api.example.com/items"
method = "POST"
headers = [
  "Authorization: Bearer example-token",
  "Content-Type: application/json",
]
body = '''
{"name":"sample","enabled":true}
'''
expect_status = ["201"]
```

Inline bodies and sensitive headers are redacted from structured configuration
metadata and debug output. Suite files themselves are plain text, so do not
commit real credentials. Prefer curl arguments that read credentials from a
protected file or inject a generated suite securely in CI.

## Endpoint policies

All single-URL features that make sense in a suite can be set in defaults or on
an endpoint:

```toml
[defaults]
expect_status = ["200-299"]
expect_header = ["content-type:application/json"]
slo = "total=750"

[[requests]]
name = "health"
url = "https://api.example.com/health"
expect_body_contains = ["healthy"]
expect_body_regex = ['"version"\s*:\s*"[0-9.]+"']
min_body_bytes = 20
max_body_bytes = 65536
compare = "baselines/health.json"
fail_if = [
  "p95.total > baseline*1.15",
  "error_rate > 0%",
]
```

Each endpoint produces a schema v3 aggregate result and keeps its own
transport, assertion, SLO, and regression exit code.

## Concurrency

The file value can be overridden on the command line:

```console
httpstatr --file suite.toml --concurrency 8
```

Concurrency is limited to 64 and defaults to 4. Workers take endpoints from a
bounded queue. Endpoints execute in parallel, while repeats within an endpoint
execute sequentially. Output remains in TOML declaration order regardless of
completion order.

Use concurrency carefully against production APIs. A suite with concurrency
`N` can have up to `N` curl requests in flight, while each endpoint still
honors its configured delay.

## Suite policies

By default, every endpoint must pass. Relax this with `[policies]`:

```toml
[policies]
min_success_rate = 90
max_failures = 2
```

Both constraints must pass. `min_success_rate` ranges from 0 through 100;
`max_failures` is a nonnegative count. CLI values override the TOML values:

```console
httpstatr --file suite.toml \
  --suite-min-success-rate 95 \
  --suite-max-failures 1
```

A suite policy failure returns code `8`. The suite document records policy
thresholds, violations, pass/fail counts, endpoint success rate, total samples,
and duration.

## Cancellation

When Ctrl-C is received, workers stop starting queued endpoints. Requests
already in progress finish, completed endpoint results are retained, and
queued endpoints are marked canceled. The suite result sets `canceled` to
`true` and returns exit code `130`.

## Output

Suites support:

```text
pretty json jsonl csv junit markdown
```

OpenMetrics and HAR support one URL and are rejected before a suite starts.
Suite JSON has top-level schema version `1`; each completed endpoint embeds its
schema v3 result. CSV, JUnit, and Markdown combine endpoint results in
declaration order.

## Validation checklist

Before committing a suite:

1. run it with `--format json` to inspect effective results;
2. confirm endpoint names are unique and stable;
3. keep repeat, warmup, delay, and concurrency safe for the target service;
4. make relative baselines part of the same portable directory layout;
5. ensure the file does not contain credentials;
6. test suite policy thresholds against both passing and intentionally failing
   endpoints.
