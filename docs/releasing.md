# Release process

## Requirements

- A clean `main` branch with passing Windows, minimum Rust version, and
  dependency validation. Cross-platform CI provides additional source checks;
  version 1.0.0 publishes and supports Windows x86-64 only.
- Rust 1.85 or newer and curl 7.50.0 or newer.
- Maintainer access to the GitHub repository and the `httpstatr` crate.
- A crates.io API token configured with `cargo login`.

## Prepare and verify

1. Update the package version and `CHANGELOG.md` date and link.
2. Run:

   ```console
   cargo fmt -- --check
   cargo clippy --all-targets -- -D warnings
   cargo test --all-targets --locked
   cargo build --release --locked
   cargo package --locked
   cargo publish --dry-run --locked
   ```

3. Run `cargo audit` and `cargo deny check` locally when those tools are
   installed. CI performs the same supply-chain checks.
4. Confirm `httpstatr --version` and exercise a request with JSON, HAR, and
   OpenMetrics output on each supported operating system.

## Publish the Windows stable release

1. Merge the verified release commit into `main`.
2. Create and push an annotated tag matching the Cargo version:

   ```console
   git tag -a v1.0.0 -m "httpstatr 1.0.0"
   git push origin v1.0.0
   ```

3. The release workflow builds the Windows x86-64 ZIP and standalone executable,
   generates `SHA256SUMS`, and creates a stable GitHub release. Download the
   ZIP, verify its checksum, and test the extracted executable on Windows.
4. Publish the already verified crate deliberately:

   ```console
   cargo publish --locked
   ```

Crates.io publication is intentionally manual because published versions
cannot be replaced. It is separate from publishing the Windows GitHub assets.

## Stable releases

Use the same verification process for a stable Cargo version such as `1.0.0`
and its matching `v1.0.0` tag. Tags with a prerelease suffix create prereleases;
stable tags create stable releases. The workflow checks the minimum Rust version,
source, dependencies, Windows tests, and extracted archive smoke tests.
It includes a standalone Windows executable, checksums for every asset, and
artifact attestations. See [production readiness](production-readiness.md) for
the additional clean-machine and staging validation required before promotion.
