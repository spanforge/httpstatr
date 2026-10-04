# Troubleshooting

## Curl cannot be found

If the command reports that curl could not be started, confirm it is installed:

```console
curl --version
```

Select a specific executable when several versions exist:

```console
httpstatr --curl-bin /usr/local/bin/curl https://example.com
```

Or set `HTTPSTAT_CURL_BIN`. The selected curl must be version 7.50.0 or newer
and list `HTTP` in its protocol support.

## A curl option is rejected

`-w`, `-D`, `-o`, `-s`, and `-S`, plus their long forms, are reserved because
`httpstatr` uses them internally. Remove those options and choose an
`httpstatr` output format or `--save` instead.

All other options are forwarded to curl. If curl rejects one, run the equivalent
request with curl directly and consult `curl --help all`. Curl features vary by
build; the version line lists enabled protocols and capabilities.

## The request timed out

Increase the connection and total timeouts separately:

```console
httpstatr https://example.com --connect-timeout 5 --timeout 30
```

Curl commonly returns code `28` for timeouts. A single request preserves that
code; a repeated run returns aggregate code `6` and records `28` on the failed
sample.

## Assertions fail unexpectedly

- Status alternatives use a comma list or repeated `--expect-status` options.
- Header names are case-insensitive, but expected header values are exact.
- Assertions inspect the final response headers. Use curl `-L` when the policy
  is intended for the final redirect destination.
- Body matching uses lossy UTF-8 and is case-sensitive.
- Shell quoting can change regexes or JSON text before the program receives
  them. Inspect the shell's quoting rules and prefer single quotes on POSIX
  shells when the expression contains backslashes.
- Content assertions reject bodies larger than 16 MiB. Use size assertions or
  a dedicated response-validation tool for larger content.

Use `--format json` to inspect structured assertion failures. Sensitive values
remain redacted.

## A baseline is rejected

The baseline must:

- be aggregate schema v2 or v3;
- contain at least one sample with timing data;
- have the same redacted URL as the current run;
- have the same redacted curl argument list.

Repeat count, warmup count, delay, and curl runtime version may differ. Create a
new baseline when the request method, headers, curl options, or endpoint URL
changes.

## Results vary between runs

HTTP timings include DNS, networking, server scheduling, TLS, and curl process
startup. For a more useful comparison:

- run from the same machine and network;
- keep curl options and protocol selection consistent;
- use warmups and enough measured samples;
- avoid running unrelated resource-heavy work at the same time;
- use percentiles and a realistic tolerance instead of one sample;
- distinguish normal variance from a repeatable regression.

Each sample uses a new curl process and connection, so the tool does not model
an application connection pool.

## JSON schema differs between commands

An ordinary single request uses schema v1 for compatibility. Repeats, warmups,
CSV/JUnit/Markdown/OpenMetrics/HAR, comparison, history, and other aggregate
features use schema v3. A suite has a suite schema v1 root and embeds schema v3
endpoint results. Schema v2 is legacy and accepted as a baseline.

## OpenMetrics or HAR does not work with a suite

Both formats currently require one URL. Use JSON, JSONL, CSV, JUnit, Markdown,
or pretty output for `--file` suites. Export endpoints separately when HAR or
OpenMetrics is required.

## History contains no matching rows

Remove filters one at a time and check the selected path:

```console
httpstatr history --history .httpstatr/history.jsonl list --last 30
```

Time filters are inclusive Unix timestamps in seconds. History paths supplied
while recording and querying must point to the same file.

## Colors or body output are unwanted

Set `NO_COLOR` to any value to disable ANSI color. Set
`HTTPSTAT_SHOW_BODY=false` to suppress the body preview and
`HTTPSTAT_SAVE_BODY=false` to delete the temporary body after rendering.

## Debug safely

Enable redacted debug output:

```console
HTTPSTAT_DEBUG=true httpstatr https://example.com
```

PowerShell:

```powershell
$env:HTTPSTAT_DEBUG = "true"
httpstatr https://example.com
```

Debug mode keeps normal redaction. Avoid `--show-secrets` in CI logs, shared
terminals, bug reports, and history workflows. When reporting a security issue,
follow [SECURITY.md](../SECURITY.md).
