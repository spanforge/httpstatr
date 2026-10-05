# Requests and assertions

## Curl argument forwarding

The command shape is:

```text
httpstatr [HTTPSTATR_OPTIONS] URL [CURL_ARGS...]
```

Recognized `httpstatr` options may occur before or after the URL. Remaining
options are forwarded to curl. This preserves curl behavior for methods,
headers, bodies, redirects, proxies, authentication, TLS, certificates, DNS
overrides, and HTTP protocol selection.

```console
httpstatr https://api.example.com/resource \
  -L \
  -X PUT \
  -H "Content-Type: application/json" \
  --data-binary @request.json \
  --cert client.pem \
  --key client.key
```

The following curl output controls are reserved because the runner supplies
them internally:

| Short | Long |
| --- | --- |
| `-w` | `--write-out` |
| `-D` | `--dump-header` |
| `-o` | `--output` |
| `-s` | `--silent` |
| `-S` | `--show-error` |

Their `--option=value` forms are reserved as well. Use `--save` for the
canonical result and `HTTPSTAT_SHOW_BODY` or `HTTPSTAT_SAVE_BODY` for response
body behavior.

## Timeouts

`--connect-timeout SECONDS` controls connection establishment and `--timeout
SECONDS` controls the whole curl operation. Positive decimal values are
accepted:

```console
httpstatr https://api.example.com --connect-timeout 2.5 --timeout 15
```

For a single request, a curl failure preserves curl's exit code. Curl commonly
uses `28` for a timeout. In an aggregate, any measured transport failure makes
the aggregate exit code `6`, while the original curl code remains in its sample.

## Status assertions

`--expect-status` accepts one status, an inclusive range, or a comma-separated
list. The option is repeatable. The response passes when its final status
matches any supplied status or range.

```console
httpstatr https://api.example.com \
  --expect-status 200-299 \
  --expect-status 304
```

Valid HTTP status values are 100 through 599. Invalid values and reversed
ranges are rejected before the request begins.

## Header assertions

Use a name to require header presence, or `NAME:VALUE` to require an exact
value. Names are matched case-insensitively; values are matched exactly.

```console
httpstatr https://api.example.com \
  --expect-header content-type \
  --expect-header "cache-control:no-cache"
```

Every repeated header assertion must pass.
When a response contains the same header more than once, a value assertion
passes if any individual value matches. Header maps preserve those values
separated by newlines.

## JSON assertions

`--expect-json` accepts a JSON Pointer and a JSON value separated by `=`.
Use `:type` before `=` to assert a type:

```console
httpstatr https://api.example.com/health --expect-json '/status="healthy"'
httpstatr https://api.example.com/health --expect-json '/ok=true'
httpstatr https://api.example.com/items --expect-json '/items:type=array'
```

Types are `null`, `boolean`, `number`, `string`, `array`, and `object`.
An empty pointer selects the whole document; `/items/0` selects an array item.
Use `~0` for a literal tilde and `~1` for a literal slash in a field name.
Every assertion must pass. Invalid JSON, missing fields, and mismatched values
or types return assertion exit code 5. Failure reports identify the pointer
without printing the expected or observed JSON value. The same syntax works
in suite `expect_json` arrays. JSON assertions use the 16 MiB body-content limit.

## Body assertions

Body size checks do not need to decode body text:

```console
httpstatr https://api.example.com \
  --min-body-bytes 10 \
  --max-body-bytes 1048576
```

Text and regular-expression checks are repeatable:

```console
httpstatr https://api.example.com \
  --expect-body-contains '"status":"ok"' \
  --expect-body-regex '"version"\s*:\s*"[0-9.]+"'
```

All configured body checks must pass. Regex syntax is validated before the
request. Body text is read as lossy UTF-8 for matching. Content and regex
assertions are limited to bodies of 16 MiB or less; a larger body produces a
local-processing error rather than an unbounded allocation. Size-only checks
still use curl's reported body size.

An assertion failure returns exit code `5` and is included in structured
output. Sensitive expected and actual header values are redacted by default.

## Latency SLOs

`--slo` accepts comma-separated `KEY=MILLISECONDS` pairs:

```console
httpstatr https://api.example.com \
  --slo total=500,connect=100,ttfb=250,dns=50,tls=150
```

| Key | Curl value checked |
| --- | --- |
| `total` | Complete request time. |
| `connect` | Cumulative time until the connection completed. |
| `ttfb` | Cumulative time to the first response byte. |
| `dns` | Cumulative name-lookup time. |
| `tls` | Cumulative pre-transfer time. |

Thresholds must be positive integers. A value violates an SLO only when it is
greater than the threshold. SLO violations return exit code `4` when no
higher-precedence transport or assertion failure exists.

## Redirects and connections

Pass curl's `-L` or `--location` to follow redirects:

```console
httpstatr https://example.com/old-path -L
```

Aggregate schema v3 records ordered redirect status/location pairs, the final
effective URL, redirect count and cumulative redirect time, local and remote
addresses, HTTP version, and curl's connection count. Curl does not expose a
full timing breakdown for each hop, so `httpstatr` does not synthesize one.

## Secret redaction

By default, output and saved metadata redact:

- URL user information;
- authorization and proxy-authorization headers;
- cookies and set-cookie headers;
- API key, authentication token, password, secret, signature, and similar
  sensitive headers;
- common token, password, secret, API-key, key, auth, and signature query
  parameters;
- sensitive curl arguments, verbose curl text, assertion failures, redirect
  locations, history records, and debug commands.

`--show-secrets` disables output redaction for intentional local debugging. It
cannot be combined with `--history`, and output produced with it should not be
attached to CI logs or issue reports.
