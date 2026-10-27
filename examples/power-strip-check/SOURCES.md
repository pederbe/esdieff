# Power-strip sources and adaptations

The repeated-outlet idea follows the informative outlet-strip composition
example in [RFC 9880, Appendix D.1](https://www.rfc-editor.org/rfc/rfc9880.html#appendix-D.1),
published in January 2026. The explicit bound override follows the ordinary
reuse convention illustrated in
[Section 6.2.1](https://www.rfc-editor.org/rfc/rfc9880.html#section-6.2.1).
These examples supply design ideas. No RFC code component or prose is copied.

Boolean outlet state also draws on the historical
[Binary Switch](https://github.com/one-data-model/playground/blob/41be1e0e564ad39f4d5a1c41fcccef435e388ee9/sdfObject/sdfobject-switch_binary.sdf.json)
from One Data Model playground revision
`41be1e0e564ad39f4d5a1c41fcccef435e388ee9`. The verified external-model pilot
retained its Git blob identity `6261359649925ca90cc20380106f560f6158f948`.
The document declares title Binary Switch, version 2019-02-22 and BSD-3-Clause.
Its exact historical SDF draft edition is unknown. It is a source of interface
ideas rather than a current standards oracle.

The original switch notice is retained here

> Copyright 2018-2019 Open Connectivity Foundation, Inc. All rights reserved.

The original conditions and disclaimer are in
[LICENSE-BSD-3-Clause](LICENSE-BSD-3-Clause), copied unchanged from the
[One Data Model licence at revision 03c51d5df074a12b8202073c85466cf8121b1463](https://github.com/one-data-model/oneDM/blob/03c51d5df074a12b8202073c85466cf8121b1463/LICENSE).
Its Git blob identity is `b56c8b85f1d01f6c922e775632d04136508f0be9`.

All SDF documents, meter constraints, descriptions, namespace names, Check
plans, Sample values and independent expectations here are newly authored
under the repository's MIT or Apache-2.0 licence. The historical switch's OCF
namespace, description text and required declarations are not imported.
`RequiredOutlet` is a separate authored definition whose own absolute
`sdfRequired` pointer shows a copied required declaration relocating to its
importer.

The rich variant adds supplied reusable `sdfData` definitions, exact watt and
watt-hour readings and a narrowed total-power bound. Namespace URIs use the
reserved example.test domain as identifiers. Normal runs retrieve nothing and
use only local relative paths. The missing-dependency plan deliberately names
an absent local file so its acquisition attempt remains visible.
