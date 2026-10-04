# Aggregate result schema v3

Schema v3 is the current aggregate document. It is produced by repeated runs,
warmups, comparisons, history, advanced reports, and endpoint-suite entries.
An ordinary single request remains schema v1 for compatibility.

## Top-level fields

| Field | Type | Meaning |
| --- | --- | --- |
| `schema_version` | integer | Always `3`. |
| `url` | string | Requested URL, redacted by default. |
| `ok` | boolean | Whether samples and regression rules passed. |
| `exit_code` | integer | Aggregate exit code using the documented precedence. |
| `warmup_count` | integer | Number of requested warmups. |
| `warmup_failures` | integer | Warmup curl transport failures. |
| `configuration` | object | Effective repeat settings and policy configuration. |
| `summary` | object | Counts, success rate, duration, and throughput. |
| `aggregate` | object | Statistics for every timing phase. |
| `samples` | array | Every measured request in execution order. |
| `comparison` | object, optional | Deltas from a supplied schema v2/v3 baseline. |
| `regression` | object, optional | Evaluated `--fail-if` rules. |
| `diagnostics` | array, optional | Deterministic performance observations. |

## Configuration

`configuration` records the inputs needed to interpret a measurement:

| Field | Meaning |
| --- | --- |
| `repeat` | Measured request count. |
| `warmup` | Warmup count. |
| `delay_seconds` | Delay between requests. |
| `connect_timeout_seconds` | Optional connection timeout. |
| `timeout_seconds` | Optional total timeout. |
| `curl_args` | Forwarded curl arguments, redacted by default. |
| `curl_version` | Validated curl runtime version when available. |
| `slo` | Optional SLO source string. |
| `expect_status`, `expect_header` | Response assertions. |
| `min_body_bytes`, `max_body_bytes` | Optional size bounds. |
| `expect_body_contains`, `expect_body_regex` | Body assertions. |
| `comparison_baseline` | Optional baseline path. |
| `regression_rules` | Original rule expressions. |

This metadata describes the run; a result is not an executable configuration.

## Summary

| Field | Meaning |
| --- | --- |
| `requested` | Measured sample count requested. |
| `transport_successful` | Samples that received a response. |
| `passed` | Samples whose transport, assertion, and SLO checks passed. |
| `failed` | Requested minus passed. |
| `success_rate` | Passed/requested as a percentage. |
| `duration_ms` | Measured run wall time, including process startup and delays. |
| `requests_per_second` | Requested samples divided by measured wall time. |

Warmups do not contribute to this summary.

## Aggregate timings

The `aggregate` object has `dns`, `connect`, `tls`, `server`, `transfer`,
`total`, `namelookup`, `initial_connect`, `pretransfer`, and `starttransfer`.
Each contains:

```json
{
  "count": 20,
  "min": 42.0,
  "max": 91.0,
  "mean": 55.35,
  "median": 53.0,
  "p90": 64.0,
  "p95": 72.0,
  "p99": 91.0,
  "stddev": 10.24
}
```

Values are milliseconds. Percentiles use deterministic nearest rank and
`stddev` is the population standard deviation. Transport failures do not
contribute timing values. Completed responses contribute even when assertions
or SLOs fail.

## Samples

Every sample contains:

| Field | Meaning |
| --- | --- |
| `index` | One-based measurement order. |
| `started_at_unix_ms` | Start time in Unix milliseconds, used by HAR. |
| `ok` | Whether transport and configured response policies passed. |
| `exit_code` | Sample result code. |

A completed response may also include `response`, `timings_ms`, `speed`,
`response_size_bytes`, `slo`, `assertions`, `redirects`, and `connection`. A
transport failure instead includes `error` and omits response/timing details.

### Redirects

Each redirect has an ordered index, status, source URL, and destination URL.
Sensitive URL components are redacted unless secrets were explicitly enabled.

### Connection details

Connection details contain local and remote addresses, negotiated HTTP version,
effective URL, connection count, inferred reuse, redirect count, and cumulative
redirect time. Curl exposes final connection values and cumulative redirect
time, not complete timings for every hop.

## Comparison and regression

`comparison` stores current, baseline, absolute delta, and percentage delta for
aggregate metrics. A zero baseline has a `null` percentage delta.

`regression` stores an overall pass flag and every rule's subject, operator,
target, actual value, and result. This lets CI reports explain code `7` without
re-evaluating the expression.

## Diagnostics

Each diagnostic contains:

- stable `code`;
- severity-like `level`;
- human-readable `observation`;
- measured `evidence`;
- suggested `investigation`.

Diagnostics do not affect `ok` or `exit_code`, and absence of a diagnostic is
not proof that a component is healthy.

## Exit-code precedence

1. `6` for one or more measured transport failures;
2. `5` for one or more assertion failures;
3. `4` for one or more SLO violations;
4. `7` for a triggered regression rule;
5. `0` when all samples and rules pass.

Every failure remains represented even when another category determines the
overall code.

## Compatibility

Schema v3 preserves the schema v2 core and adds optional/defaulted fields.
Schema v2 baselines remain accepted. Consumers should ignore unknown fields so
future additive fields remain compatible, and should branch on
`schema_version` before assuming aggregate fields exist.
