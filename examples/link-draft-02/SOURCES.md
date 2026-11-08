# Link example sources

The five model files are extracted from Section 2 and Appendix A of
`draft-ietf-asdf-sdftype-link-02`, "An sdfType for Links", by Carsten Bormann
and Ari Keränen, published on 23 June 2026.

- [Published text](https://www.ietf.org/archive/id/draft-ietf-asdf-sdftype-link-02.txt)
- [Published XML](https://www.ietf.org/archive/id/draft-ietf-asdf-sdftype-link-02.xml)

The text's SHA-256, recorded in reports, is
`27c42209d743d020a26c6d5a35b0c70b80e07beeb149db674081c9809929c95d`. The XML
source's SHA-256 is
`4cfd01c4ff6497719b74be98349ae3e7bdda17486ba6f3135208c897a78a43ac`.

Each file contains the JSON text of one XML `sourcecode` element, from the
opening brace through the closing brace and a final newline. The two Appendix
A.2 examples are folded in the draft under the single-backslash strategy of
[RFC 8792](https://www.rfc-editor.org/rfc/rfc8792.html#section-7). Their
`NOTE` header line and the following blank line are removed, and each line
ending in a backslash is joined with the next line after its leading spaces.
No other fields, values or indentation are changed. The fragments are not
wrapped in invented document metadata.

| File | Draft location | SHA-256 |
| --- | --- | --- |
| `section-2-link.sdf.json` | Section 2 | `cb15a72759d8b0320f3be6d4fd5a79c8f1e150ca101bc72564a3e8ffe04ff43c` |
| `my-object.sdf.json` | Appendix A.1, first example | `770b54094057af93f4b936cdd6cef749ff3301828f3d1b6b2da27c1dd3c196dc` |
| `org-types.sdf.json` | Appendix A.1, second example | `49e2da34d5d5d5403b0b4013783f1245f217685cdf63d6b6391cab735a92ae29` |
| `test-object.sdf.json` | Appendix A.2, first example, unfolded | `ebdbad2619a6a1e5aea255d7061b82a90829748fb2784ba1bec8f34a5bba9581` |
| `oma-types.sdf.json` | Appendix A.2, second example, unfolded | `92b99d2ee1ba70428df6be13328543de31ffc7489e726bb3958454b2738711bf` |

These IETF Code Components are covered by the
[Revised BSD license](LICENSE-BSD-3-Clause) under
[Section 4 of the IETF Trust Legal Provisions](https://trustee.ietf.org/documents/trust-legal-provisions/tlp-5/#:~:text=4.%20License%20to%20Code%20Components).
Copyright (c) 2026 IETF Trust and the persons identified as authors of the code.
All rights reserved.

The `info` blocks inside the Appendix A.2 examples are illustrative model
content and do not replace the IETF Code Component license. The JSON sample
values, the Check plan, the regression tests and the explanatory prose are newly
authored under the repository's [MIT](../../LICENSE-MIT) or
[Apache-2.0](../../LICENSE-APACHE) license.
