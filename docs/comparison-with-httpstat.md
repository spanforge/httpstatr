# Comparison with the original httpstat

`httpstatr` is a Rust rewrite and extension of
[reorx/httpstat](https://github.com/reorx/httpstat). This comparison describes
the original project's documented `master` behavior as checked on 2026-10-04
and `httpstatr` 1.0.0. The original project can evolve, so its README is
the authority for its current feature set.

The goal of `httpstatr` is to retain the familiar curl timing workflow while
supporting repeated measurements, correctness checks, regression policies,
multi-endpoint suites, and CI reporting. It is not intended to reproduce every
line of output or serve as an executable-name drop-in replacement.

## Shared foundation

Both tools:

- invoke curl for request execution;
- display DNS lookup, TCP connection, TLS handshake, server processing,
  content transfer, and total time;
- forward arbitrary curl options after the URL, except curl output controls
  required internally;
- support HTTP methods, request data, authentication, proxies, certificates,
  redirects, and protocol options through curl;
- support pretty terminal output, JSON, JSONL, saved JSON, latency SLOs, and
  `NO_COLOR`;
- expose the original `HTTPSTAT_SHOW_BODY`, `HTTPSTAT_SHOW_IP`,
  `HTTPSTAT_SHOW_SPEED`, `HTTPSTAT_SAVE_BODY`, `HTTPSTAT_CURL_BIN`,
  `HTTPSTAT_METRICS_ONLY`, and `HTTPSTAT_DEBUG` environment model;
- retain schema v1 for an ordinary single-request JSON result;
- preserve curl's transport exit code for a failed single request.

The shared curl boundary matters: a request that depends on a specific curl
authentication, proxy, TLS, or HTTP feature continues to use curl's behavior
instead of a separate Rust HTTP implementation.

## Capability comparison

| Capability | Original Python `httpstat` | `httpstatr` 1.0.0 |
| --- | --- | --- |
| Implementation | Single-file Python 3 script | Compiled Rust CLI |
| External request engine | curl | curl 7.50.0 or newer, validated at startup |
| Basic timing visualization | Yes | Yes |
| Curl option forwarding | Yes | Yes |
| Pretty terminal output | Yes | Yes |
| JSON and JSONL | Stable schema v1 | Schema v1 for ordinary single requests; schema v3 for aggregates |
| Save JSON separately from display | Yes | Yes |
| Latency SLOs | Yes | Yes |
| Status and header assertions | No documented support | Yes |
| Body size, text, and regex assertions | No documented support | Yes, with a 16 MiB content-check limit |
| Repeated requests and warmups | No documented support | Yes, bounded to 10,000 each |
| Distribution statistics | No documented support | Min, max, mean, median, p90, p95, p99, and population deviation |
| Baseline comparison | No documented support | Schema v2/v3 baselines with absolute and percentage deltas |
| Regression expressions | No documented support | Absolute, rate, and relative-to-baseline rules |
| Multi-endpoint suites | No documented support | Concurrent versioned TOML suites with shared defaults and policies |
| Historical trends | No documented support | Redacted JSONL history, filters, charts, import, export, and pruning |
| CI report formats | No documented support | JUnit XML and Markdown |
| Analysis formats | JSON/JSONL | JSON/JSONL, per-sample CSV, OpenMetrics, and HAR 1.2 |
| Redirect and connection metadata | No documented structured redirect history | Ordered redirects, effective URL, HTTP version, addresses, counts, and reuse inference |
| Automated diagnostics | Agent skill supplied separately by the original project | Deterministic observations embedded in aggregate results |
| Secret handling | No documented comprehensive redaction layer | Headers, curl arguments, URL credentials, query secrets, redirects, assertions, debug output, and history redacted by default |
| Completion and man-page generation | No documented support | Bash, Zsh, Fish, PowerShell, and roff generation |
| Windows distribution | Original README recommends a Go alternative | Native Windows x86-64 release archive |
| Release archives | Script, pip, and Homebrew installation | Cargo plus Linux, Windows, Intel macOS, and Apple Silicon archives with checksums |

“No documented support” means the capability is not presented in the original
project's current README. It does not claim that no fork, external script, or
undocumented workflow can provide it.

## Where httpstatr adds value

### Repeatable performance evidence

A single timing is useful for diagnosis but sensitive to normal network and
server variance. `httpstatr` can collect a bounded distribution:

```console
httpstatr https://api.example.com --warmup 5 --repeat 50 --delay 0.1
```

It records every measured sample and calculates deterministic percentiles and
population standard deviation. Transport failures stay visible rather than
being silently removed from the run.

### Correctness and performance in one command

The original supports latency SLOs. `httpstatr` retains them and adds response
assertions:

```console
httpstatr https://api.example.com/health \
  --expect-status 200-299 \
  --expect-header "content-type:application/json" \
  --expect-body-contains '"healthy":true' \
  --slo total=500,ttfb=250
```

Separate exit codes distinguish transport, assertion, SLO, regression, and
suite-policy failures.

### Regression gates

Saved aggregate results can become baselines:

```console
httpstatr https://api.example.com --repeat 50 --save baseline.json

httpstatr https://api.example.com --repeat 50 \
  --compare baseline.json \
  --fail-if "p95.total > baseline*1.10" \
  --fail-if "error_rate > 1%"
```

The structured result records every rule, actual value, target, and outcome so
a CI failure can be explained after the command finishes.

### Multiple endpoints

Versioned TOML suites apply common defaults and policies to named endpoints:

```console
httpstatr --file examples/httpstatr-suite.toml --concurrency 4 --format junit
```

Endpoints run with bounded concurrency, remain ordered in the result, and can
have distinct methods, headers, bodies, assertions, SLOs, and regression rules.

### Long-term and tool-friendly results

History stores redacted aggregate records locally, while CSV, JUnit, Markdown,
OpenMetrics, and HAR connect the same canonical measurement to analysis, CI,
monitoring, and browser-oriented tooling.

### Safer automation

`httpstatr` validates curl before requests, bounds repeat and body-content
processing, and redacts common secrets from structured results and diagnostic
output. The override `--show-secrets` is explicit and incompatible with
history recording.

## Where the original may be preferable

The original project has meaningful advantages for some workflows:

- it is a small Python file that is easy to download, inspect, and modify;
- it has established pip and Homebrew installation paths;
- it has a mature community and a recognizable `httpstat` command;
- it includes a separately installable agent skill;
- its smaller feature surface is easier when only one-request visualization is
  needed.

`httpstatr` has more concepts, options, result fields, and policy exit codes.
Those additions are valuable for automation but unnecessary for every user.

## Intentional differences

### Executable and runtime

The command is `httpstatr`, not `httpstat`. It is a compiled executable and
does not require Python at runtime. Curl remains required; the Rust rewrite does
not reimplement curl.

### Structured schemas

An ordinary request keeps compatible schema v1 JSON. Aggregate work uses schema
v3 because it must represent configuration, samples, statistics, comparisons,
policies, connections, redirects, and diagnostics. Schema v2 is accepted as a
legacy comparison baseline.

### Additional exit codes

Both tools use code `4` for an SLO violation and preserve curl codes for a
single transport failure. `httpstatr` adds codes for assertions (`5`), aggregate
transport failure (`6`), regressions (`7`), suite policies (`8`), and suite
cancellation (`130`). Invalid command-line usage uses `2`.

### Stricter validation and limits

`httpstatr` validates numeric input, regular expressions, suites, baselines,
curl availability, and curl version before the related work begins. Repeats,
warmups, concurrency, and body-content memory are bounded. These checks can
reject an invocation earlier than the original script would.

### Redaction

Sensitive values are redacted by default, so debug and structured output may
not be byte-for-byte equal to the input curl command. Use `--show-secrets` only
for deliberate local inspection.

## Migration examples

Common original commands retain the same shape with the executable renamed:

```console
# Original
httpstat https://httpbin.org/get
httpstat https://httpbin.org/post -X POST --data-urlencode "a=b" -v
httpstat https://httpbin.org/get --format json
httpstat https://httpbin.org/get --slo total=500,connect=100

# Rust port
httpstatr https://httpbin.org/get
httpstatr https://httpbin.org/post -X POST --data-urlencode "a=b" -v
httpstatr https://httpbin.org/get --format json
httpstatr https://httpbin.org/get --slo total=500,connect=100
```

The existing `HTTPSTAT_*` environment names remain available:

```console
HTTPSTAT_SHOW_SPEED=true HTTPSTAT_SHOW_IP=false \
  httpstatr https://httpbin.org/get
```

Before replacing a scripted use, verify its expected executable name, output
text, JSON schema, and exit-code handling. Use schema v1 for compatible
single-request JSON and adopt schema v3 deliberately for aggregate features.

## Choosing between them

Choose the original when the priority is the smallest familiar Python workflow
for visually inspecting one request. Choose `httpstatr` when the same curl
timing model needs repeatable statistics, correctness policies, comparisons,
suites, historical records, secret-aware automation, or CI-specific reports.

The [software development workflow guide](development-workflows.md) shows how
those additional capabilities fit into local development, pull requests,
releases, integration testing, and incident investigation.
