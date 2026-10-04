# Version 3.0 architecture decisions

## Transport backend

The first public 1.0 release retains curl as the only request backend. A native Rust client
would improve in-process connection pooling and make span instrumentation
easier, but it cannot preserve arbitrary curl argument forwarding. Shipping it
as a selectable backend would create two request semantics for authentication,
proxies, TLS, redirects, and protocol negotiation.

A native backend should be reconsidered only with a separate, explicitly
typed request configuration and conformance tests against curl. Until then,
curl remains the compatibility boundary and the `Runner` trait remains the
extension point.

## OpenTelemetry

Direct span export is deferred. OpenTelemetry requires exporter selection,
endpoint and authentication configuration, retry behavior, and a dependency
set disproportionate to this CLI. JSON schema v3 and history records provide a
stable source for an external collector or CI adapter without changing request
execution.

## OpenMetrics and HAR

OpenMetrics text exposition is included because it is deterministic and needs
no runtime service. HAR 1.2 export is included for successful and failed
samples, with unavailable fields represented according to the HAR convention.
Neither renderer changes policy status or execution behavior.
