# Lock sources and adaptations

The interface ideas come from the experimental One Data Model playground at
revision `41be1e0e564ad39f4d5a1c41fcccef435e388ee9`, also used by the
existing external-model pilot. The source documents are historical examples
whose exact SDF draft edition is unknown, rather than a current RFC 9880
conformance oracle.

| Source | Original metadata | Adaptation here |
| --- | --- | --- |
| [Lock](https://github.com/one-data-model/playground/blob/41be1e0e564ad39f4d5a1c41fcccef435e388ee9/sdfObject/sdfobject-lock_status.sdf.json) | Title Lock, version 2019-02-15, BSD-3-Clause | Adapt the Locked/Unlocked text enumeration into lowercase state values and add jammed in the rich variant |
| [Lock Code](https://github.com/one-data-model/playground/blob/41be1e0e564ad39f4d5a1c41fcccef435e388ee9/sdfObject/sdfobject-lock_code.sdf.json) | Title Lock Code, version 2019-02-15, BSD-3-Clause | Re-express the text-code idea as one credential Property and an authorization input, adding explicit length and pattern constraints |

Both source documents retain this original notice

> copyright 2016-2017, 2019 Open Connectivity Foundation, Inc. All rights reserved.

The original licence conditions and disclaimer are retained in
[LICENSE-BSD-3-Clause](LICENSE-BSD-3-Clause), copied unchanged from the
[One Data Model licence at revision 03c51d5df074a12b8202073c85466cf8121b1463](https://github.com/one-data-model/oneDM/blob/03c51d5df074a12b8202073c85466cf8121b1463/LICENSE).
Its Git blob identity is `b56c8b85f1d01f6c922e775632d04136508f0be9`.

All model prose, numeric PIN and Unicode token constraints, Action maps,
timestamped Event descriptions, faulty attempts bound, limited credential
namespace, Check plans, acknowledgements, samples and expectations are newly authored here under
the repository's MIT or Apache-2.0 licence. No external document bytes or
authentication implementation are imported. The source's code array, OCF
namespace and absolute `sdfRequired` declarations are not part of these models.

The credential namespace uses the reserved example.test domain as an identifier
and is never retrieved. Ordinary checking uses only the supplied local files.
