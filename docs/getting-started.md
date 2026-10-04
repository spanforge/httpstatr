# Getting started

## Install the prerequisites

`httpstatr` uses curl as its transport. Install curl 7.50.0 or newer and make
sure it advertises the `HTTP` protocol:

```console
curl --version
```

Install the release candidate from crates.io:

```console
cargo install httpstatr --version 1.0.0-rc.1 --locked
```

Rust 1.85 or newer is needed for this installation method. Prebuilt archives
from [GitHub Releases](https://github.com/spanforge/httpstatr/releases) do not
require Rust. Verify the downloaded archive against `SHA256SUMS`, extract it,
and place the executable in a directory on `PATH`.

If curl is installed outside `PATH`, select it explicitly:

```console
httpstatr --curl-bin /opt/curl/bin/curl https://example.com
```

On Windows:

```powershell
httpstatr --curl-bin C:\Tools\curl\bin\curl.exe https://example.com
```

`HTTPSTAT_CURL_BIN` sets a persistent default; `--curl-bin` overrides it.

## Measure a request

```console
httpstatr https://example.com
```

The default pretty view displays the response, connection information, and a
waterfall-like timing breakdown. A successful request returns exit code `0`.

Choose JSON when another program will consume the result:

```console
httpstatr https://example.com --format json
```

Save canonical JSON while retaining the pretty terminal view:

```console
httpstatr https://example.com --save result.json
```

## Make an authenticated API request

Options not owned by `httpstatr` are forwarded to curl:

```console
httpstatr https://api.example.com/v1/items \
  -H "Authorization: Bearer $API_TOKEN" \
  -H "Accept: application/json" \
  --http2
```

Common authentication headers, cookies, credentials, and token-like query
parameters are redacted from output. Environment-variable expansion is done by
your shell before `httpstatr` starts, just as it is for curl.

## Send data

```console
httpstatr https://api.example.com/v1/items \
  -X POST \
  -H "Content-Type: application/json" \
  --data-binary '{"name":"sample"}' \
  --expect-status 201
```

On PowerShell, use its normal quoting rules and a backtick for multiline
commands, or keep the command on one line.

## Add correctness and performance checks

```console
httpstatr https://api.example.com/health \
  --expect-status 200-299 \
  --expect-header "content-type:application/json" \
  --expect-body-contains '"healthy":true' \
  --slo total=500,ttfb=250
```

Assertions answer whether the response is correct. SLOs answer whether the
request stayed within fixed latency limits. Both affect the process exit code,
which makes the command suitable for scripts and CI.

This lets the same command begin as a local debugging check and later become a
repeatable pull-request or release gate. Store it in a project script or suite
when the request and policy should be shared by the team.

## Measure a distribution

```console
httpstatr https://api.example.com/health --warmup 3 --repeat 30 --delay 0.1
```

This performs three unmeasured warmups followed by 30 measured requests. Each
request runs in a separate curl process and uses a separate connection. The
aggregate contains min, max, mean, median, p90, p95, p99, and standard
deviation for every phase.

## Choose the next guide

- For examples across the development lifecycle, read
  [Software development workflows](development-workflows.md).
- For request construction and validation, read
  [Requests and assertions](requests-and-assertions.md).
- For percentiles and CI gates, read
  [Measurements and regression gates](measurements.md).
- For multiple APIs, read [Endpoint suites](suites.md).
- For machine-readable reports, read [Output formats](output-formats.md).
- For long-term local trends, read [History and trends](history.md).
