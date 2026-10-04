# Output formats

Choose a display renderer with `--format FORMAT` or `-f FORMAT`. `--save PATH`
is independent of the renderer and always writes canonical versioned JSON.

The formats let one measurement serve different development tools: pretty
output for a developer, JUnit for a CI test tab, Markdown for a build summary,
JSON for automation, CSV for analysis, HAR for HTTP tooling, and OpenMetrics
for a textfile collector. Example workflows are in
[Software development workflows](development-workflows.md).

## Availability

| Format | Single URL | Suite | Intended use |
| --- | --- | --- | --- |
| `pretty` | Yes | Yes | Interactive terminal output. |
| `json` | Yes | Yes | Indented canonical structured data. |
| `jsonl` | Yes | Yes | One compact JSON document per invocation. |
| `csv` | Aggregate | Yes | Spreadsheets and per-sample analysis. |
| `junit` | Aggregate | Yes | CI test-report ingestion. |
| `markdown` | Aggregate | Yes | Build summaries and review comments. |
| `openmetrics` | Aggregate | No | Metrics scraping or textfile collectors. |
| `har` | Aggregate | No | Browser and HTTP archive tooling. |

Requesting CSV, JUnit, Markdown, OpenMetrics, or HAR causes a single URL to use
the aggregate path even when `--repeat` is one. OpenMetrics and HAR reject
`--file` before suite requests begin.

## Pretty

Pretty output is the default. It displays response metadata, phase timings,
and configured policy results. For aggregates it adds statistical tables,
failed samples, baseline comparison, regression results, and diagnostics.

The `NO_COLOR` environment variable disables ANSI color. The display-related
environment variables are documented in the [CLI reference](cli-reference.md).

## JSON and JSONL

```console
httpstatr https://example.com --format json
httpstatr https://example.com --repeat 20 --format jsonl
```

`json` is indented. `jsonl` is one compact object followed by a newline. It is
safe to append multiple invocations to a file:

```console
httpstatr https://example.com --repeat 20 --format jsonl >> runs.jsonl
```

Schema selection is based on behavior:

- an ordinary single request uses schema v1 for original-httpstat
  compatibility;
- repeated and aggregate features use the current schema v3;
- a suite uses suite schema v1 and embeds schema v3 endpoint results.

Read [Aggregate schema v3](result-schema-v3.md) for field semantics.

## CSV

```console
httpstatr https://example.com --repeat 50 --format csv > samples.csv
```

CSV writes one row per measured sample, including status, phase timings,
response size, sample exit code, and transport or policy errors. It is intended
for statistical analysis rather than round-tripping back into `httpstatr`.

Suite CSV identifies the endpoint and emits its sample rows in endpoint
declaration order.

## JUnit XML

```console
httpstatr https://example.com --repeat 20 --format junit > httpstatr.xml
```

JUnit contains one test case per measured sample. Failed transports,
assertions, and SLOs become failures. When regression rules are configured, a
regression-policy test case records their result. Suite JUnit combines endpoint
results for CI systems that support JUnit report ingestion.

## Markdown

```console
httpstatr https://example.com --repeat 20 --format markdown > report.md
```

Markdown includes summary and aggregate timing tables, baseline p95 changes,
regression outcomes, and failed samples. Suite Markdown includes suite policy
status and each endpoint.

## OpenMetrics

```console
httpstatr https://example.com --repeat 20 --format openmetrics > metrics.prom
```

The text exposition contains phase statistics, pass ratio, and sample counts.
It is deterministic and suitable for a Prometheus textfile collector or an
adapter that publishes command output. It does not send metrics by itself.

OpenMetrics supports one URL per invocation and is unavailable for suites.

## HAR 1.2

```console
httpstatr https://example.com -L --repeat 10 --format har > result.har
```

HAR emits one entry per measured sample. It includes a UTC
`startedDateTime`, response status, available response metadata, and phase
timings. Failed samples are retained. Values that curl cannot provide are
represented using the HAR convention, including `-1` for unavailable timing
fields. Curl only exposes cumulative redirect timing and final connection
measurements, so HAR output does not invent per-hop timings.

HAR supports one URL per invocation and is unavailable for suites.

## Saved results

```console
httpstatr https://example.com --repeat 30 \
  --format markdown --save result.json > result.md
```

Here, stdout receives Markdown and `result.json` receives schema v3 JSON. Saved
aggregate results can be used with `--compare`. Sensitive request metadata is
redacted unless `--show-secrets` was explicitly selected.
