# History and trends

History records aggregate measurements in a local append-only JSONL file. It is
opt-in and supports one URL per invocation.

For development teams, history provides lightweight evidence across commits,
builds, or environments without requiring a metrics service. It can reveal
when a latency shift first appeared and provide candidates for investigation;
it is not a replacement for production monitoring. See
[Software development workflows](development-workflows.md) for a release-trend
example.

## Record a run

```console
httpstatr https://api.example.com/health \
  --repeat 20 \
  --history .httpstatr/history.jsonl \
  --history-name health \
  --tag ci \
  --tag staging \
  --commit abc123
```

History options are:

| Option | Meaning |
| --- | --- |
| `--history PATH` | Append to this JSONL file. |
| `--history-name NAME` | Assign a stable logical name to the measurement. |
| `--tag TAG` | Add a searchable tag; repeatable. |
| `--commit VALUE` | Associate a source-control revision or build identifier. |

The parent directory is created when needed. Each successful history write
appends one validated JSON object and flushes it before returning. Records
contain a Unix timestamp, metadata, and a redacted schema v3 result.

`--history` cannot be combined with `--file` or `--show-secrets`. An ordinary
single measurement is promoted to an aggregate record so history entries have
a consistent schema.

## List records

Without a subcommand, `history` lists the newest 30 matching records, oldest to
newest within the selected set:

```console
httpstatr history --history .httpstatr/history.jsonl
httpstatr history --history .httpstatr/history.jsonl list --last 100
```

Filters can be combined:

```console
httpstatr history --history .httpstatr/history.jsonl list \
  --name health \
  --url https://api.example.com/health \
  --tag staging \
  --commit abc123 \
  --since 1767225600 \
  --until 1769817600 \
  --last 30
```

`--since` and `--until` are inclusive Unix timestamps in seconds. `--last 0`
returns no rows.

## Display a trend chart

```console
httpstatr history --history .httpstatr/history.jsonl list \
  --name health --last 30 --chart
```

The terminal chart visualizes the filtered series using aggregate total
latency. Keep the measurement configuration and environment consistent if the
chart is being used to judge a performance trend.

## Export

Write every record to stdout:

```console
httpstatr history --history history.jsonl export > backup.jsonl
```

Or write directly to a file:

```console
httpstatr history --history history.jsonl export --output backup.jsonl
```

## Import

```console
httpstatr history --history history.jsonl import backup.jsonl
```

Every source line is parsed and validated before compatible records are
appended. Keep backups when combining histories from multiple machines because
history has no automatic record de-duplication.

## Prune

Retain only the newest 500 records:

```console
httpstatr history --history history.jsonl prune --keep 500
```

Pruning rewrites through a temporary file before replacing the history file.
Use `--keep 0` to remove all records while retaining an empty history file.

## Storage and privacy

History is an ordinary local JSONL file, not a database or remote service. URL
credentials, sensitive headers, curl arguments, assertion values, redirect
locations, and token-like query parameters are redacted before storage. Avoid
putting confidential business data in history names, tags, and commit fields;
those metadata values are intended to remain searchable and are not secret
containers.
