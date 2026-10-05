# Legacy aggregate result schema v2

Schema v2 was the earlier repeated-run document. `httpstatr` 1.0.0 emits
schema v3 for aggregate work, but still deserializes schema v2 baselines for
`--compare`. This reference exists for stored-result compatibility.

New consumers should implement [schema v3](result-schema-v3.md). The v3 schema
keeps the v2 configuration, summary, aggregate, samples, comparison, and
regression fields and adds request start timestamps, curl version, connection
details, redirects, and diagnostics.

## Top-level fields

| Field | Meaning |
| --- | --- |
| `schema_version` | `2`. |
| `url` | Requested URL, normally redacted. |
| `ok` | Whether measured samples and configured policies passed. |
| `exit_code` | Aggregate process result. |
| `warmup_count` | Requested warmups. |
| `warmup_failures` | Warmup transport failures. |
| `configuration` | Repeat settings, redacted curl arguments, and policies. |
| `summary` | Counts, success rate, wall duration, and throughput. |
| `aggregate` | Timing statistics calculated from completed responses. |
| `samples` | Every measured request in execution order. |
| `comparison` | Optional baseline timing deltas. |
| `regression` | Optional evaluated regression policies. |

`summary.success_rate` is a percentage from 0 through 100. A sample passes only
when transport, assertions, and SLOs pass. `transport_successful` counts
responses received even when a policy fails.

## Timing statistics

Each phase uses:

```json
{
  "count": 100,
  "min": 12.0,
  "max": 48.0,
  "mean": 20.31,
  "median": 19.0,
  "p90": 25.0,
  "p95": 29.0,
  "p99": 41.0,
  "stddev": 5.42
}
```

Values are milliseconds. Mean and population standard deviation are rounded to
two decimals. Median and percentiles use nearest rank. Transport failures are
excluded. Empty aggregates contain count and numeric values of zero.

## Baseline compatibility

When schema v2 is supplied with `--compare`, the current command requires:

- at least one successful total timing;
- a URL equal to the current redacted URL;
- a redacted curl argument array equal to the current request.

Missing v3-only fields receive defaults. Repeat counts can differ. The current
comparison and any newly saved result are emitted using schema v3.

## Legacy exit precedence

The aggregate precedence remains unchanged in schema v3:

1. transport failure: `6`;
2. assertion failure: `5`;
3. SLO violation: `4`;
4. regression failure: `7`;
5. success: `0`.
