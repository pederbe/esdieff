# HTTPS acquisition exercise

This standalone tool evaluates an existing Rust HTTP/TLS combination without
changing `esdieff acquire`. It writes no downloaded files. It returns success
only for a complete HTTP 200 body with identity encoding and the independently
supplied SHA-256. It does not yet meet two aggregate resource guarantees in the
[acquisition contract](../../docs/acquisition-contract.md).

Run from the repository root:

```sh
cargo test --locked --manifest-path tools/https-qualification/Cargo.toml
cargo run --locked --manifest-path tools/https-qualification/Cargo.toml -- \
  https://raw.githubusercontent.com/one-data-model/playground/41be1e0e564ad39f4d5a1c41fcccef435e388ee9/sdfObject/sdfobject-switch_binary.sdf.json \
  6fe5a1538da0b5f48898e367f0f03b6b70fa38b6ebaa67117e408ab87fb0b61a
```

The live example should report 787 verified bytes. An unavailable network is a
failed live observation. The deterministic tests use local TLS servers and do
not need internet access or a certificate-generation executable.

## Transport and trust configuration

The exercise uses ureq 3.5.0, rustls 0.23.46 and
[rustls-provider-rustcrypto-pq 0.1.1](https://docs.rs/crate/rustls-provider-rustcrypto-pq/0.1.1).
The provider is passed directly to the TLS configuration. No process-global
provider is installed or consulted. ureq's defaults are disabled. Only
`rustls-no-provider` and `rustls-webpki-roots` are enabled, excluding ring,
compression, cookies, platform certificate stores and automatic proxy discovery.
Redirects and connection pooling are disabled. Each operation uses one new
agent, so the stale pooled-connection retry path is unavailable.

The bundled Mozilla roots come from locked `webpki-roots` 1.0.11. Ordinary leaf
and intermediate rotation works when a valid chain reaches an included root.
A new root absent from that snapshot needs a dependency update and new binary.
There is no current-certificate pin, permissive verifier, client certificate,
credential source or native trust-store override. A matching digest does not
replace certificate verification.

The body is read in 16 KiB blocks, retaining at most 4 MiB. An oversized declared
length fails before reading. Chunked and close-delimited bodies use the same
ceiling, without decoding content encodings. A 30-second global timeout extends
through body consumption. This is an operational network timeout, not a promise
to interrupt arbitrary computation or an unresponsive operating system.

`Failure` variants are diagnostic exercise results, not proposed receipt phases.
Application staging, destination protection and receipt delivery are outside
this standalone exercise.

## Checked failures and limitations

Tests exercise RSA and ECDSA server authentication, an unknown issuer, an
incorrect hostname and an expired leaf. The expiration test calls the selected
webpki verifier at a fixed future time. It does not disable verification in the
HTTP client. The [fixtures](fixtures/README.md) contain public test keys and a
test CA. OpenSSL is not a build or test dependency.

Two tests deliberately preserve negative qualification evidence:

- ureq starts its connection allowance after target resolution. A delayed resolver
  and delayed TLS handshake can together exceed one connection allowance while
  each phase succeeds. Setting both timeouts to ten seconds does not implement
  the existing cumulative ten-second promise.
- ureq checks header size separately for each informational response and the final
  response. Twenty 4 KiB informational headers followed by HTTP 200 succeed,
  exceeding the existing cumulative 64 KiB promise. A single oversized header is
  rejected, so this is a cumulative-work gap rather than unbounded header storage.

Those gaps prevent treating this configuration as a drop-in replacement for the
current application transport. The application contract remains unchanged.

The provider is unaudited and pins `rsa` 0.10.0-rc.19, a release candidate.
`cargo deny check advisories` reports unfixed
[RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071.html).
This client performs public-key verification without a private key. Do not
interpret a successful download as clearing the advisory or as qualifying the
provider for servers or clients with RSA credentials. The local RSA server
fixture signs with a deliberately public test key.

## Feature inspection and supported platforms

Inspect normal, build and development dependencies for each supported target:

```sh
cargo tree --locked --manifest-path tools/https-qualification/Cargo.toml \
  --target x86_64-unknown-linux-gnu -e normal,build,dev
cargo tree --locked --manifest-path tools/https-qualification/Cargo.toml \
  --target aarch64-unknown-linux-gnu -e normal,build,dev
cargo tree --locked --manifest-path tools/https-qualification/Cargo.toml \
  --target aarch64-apple-darwin -e normal,build,dev
cargo tree --locked --manifest-path tools/https-qualification/Cargo.toml \
  --target x86_64-pc-windows-msvc -e normal,build,dev
```

None of those enabled graphs contains libcurl, OpenSSL, ring, aws-lc-rs, zlib or
`cc`. Optional inactive packages can still appear in Cargo.lock or metadata.
The provider's upstream tests use aws-lc-rs as an independent peer. Those
upstream development dependencies are not dependencies of this exercise.

RustCrypto uses Rust CPU intrinsics and target-specific implementations.
`cmov` 0.5.4 uses inline `asm!` on x86_64 and aarch64. The selected closure is not
assembly-free. The exercise forbids first-party unsafe Rust. OS entropy and
sockets still use operating-system interfaces. This is not a claim that the
operating system or Rust standard library contains only Rust.

The maintained `scripts/check.sh` runs the exercise on the existing Linux,
macOS and Windows hosted matrix. Target graph inspection alone does not prove
native execution. Recheck the graphs after dependency or feature changes.
