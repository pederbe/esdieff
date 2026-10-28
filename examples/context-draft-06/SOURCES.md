# Context example sources

The five original model files are extracted from Figures 2 through 6 of
`draft-ietf-asdf-sdf-nonaffordance-06`, by Jungha Hong and Hyunjeong Lee,
published on 27 October 2026.

- [Published HTML](https://www.ietf.org/archive/id/draft-ietf-asdf-sdf-nonaffordance-06.html#section-5)
- [Published XML](https://www.ietf.org/archive/id/draft-ietf-asdf-sdf-nonaffordance-06.xml)
- [Publication announcement](https://mailarchive.ietf.org/arch/msg/asdf/sTXB0ZCIZFgfPfwSu1qehN4bgjE/)

The XML source's SHA-256 is
`d788125606f6ad1a5bf68fcdfa9d529b7a9dd1b7c29394d94658aa8dae1ec2cf`.
Each file contains the JSON text between its figure's `sourcecode` tags, from
the opening brace through the closing brace and final newline. No fields,
values, indentation or unit spellings are changed. The four fragments are not
wrapped in invented document metadata.

| File | XML figure anchor | SHA-256 |
| --- | --- | --- |
| `device-with-context.sdf.json` | `fig-sdfcontext-example` | `0592deeb4a0f029fffde70a99df76a2ebb7721895c2b4c2df8deef7a82a71c86` |
| `asset-container.sdf.json` | `fig-asset` | `b0bd5b484c0c46aea8895bf553a33baf3409645012be875517fe3728c2060b86` |
| `environmental-sensor.sdf.json` | `fig-environment` | `801c461517213035caf6c6e8880bff75354b154a3d8ed7173f586ff2cd2733e6` |
| `device-metadata.sdf.json` | `fig-regulatory` | `f1ee58cfad4360d73ee02bb43e21d42851947758d57682739c36c325ceef91c6` |
| `device-with-links.sdf.json` | `fig-sdftype-link` | `4c19614fbcd7e5d39aea248d80812e966faa227bd19f3377e06d1cac2518b57a` |

These IETF Code Components and the derived revision are covered by the
[Revised BSD license](LICENSE-BSD-3-Clause) under
[Section 4 of the IETF Trust Legal Provisions](https://trustee.ietf.org/documents/trust-legal-provisions/tlp-5/#:~:text=4.%20License%20to%20Code%20Components).
Copyright (c) 2026 IETF Trust and the persons identified as authors of the code.
All rights reserved.

Figure 2's `info.copyright` and `info.license` fields are illustrative model
content. Its Example Corp notice and example.com URL do not replace the IETF
Code Component license.

`device-with-context-revised.sdf.json` changes only `info.version` and the
calibration offset's maximum, as described in the [example guide](README.md).
It is a local test case, not a published IETF revision. The Check plan,
regression tests and explanatory prose are newly authored under the
repository's [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE)
license.
