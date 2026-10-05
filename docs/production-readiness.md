# Production readiness

Production readiness is established for a specific commit and supported runtime
matrix. Passing local tests alone does not certify a release.

## Implemented safeguards

- curl arguments execute directly, without a shell. Implicit curlrc configuration
  is disabled with `-q` as the first argument; explicit config files and output,
  retry, URL, parallel, and multi-transfer overrides are rejected.
  Requests and redirects are restricted to HTTP(S) protocols.
- A request defaults to 10 seconds for connecting and 60 seconds overall.
  Both can be overridden with tool flags. `--run-timeout SECONDS` bounds the
  whole request run or suite, including warmups and delays.
- Downloads default to 64 MiB. `--max-download-bytes BYTES` changes this limit.
  curl enforces supported size checks, and the supervisor checks file sizes
  every 10 ms. A transfer can briefly exceed the limit between checks on older
  curl versions. Headers and curl diagnostic output are limited to 1 MiB each.
- Ctrl-C terminates active curl processes and interrupts delays. Completed batch
  samples remain in the output; canceled samples have exit code 130. Forceful
  termination cannot guarantee a report or temporary-file cleanup.
- Body content assertions remain limited to 16 MiB. Files containing suites are
  limited to 4 MiB; baselines and individual history records to 64 MiB.
- Suites allow at most 256 endpoints and 100,000 measured plus warmup requests,
  with concurrency bounded to 64. Results retain individual samples, so memory
  use grows with retained sample count and header size.
- `--history` rejects `--show-secrets`. Credential redaction includes attached
  user/proxy authentication options, known sensitive headers and query keys,
  and JSON assertion failure output. Redaction is not an arbitrary-body scanner;
  avoid secrets in names, tags, custom non-sensitive headers, and body assertions.
- History operations hold an operating-system file lock. Import locks its source
  without waiting to avoid opposite-direction import deadlocks. Listing and pruning
  retain only the requested number of records. Import and export scan records.
  Saved reports and prune operations stage and sync output before atomic replacement.
- History will reject an incomplete trailing record before appending. Back up the
  file and repair the incomplete tail deliberately. Existing `.bak` replacement
  files are recovered under the lock. Local filesystem semantics are assumed;
  shared network filesystems need separate validation.

## Local verification

```console
cargo fmt -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
cargo +1.85.0 test --all-targets --locked
cargo audit
cargo deny check
cargo build --release --locked
cargo package --locked
```

Integration tests use local HTTP, HTTPS, and proxy servers. They cover successful
TLS and certificate rejection, response checks, limits, interruption, concurrent
history writes, relative redirects, XML parsing, and HAR timing consistency.
The TLS private key in `tests/fixtures` is a public test fixture and must never
be deployed. Windows sandbox restrictions can prevent Schannel from acquiring
credentials; rerun the HTTPS test outside that sandbox when this occurs.

## Stable-release acceptance

Before promoting a release to `1.0.0`:

1. Confirm the exact tagged commit passes Windows x86-64, Rust 1.85, and
   dependency validation jobs. Windows x86-64 is the supported platform for
   1.0.0; cross-platform source checks do not imply supported release binaries.
2. Download each archive, verify `SHA256SUMS`, and run the executable on a clean
   machine. Test installation with curl absent and with the documented minimum.
3. Verify bounded behavior against slow headers, streaming bodies, redirects,
   proxies, invalid certificates, storage failures, and interruption.
4. Exercise representative authenticated staging endpoints and suites. Record
   runtime, memory, disk use, policy outcomes, and any environment-specific issues.
5. Confirm reported timings and pass/fail status match across output formats.
   XML tests check syntax and HAR tests check recorded fields and timing totals;
   these do not constitute complete external format certification.
6. Publish binaries from the verified workflow, with checksums and artifact
   attestations. Confirm that downloaded binaries match the release version.

Automated releases use `vVERSION` tags matching Cargo. Tags with a prerelease
suffix create prereleases; stable tags create stable releases. The workflow
validates source, tests Windows, checks packaging, and smoke-tests the extracted
Windows archive before publishing. Linux and macOS release binaries are deferred
until those platforms have their own supported installation matrix.

## Follow-on distribution

Winget/Scoop and Homebrew submissions require final stable artifact URLs,
checksums, supported OS versions, and feed-maintainer review. Prepare those
submissions after the stable workflow has produced verified artifacts.
Persistent connections and automatic retries require separately documented
measurement semantics; the current sampling method starts one curl process
per sample and records every requested sample.
