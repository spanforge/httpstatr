# httpstatr documentation

This directory contains the complete user and maintainer documentation for
`httpstatr` 1.0.0-rc.1.

## User guides

| Guide | Use it for |
| --- | --- |
| [Getting started](getting-started.md) | Installing curl and `httpstatr`, running the first request, and choosing the next workflow. |
| [Software development workflows](development-workflows.md) | Local debugging, API checks, pull requests, CI, release validation, integration suites, and incident evidence. |
| [Comparison with the original httpstat](comparison-with-httpstat.md) | Shared behavior, additional capabilities, intentional differences, and migration examples. |
| [CLI reference](cli-reference.md) | Every command-line option, environment variable, limit, and exit code. |
| [Requests and assertions](requests-and-assertions.md) | Curl forwarding, timeouts, HTTP assertions, SLOs, response bodies, and secret redaction. |
| [Measurements and regression gates](measurements.md) | Repeats, warmups, statistics, baselines, regression expressions, diagnostics, and CI. |
| [Output formats](output-formats.md) | Pretty, JSON, JSONL, CSV, JUnit, Markdown, OpenMetrics, HAR, and saved results. |
| [Endpoint suites](suites.md) | The complete versioned TOML suite schema, defaults, concurrency, policies, and cancellation. |
| [History and trends](history.md) | Recording, filtering, charting, exporting, importing, and pruning local measurements. |
| [Troubleshooting](troubleshooting.md) | Curl errors, option conflicts, assertion failures, baseline errors, and secret-safe debugging. |

## Data formats

| Reference | Scope |
| --- | --- |
| [Aggregate schema v3](result-schema-v3.md) | Current aggregate result written by repeats, history, suites, reports, and advanced exporters. |
| [Legacy aggregate schema v2](result-schema-v2.md) | Compatibility reference for older saved baselines accepted by the current CLI. |

An ordinary single request serialized as JSON retains schema v1 for compatibility
with the original command. Aggregate features use schema v3.

## Maintainer references

- [Release process](releasing.md)
- [Transport and telemetry decisions](version-3-decisions.md)
- [Security policy](../SECURITY.md)
- [Changelog](../CHANGELOG.md)
- [Implementation plan](../implementationplan.md)

Run `httpstatr --help` for generated command help and `httpstatr history
--help` for history maintenance commands. Generated shell completions and a
roff man page are available through `--generate-completion` and
`--generate-man`.
