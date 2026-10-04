# Software development workflows

`httpstatr` helps developers turn an ad hoc HTTP request into a repeatable
check with inspectable evidence. The same request can move from local debugging
to integration testing, CI policy enforcement, release validation, and trend
analysis without changing its curl semantics.

## What value it provides

### Faster first diagnosis

A slow HTTP request can involve name resolution, connection establishment, TLS,
application processing, redirects, or transferring a large payload. The timing
breakdown helps choose the first place to investigate:

| Observation | Useful first investigation |
| --- | --- |
| High DNS time | Resolver health, DNS caching, search domains, or network path to the resolver. |
| High connection time | Routing, firewall, proxy, distance, packet loss, or service reachability. |
| High TLS time | Certificate chain, TLS termination, protocol negotiation, or network round trips. |
| High server time | Application code, dependencies, database work, queues, or server saturation. |
| High transfer time | Payload size, compression, bandwidth, buffering, or client/server streaming. |
| Redirect chain | Incorrect canonical URLs, authentication flow, routing, or avoidable hops. |
| Wide p95/p99 tail | Intermittent contention, cold paths, dependency variance, or network instability. |

These observations narrow an investigation; they do not identify a root cause
by themselves. Pair them with service logs, metrics, profiles, and traces when
server-side detail is needed.

### One check for behavior and speed

An endpoint that responds quickly with the wrong status or body is still
broken. Combining response assertions with latency policies lets one command
answer both questions:

```console
httpstatr https://api.example.com/health \
  --expect-status 200-299 \
  --expect-header "content-type:application/json" \
  --expect-body-contains '"healthy":true' \
  --slo total=500,ttfb=250
```

Distinct exit codes make the result actionable in scripts: a transport failure
is different from a response-contract failure, an SLO violation, or a
performance regression.

### Reviewable evidence

Canonical JSON records the inputs, samples, statistics, and policy outcomes.
JUnit integrates with CI test views, Markdown fits job summaries and review
comments, CSV supports offline analysis, and HAR can be opened by HTTP tooling.
The report explains why the process passed or failed instead of returning only
a generic error.

## Local endpoint debugging

Begin with one request and inspect its phase breakdown:

```console
httpstatr http://localhost:8080/api/items \
  -H "Accept: application/json"
```

Add assertions once the expected response is known:

```console
httpstatr http://localhost:8080/api/items \
  --expect-status 200 \
  --expect-header "content-type:application/json" \
  --expect-body-regex '"items"\s*:'
```

Use a repeated run when deciding whether a change affected latency:

```console
httpstatr http://localhost:8080/api/items \
  --warmup 3 --repeat 30 --delay 0.05
```

Warmups reduce the influence of one-time initialization. Percentiles show the
distribution that a single request hides. Each sample starts a new curl process
and connection, so this measures independent end-to-end requests rather than an
application connection pool.

## API contract checks during development

Keep a small command in a project task runner or development script:

```console
httpstatr "$API_URL/v1/orders/123" \
  -H "Authorization: Bearer $API_TOKEN" \
  --expect-status 200 \
  --expect-header "content-type:application/json" \
  --expect-body-regex '"id"\s*:\s*"123"' \
  --max-body-bytes 262144
```

This is useful after changing routing, serialization, authentication,
middleware, caching, or an upstream client. Secret-bearing arguments and common
query tokens are redacted from normal output, which makes the result safer to
share during review.

For comprehensive contract semantics and domain assertions, retain the
project's normal test framework. `httpstatr` is most valuable as a real HTTP
boundary check with timing evidence.

## Pull-request performance checks

Create a baseline from the same runner and test environment used by CI:

```console
httpstatr "$PREVIEW_URL/api/search?q=sample" \
  --warmup 5 --repeat 50 \
  --save ci/search-baseline.json
```

Evaluate the pull request against it:

```console
httpstatr "$PREVIEW_URL/api/search?q=sample" \
  --warmup 5 --repeat 50 \
  --compare ci/search-baseline.json \
  --fail-if "p95.total > baseline*1.15" \
  --fail-if "mean.server > 300ms" \
  --fail-if "error_rate > 0%" \
  --format junit --save current.json > httpstatr.xml
```

Publish `httpstatr.xml` through the CI system's JUnit report feature and keep
the current JSON result as a job artifact when a detailed investigation may be
needed.

Performance gates need controlled conditions. Keep region, runner class,
target data, deployment size, curl arguments, and sample count consistent.
Choose thresholds that tolerate normal variation while still detecting a
meaningful regression.

## Integration and smoke-test suites

Store a suite such as `tests/httpstatr.toml` in the repository:

```toml
schema_version = 1
concurrency = 3

[defaults]
repeat = 3
timeout = 10
expect_status = ["200-299"]
slo = "total=1000"

[policies]
min_success_rate = 100
max_failures = 0

[[requests]]
name = "health"
url = "https://staging.example.com/health"
expect_body_contains = ["healthy"]

[[requests]]
name = "catalog"
url = "https://staging.example.com/api/catalog"
headers = ["Accept: application/json"]
expect_header = ["content-type:application/json"]
```

Run it locally with readable output:

```console
httpstatr --file tests/httpstatr.toml
```

Run the same file in CI:

```console
httpstatr --file tests/httpstatr.toml \
  --format junit > httpstatr-suite.xml
```

Suites are useful after deploying a preview, staging, or production release.
They validate multiple externally visible paths and return one suite-level
policy result while preserving each endpoint's detailed failure.

Do not put live credentials directly in a committed suite. Generate a protected
suite in CI or use curl options that read sensitive material from a secured
file.

## Release validation

Measure before and after deployment from the same environment:

```console
# Before deployment
httpstatr "$SERVICE_URL/health" \
  --warmup 3 --repeat 30 --save before-release.json

# After deployment
httpstatr "$SERVICE_URL/health" \
  --warmup 3 --repeat 30 \
  --compare before-release.json \
  --fail-if "p95.total > baseline*1.20" \
  --format markdown --save after-release.json > release-check.md
```

Archive the baseline, current JSON, and Markdown report with the release. They
provide a reproducible record of the request, sample distribution, and policy
decision.

For several critical paths, use a suite and configure suite-wide success
requirements. A failed release check should prompt investigation or rollback
according to the team's deployment policy; `httpstatr` does not make that
decision itself.

## Tracking changes across builds

Record a tagged result from a stable runner:

```console
httpstatr "$SERVICE_URL/health" \
  --repeat 20 \
  --history .httpstatr/history.jsonl \
  --history-name health \
  --tag staging \
  --commit "$BUILD_COMMIT"
```

Review recent measurements:

```console
httpstatr history --history .httpstatr/history.jsonl list \
  --name health --tag staging --last 30 --chart
```

This lightweight history can show which commit or build coincided with a
latency shift. It remains a local file and does not replace production metrics,
retention policies, alerting, or distributed tracing.

## Incident investigation

Capture an aggregate result instead of relying on one failing request:

```console
httpstatr "$AFFECTED_URL" \
  -L \
  --connect-timeout 3 --timeout 20 \
  --repeat 20 \
  --format json --save incident-http.json
```

The result can preserve intermittent transport errors, response codes, timing
tails, redirect chains, effective URL, protocol, addresses, and deterministic
diagnostics. Compare the evidence with a known-good baseline or a measurement
from another network location to narrow the investigation.

Normal redaction protects common credentials and tokens, but inspect every
artifact before sharing it outside the team. Do not use `--show-secrets` for an
incident artifact.

## Choosing the right tool boundary

`httpstatr` is a good fit for:

- developer-driven endpoint diagnosis;
- lightweight API boundary and smoke checks;
- repeated end-to-end latency samples;
- performance regression gates;
- deployment verification;
- portable CI and investigation artifacts.

Use complementary tools when the goal is:

- sustained or highly concurrent load generation;
- browser rendering and frontend performance analysis;
- server CPU or memory profiling;
- distributed tracing across internal services;
- production monitoring, alerting, or long-term centralized retention;
- exhaustive application-domain contract testing.

This boundary keeps the tool useful and predictable: it measures real curl
requests, explains the client-visible timing, and turns configured expectations
into a clear result.
