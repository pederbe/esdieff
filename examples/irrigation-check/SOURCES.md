# Irrigation sources and adaptations

The models, Check plans, samples, descriptions and independent expectations
are newly authored for this repository under its
[MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE) licence.
No external irrigation model, simulator implementation or vendor data is
imported. Their source revision is the repository commit containing these files.

The recorded standards source is [RFC 9880](https://www.rfc-editor.org/rfc/rfc9880.html),
particularly [Appendix C.4](https://www.rfc-editor.org/rfc/rfc9880.html#appendix-C.4)
for array counts and uniqueness,
[Appendix C.5](https://www.rfc-editor.org/rfc/rfc9880.html#appendix-C.5)
for required map entries and described values, and
[Section 4.7.2](https://www.rfc-editor.org/rfc/rfc9880.html#section-4.7.2)
for Full Choice alternatives. No RFC example text is copied.

The zone names, four-zone capacity, one-to-sixty duration range, percentage
feedback and configuration results are authored demonstration constraints.
They establish no device interoperability or relationship between identifiers
and zone records. The documentary duration default is never inserted into
supplied samples.

The limited variant adds an optional calibration reference under the reserved
example.test domain. No dependency is supplied and the identifier is never
retrieved. It deliberately separates absent optional data, needed unavailable
child meaning and independent required-entry rejection. Historical SDF dialects
and optional extension semantics are not imported or resolved.
