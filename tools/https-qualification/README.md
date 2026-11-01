# HTTPS acquisition exercise

This standalone tool evaluates an existing Rust HTTP/TLS combination without
changing `esdieff acquire`. It writes no downloaded files. It returns success
only for a complete HTTP 200 body with identity encoding and the independently
supplied SHA-256. Production integration remains pending because the candidate
does not yet meet two aggregate resource guarantees in the
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

## Candidate integration boundary

The candidate is ureq 3.5.0, rustls 0.23.46 and
[rustls-provider-rustcrypto-pq 0.1.1](https://docs.rs/crate/rustls-provider-rustcrypto-pq/0.1.1).
The provider is passed directly to the TLS configuration. No process-global
provider is installed or consulted. ureq's defaults are disabled. Only
`rustls-no-provider` and `rustls-webpki-roots` are enabled, excluding ring,
compression, cookies, platform certificate stores and automatic proxy discovery.
Redirects and connection pooling are disabled. Each operation uses one new
agent, so the stale pooled-connection retry path is unavailable.

The intended application seam stays private: a validated fixed-origin request
enters `ResponseSource::fetch`, which returns complete body bytes or an
acquisition failure. SHA-256 comparison, staging, protected destinations and
receipt delivery retain their existing owners. The exercise does not introduce
a public networking API. Its `Failure` variants are diagnostic exercise results,
not proposed receipt phases.

The body is read in 16 KiB blocks, retaining at most 4 MiB. An oversized declared
length fails before reading. Chunked and close-delimited bodies use the same
ceiling, without decoding content encodings. A 30-second global timeout extends
through body consumption. This is an operational network timeout, not a promise
to interrupt arbitrary computation or an unresponsive operating system.

## Trust and provider suitability

The bundled Mozilla roots come from locked `webpki-roots` 1.0.11. Ordinary leaf
and intermediate rotation works when a valid chain reaches an included root.
A new root absent from that snapshot needs a dependency update and new binary.
There is no current-certificate pin, custom permissive verifier, client
certificate, credential source or native trust-store override. A matching digest
does not replace certificate verification.

Tests exercise RSA and ECDSA server authentication, an unknown issuer, an
incorrect hostname and an expired leaf. The expiration test calls the selected
webpki verifier at a fixed future time. It does not disable verification in the
HTTP client. The fixtures contain deliberately public test keys and a test CA.
They were generated once with OpenSSL. OpenSSL is not a build or test dependency.

The provider is young and unaudited. Its upstream README asks consumers to review
it, but does not prohibit production use. Its tests use aws-lc-rs as an independent
peer. Those upstream development dependencies are not dependencies of this
exercise, whose own TLS tests use the explicitly selected Rust provider.

`cargo deny check advisories` reports
[RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071.html)
for `rsa` 0.10.0-rc.19. This remains an unfixed advisory, not a clean audit.
The acquisition client has no RSA private key and performs public-key signature
verification only. The vulnerable private-key operations are outside that use.
The local RSA server fixture does sign with a public test key. Do not reuse this
assessment for a server or authenticated client. The provider pins this RSA
release candidate for its current RustCrypto generation. Reassess the selection
when a provider release supports stable RSA or a relevant advisory changes.

## Requirements to settle before integration

Two tests deliberately preserve negative qualification evidence:

- ureq starts its connection allowance after target resolution. A delayed resolver
  and delayed TLS handshake can together exceed one connection allowance while
  each phase succeeds. Setting both timeouts to ten seconds does not implement
  the existing cumulative ten-second promise.
- ureq checks header size separately for each informational response and the final
  response. Twenty 4 KiB informational headers followed by HTTP 200 succeed,
  exceeding the existing cumulative 64 KiB promise. A single oversized header is
  rejected, so this is a cumulative-work gap rather than unbounded header storage.

A simpler proposed contract is ten seconds each for resolution and connection,
thirty seconds for the whole transfer, and 64 KiB per response header block.
The whole-transfer deadline still limits slow or repeated informational responses.
These defaults fit one anonymous public-file download and avoid custom resolver,
connector and HTTP-accounting adapters. They are proposals requiring a contract
decision. The current application contract remains unchanged.

Keeping the exact aggregate guarantees is a valid alternative. It requires
qualifying a client exposing both counters, or a private adapter that shares an
absolute connection deadline and counts all header bytes before accepting the
response. Neither behavior may be inferred from this candidate's configuration.
Do not begin replacement while that decision remains open.

FIPS certification, post-quantum negotiation, custom roots, TLS server use and
arbitrary websites are not acquisition requirements. RSA verification remains
necessary for ordinary GitHub chains and future certificate rotation. Do not
remove it merely because an ECDSA-only provider works against another server.

## Other providers and HTTP clients

- rustls's ring and aws-lc-rs providers retain native cryptography and do not meet
  the Rust-only requirement. reqwest does not change that provider choice.
- [rustls-rustcrypto](https://github.com/RustCrypto/rustls-rustcrypto) and the
  [OxiTLS provider](https://docs.rs/crate/oxitls-rustcrypto-provider/0.3.0)
  retain an explicit warning against production use. OxiTLS removes a vulnerable
  historical webpki dependency, but retains that warning and an X25519 completion
  path without contributory-secret rejection. Neither is selected.
- [rustls-pure 0.15.4](https://docs.rs/crate/rustls-pure/0.15.4) intentionally
  rejects every RSA-signed chain. It cannot satisfy GitHub compatibility.
- [Graviola](https://github.com/ctz/graviola) embeds assembler and imposes CPU
  feature minimums beyond Esdieff's platform contract. It is not a solution to a
  portable acquisition requirement because of those CPU minimums, even though building it needs only rustc.
- [rustls-scytale](https://github.com/user-serviceable-parts/scytale/tree/main/rustls-scytale)
  0.10.0-dev.2 pins development rustls 0.24. It adds no
  demonstrated benefit for this acquisition operation over the selected candidate.

## Feature inspection and supported platforms

Inspect the actual normal, build and development graph for each supported target:

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
RustCrypto uses Rust CPU intrinsics and target-specific implementations. In particular, `cmov` 0.5.4 uses inline `asm!` on x86_64 and aarch64. The selected closure is not assembly-free. The application and exercise still forbid first-party unsafe Rust. This qualification interprets Rust-only as excluding external C cryptography, not CPU instructions expressed inside third-party Rust crates.
OS entropy and sockets still use operating-system interfaces. This is not a
claim that the operating system or Rust standard library contains only Rust.

The maintained `scripts/check.sh` runs the exercise on the existing Linux,
macOS and Windows hosted matrix. Target graph inspection alone does not prove
native execution. Recheck the graphs after dependency or feature changes.
