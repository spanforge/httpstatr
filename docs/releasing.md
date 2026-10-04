# Release process

## Requirements

- A clean `main` branch whose CI run passes on Windows, Linux, and macOS.
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

## Publish the release candidate

1. Merge the verified release commit into `main`.
2. Create and push an annotated tag matching the Cargo version:

   ```console
   git tag -a v1.0.0-rc.1 -m "httpstatr 1.0.0-rc.1"
   git push origin v1.0.0-rc.1
   ```

3. The release workflow builds four archives, generates `SHA256SUMS`, and
   creates a GitHub prerelease. Download every archive and verify its checksum.
4. Publish the already verified crate deliberately:

   ```console
   cargo publish --locked
   ```

Crates.io publication is intentionally manual because published versions
cannot be replaced. Promote a later stable `1.0.0` only after release-candidate
feedback and successful installation tests.
