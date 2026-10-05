# Measurements and regression gates

## Repeated measurements

Use `--repeat` to collect multiple measured requests:

```console
httpstatr https://api.example.com/health --repeat 50
```

Use `--warmup` to run unmeasured requests first and `--delay` to wait between
requests:

```console
httpstatr https://api.example.com/health \
  --warmup 5 --repeat 50 --delay 0.25
```

The delay is expressed in seconds and may be fractional. Every request uses a
new curl process and connection. This provides independent end-to-end samples;
it does not measure connection-pool performance.

Aggregate configuration also records tool version, operating system, architecture,
curl version, the sampling method, and effective request timeouts. Runs with fewer
than 100 successful samples include a tail-percentile guidance diagnostic. This
is sampling guidance, not a confidence interval or a policy failure.

TLS phase timing uses `time_appconnect - time_connect`, clamped at zero for
plain HTTP. Protocol preparation between handshake completion and pretransfer
is not classified as TLS. Redirect timings remain cumulative curl measurements;
the tool does not claim independent per-hop phase timings.

Measured requests and warmups are each capped at 10,000. Warmup timings,
warmup failures, and delays following warmups do not enter aggregate
statistics. Warmup transport failures are counted separately.

## Statistics

For each timing phase, aggregate schema v3 provides:

| Statistic | Definition |
| --- | --- |
| `count` | Samples that received a response and contributed a timing value. |
| `min` | Smallest value. |
| `max` | Largest value. |
| `mean` | Arithmetic mean, rounded to two decimals. |
| `median` | 50th percentile using nearest rank. |
| `p90` | 90th percentile using nearest rank. |
| `p95` | 95th percentile using nearest rank. |
| `p99` | 99th percentile using nearest rank. |
| `stddev` | Population standard deviation, rounded to two decimals. |

Nearest rank selects `ceil(percentile * sample_count)` after sorting. Transport
failures stay in `samples` but are excluded from timing statistics. Completed
responses that fail an assertion or SLO still have valid timing data and
contribute to timing aggregates.

The summary reports requested, transport-successful, passed, and failed sample
counts, success percentage, measured wall-clock duration, and requests per
second. Duration includes curl startup and configured delays between measured
requests.

## Save a baseline

```console
httpstatr https://api.example.com/health \
  --warmup 5 --repeat 50 \
  --save baseline.json
```

`--save` writes canonical JSON regardless of the terminal format. A baseline
must be aggregate schema v2 or v3, contain at least one successful timing
sample, and match the current redacted URL and curl arguments. Repeat and
warmup counts may differ.

Compare a later measurement:

```console
httpstatr https://api.example.com/health \
  --repeat 50 --compare baseline.json
```

Comparison output includes absolute and percentage deltas for every timing
statistic and phase. Percentage change is `null` when the baseline is zero.

## Regression rules

`--fail-if` turns an aggregate into a CI performance gate and may be repeated:

```console
httpstatr https://api.example.com/health \
  --repeat 50 \
  --compare baseline.json \
  --fail-if "p95.total > baseline*1.10" \
  --fail-if "mean.server > 250ms" \
  --fail-if "success_rate < 99%" \
  --fail-if "error_rate > 1%"
```

The grammar is:

```text
SUBJECT OPERATOR THRESHOLD
```

Operators are `>`, `>=`, `<`, and `<=`.

Timing subjects use `STATISTIC.PHASE`:

- Statistics: `min`, `max`, `mean`, `median`, `p90`, `p95`, `p99`, `stddev`.
- Phases: `dns`, `connect`, `tls`, `server`, `transfer`, `total`,
  `namelookup`, `initial_connect`, `pretransfer`, `starttransfer`.
- Rate subjects: `success_rate`, `error_rate`.

Timing thresholds are milliseconds. The `ms` suffix is optional. Rate
thresholds are percentages and may use `%`. A relative threshold has the form
`baseline` or `baseline*FACTOR` and requires `--compare`:

```console
--fail-if "p99.total >= baseline*1.25"
```

A triggered rule returns exit code `7` when all requests, assertions, and SLOs
otherwise pass. Every rule and its evaluated values remain in the structured
result.

## Diagnostics

Aggregate schema v3 includes deterministic observations about:

- large DNS, TLS, server, or transfer shares;
- wide latency tails;
- redirect chains.

Each diagnostic contains a stable code, level, observation, measured evidence,
and suggested investigation. Diagnostics explain measurements and do not claim
a root cause. They do not change the process exit code.

## CI examples

Repeated measurements give a pull request or release pipeline evidence about a
distribution rather than one potentially lucky request. Baseline rules are
most useful when the runner location, target environment, request options, and
sample count stay consistent. For a complete progression from local diagnosis
to release validation, see
[Software development workflows](development-workflows.md).

Create a JUnit report while enforcing correctness and latency:

```console
httpstatr "$HEALTH_URL" \
  --repeat 20 \
  --expect-status 200-299 \
  --slo total=750 \
  --fail-if "p95.total > 500ms" \
  --format junit > httpstatr.xml
```

Compare with a version-controlled or downloaded baseline:

```console
httpstatr "$HEALTH_URL" \
  --repeat 50 \
  --compare ci/baseline.json \
  --fail-if "p95.total > baseline*1.15" \
  --fail-if "error_rate > 0%" \
  --format json --save current.json
```

Keep baseline environment, region, curl arguments, authentication mode, and
network path consistent. Network latency naturally varies, so use enough
samples and a tolerance appropriate to the environment.

## Aggregate exit precedence

When several failure categories occur, the process uses:

1. transport failure: `6`;
2. assertion failure: `5`;
3. SLO violation: `4`;
4. regression: `7`;
5. success: `0`.

The lower-precedence failures are still recorded in the samples and policy
results.
