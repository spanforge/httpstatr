# CLI reference

## Request command

```text
httpstatr [OPTIONS] [URL] [CURL_ARGS]...
```

Provide either a URL or `--file SUITE`, but not both. Options recognized by
`httpstatr` are extracted wherever they appear; remaining arguments after the
URL are forwarded to curl. Known curl value-taking options keep their values
literal. `--` stops extraction of tool options.

### General options

| Option | Value | Description |
| --- | --- | --- |
| `-h`, `--help` | | Print command help. |
| `-V`, `--version` | | Print the package version. |
| `-f`, `--format` | `pretty`, `json`, `jsonl`, `csv`, `junit`, `markdown`, `openmetrics`, `har` | Select output; default `pretty`. |
| `--save` | path | Write canonical JSON in addition to display output. |
| `--curl-bin` | path or command | Select curl; overrides `HTTPSTAT_CURL_BIN`. |
| `--show-secrets` | | Disable normal output redaction. Incompatible with history. |
| `--generate-completion` | `bash`, `zsh`, `fish`, `powershell` | Write a completion script to stdout and exit. |
| `--generate-man` | | Write a roff man page to stdout and exit. |

### Request and timing options

| Option | Value | Description |
| --- | --- | --- |
| `--connect-timeout` | positive seconds | Connection timeout; default 10 seconds. |
| `--timeout` | positive seconds | Total request timeout; default 60 seconds. |
| `--run-timeout` | positive seconds | Whole-run deadline, including warmups and delays. |
| `--max-download-bytes` | positive bytes | Response download limit; default 67108864 bytes. |
| `--repeat` | `1..10000` | Number of measured requests; default `1`. |
| `--warmup` | `0..10000` | Number of unmeasured warmups; default `0`. |
| `--delay` | nonnegative seconds | Wait between requests; decimals accepted; default `0`. |

### Response and performance policies

| Option | Value | Description |
| --- | --- | --- |
| `--expect-status` | status, range, or comma list | Require a final status; repeatable. |
| `--expect-header` | `NAME` or `NAME:VALUE` | Require header presence or exact value; repeatable. |
| `--min-body-bytes` | bytes | Require at least this response size. |
| `--max-body-bytes` | bytes | Require no more than this response size. |
| `--expect-body-contains` | text | Require body text; repeatable. |
| `--expect-body-regex` | regex | Require a Rust regular-expression match; repeatable. |
| `--expect-json` | `POINTER=JSON` or `POINTER:type=TYPE` | Assert a JSON field value or type; repeatable. |
| `--slo` | comma-separated thresholds | Apply `total`, `connect`, `ttfb`, `dns`, or `tls` limits in milliseconds. |
| `--compare` | path | Compare with a schema v2 or v3 aggregate baseline. |
| `--fail-if` | expression | Apply an aggregate regression rule; repeatable. |

### History recording options

| Option | Value | Description |
| --- | --- | --- |
| `--history` | path | Append a redacted schema v3 record to this JSONL file. |
| `--history-name` | name | Store a logical measurement name. |
| `--tag` | tag | Store a searchable tag; repeatable. |
| `--commit` | value | Store a source revision or build identifier. |

History recording supports one URL and cannot be combined with `--file` or
`--show-secrets`.

### Suite options

| Option | Value | Description |
| --- | --- | --- |
| `--file` | TOML path | Run a versioned endpoint suite. |
| `--validate` | | Check the suite without curl or network requests. |
| `--profile` | name | Select suite variables from a named environment profile. |
| `--concurrency` | `1..64` | Override maximum concurrent endpoints. |
| `--suite-min-success-rate` | `0..100` | Override minimum endpoint pass percentage. |
| `--suite-max-failures` | nonnegative integer | Override maximum failed endpoint count. |

Suites do not support OpenMetrics or HAR output. Their supported formats are
pretty, JSON, JSONL, CSV, JUnit, and Markdown.

## Reserved curl arguments

`httpstatr` owns curl's write-out, header, body-file, and silence controls.
These arguments and their long or `--option=value` forms are rejected:

```text
-w --write-out
-D --dump-header
-o --output
-s --silent
-S --show-error
-K --config
-: --next
-Z --parallel
-O --remote-name
-m --max-time
--url --remote-name-all --output-dir --trace --trace-ascii --stderr
--max-filesize --retry* --parallel* --no-disable --no-globoff
--proto --proto-redir
```

Attached reserved short options and reserved long-option abbreviations are
also rejected. Use separate value-taking short options (for example `-v -u
USER:PASSWORD`, rather than a combined token). Implicit curlrc loading and URL
globbing are disabled so each request produces one measured transfer.
Requests and followed redirects are restricted to HTTP and HTTPS.

## Local diagnostics

```console
httpstatr doctor
httpstatr doctor --curl-bin /path/to/curl
```

Checks curl capability, writable temporary storage, proxy configuration presence,
and configured certificate paths without requests or exposed proxy credentials.
This checks local setup, not remote certificate trust or endpoint availability.

All other curl options are passed through. Consult `curl --help all` and your
installed curl documentation for their semantics.

## History command

```text
httpstatr history [--history PATH] [COMMAND]
```

The default path is `.httpstatr/history.jsonl`. With no command, history behaves
like `list` with `--last 30`.

### List

```text
httpstatr history [--history PATH] list [OPTIONS]
```

| Option | Description |
| --- | --- |
| `--last N` | Return the newest N matches, displayed oldest to newest; default `30`. |
| `--name NAME` | Filter by stored name. |
| `--url URL` | Filter by requested URL. |
| `--tag TAG` | Filter by tag. |
| `--commit VALUE` | Filter by commit/build value. |
| `--since UNIX_SECONDS` | Inclusive lower timestamp bound. |
| `--until UNIX_SECONDS` | Inclusive upper timestamp bound. |
| `--chart` | Render a terminal trend chart. |

### Prune, import, and export

```console
httpstatr history --history history.jsonl prune --keep 500
httpstatr history --history history.jsonl import backup.jsonl
httpstatr history --history history.jsonl export
httpstatr history --history history.jsonl export --output backup.jsonl
```

## Environment variables

| Variable | Default | Description |
| --- | --- | --- |
| `HTTPSTAT_SHOW_BODY` | `false` | Show up to 1024 body bytes in pretty output. |
| `HTTPSTAT_SHOW_IP` | `true` | Show local and remote addresses and ports. |
| `HTTPSTAT_SHOW_SPEED` | `false` | Show upload and download speeds. |
| `HTTPSTAT_SAVE_BODY` | `true` | Keep the temporary body file after pretty output. |
| `HTTPSTAT_CURL_BIN` | platform curl name | Select the curl executable. |
| `HTTPSTAT_METRICS_ONLY` | `false` | Turn default pretty output into JSON. |
| `HTTPSTAT_DEBUG` | `false` | Print configuration and the redacted curl command. |
| `NO_COLOR` | unset | Disable ANSI color whenever present. |

Boolean variables accept `1`, `true`, `yes`, `on`, `0`, `false`, `no`, and
`off`, case-insensitively. An unrecognized boolean is a configuration error.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | Configuration, file, serialization, or other local-processing error. |
| `2` | Invalid command-line usage. |
| `4` | SLO violation. |
| `5` | Response assertion failure. |
| `6` | Aggregate transport failure. |
| `7` | Regression-policy failure. |
| `8` | Suite-policy failure. |
| `130` | Suite interrupted with Ctrl-C. |
| other | Single-request curl failure with curl's exit code preserved. |

Aggregate precedence is `6`, `5`, `4`, `7`, then `0`. A suite whose policy
fails returns `8`, while embedded endpoints keep their individual codes.

## Completion installation

Bash for the current user:

```console
mkdir -p ~/.local/share/bash-completion/completions
httpstatr --generate-completion bash > ~/.local/share/bash-completion/completions/httpstatr
```

Zsh, using a directory already in `fpath`:

```console
httpstatr --generate-completion zsh > /path/in/fpath/_httpstatr
```

Fish:

```console
mkdir -p ~/.config/fish/completions
httpstatr --generate-completion fish > ~/.config/fish/completions/httpstatr.fish
```

PowerShell for the current session:

```powershell
httpstatr --generate-completion powershell | Out-String | Invoke-Expression
```

Add the generated PowerShell script to the PowerShell profile for persistent
completion.

## Man page

```console
httpstatr --generate-man > httpstatr.1
man ./httpstatr.1
```

For a system-wide installation, copy the generated page into the appropriate
manual directory for the operating system and refresh its man-page index.
