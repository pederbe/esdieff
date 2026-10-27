# Lamp sources and adaptations

The interface ideas come from the experimental One Data Model playground at
revision `41be1e0e564ad39f4d5a1c41fcccef435e388ee9`. The source documents
are historical examples whose exact SDF draft edition is unknown. They are
not used as a current RFC 9880 conformance oracle.

| Source | Original metadata | Adaptation here |
| --- | --- | --- |
| [Binary Switch](https://github.com/one-data-model/playground/blob/41be1e0e564ad39f4d5a1c41fcccef435e388ee9/sdfObject/sdfobject-switch_binary.sdf.json) | Title Binary Switch, version 2019-02-22, BSD-3-Clause | Reuse the previously exercised Boolean switching idea, rename the Property to on and explicitly forbid null |
| [Dimming](https://github.com/one-data-model/playground/blob/41be1e0e564ad39f4d5a1c41fcccef435e388ee9/sdfObject/sdfobject-light_dimming.sdf.json) | Title Dimming, version 2019-02-15, BSD-3-Clause | Turn the source's documented default range 0 through 100 into explicit integer brightness bounds, without importing its range or step Properties |

Original Binary Switch notice

> Copyright 2018-2019 Open Connectivity Foundation, Inc. All rights reserved.

Original Dimming notice

> copyright 2016-2017, 2019 Open Connectivity Foundation, Inc. All rights reserved.

The original licence conditions and disclaimer are retained in
[LICENSE-BSD-3-Clause](LICENSE-BSD-3-Clause), copied unchanged from the
[One Data Model licence at revision 03c51d5df074a12b8202073c85466cf8121b1463](https://github.com/one-data-model/oneDM/blob/03c51d5df074a12b8202073c85466cf8121b1463/LICENSE).
Its Git blob identity is `b56c8b85f1d01f6c922e775632d04136508f0be9`.

All model prose, RGB constraints, modes, Choice scenes, Action results, limited
calibration namespace, Check plans, samples and expectations are newly authored
here under the repository's MIT or Apache-2.0 licence. No external document bytes
or simulator code are imported. The source's OCF namespace and absolute
`sdfRequired` declarations are not part of these standalone models.

The calibration identifier uses the reserved example.test domain. It describes
missing supplied content and is never retrieved. Ordinary checking uses only
the local files in this directory.
