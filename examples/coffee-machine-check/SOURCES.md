# Coffee-machine sources and adaptations

Interface ideas come from Eclipse Thingweb test-things at revision
`34c7548aeaa6109306a9192785689b81cecd7fc1`.
The [advanced coffee-machine Thing Model](https://github.com/eclipse-thingweb/test-things/blob/34c7548aeaa6109306a9192785689b81cecd7fc1/things/advanced-coffee-machine/advanced-coffee-machine.tm.json)
has Git blob identity `7891bb5fa535553587463161d938f263ab939567`.
It is a WoT Thing Model rather than an SDF document or RFC 9880 conformance
oracle. Its title is a deployment placeholder.

| Source idea | Adaptation here |
| --- | --- |
| allAvailableResources with integer percentages 0 through 100 | Author an SDF resource Property with water, beans and optional milk, adding explicit required entries and nonnullability |
| makeDrink quantity 1 through 5 and structured result/message output | Author a brew Action with explicit JSON input and output, required entries and nonempty message |
| Drink and size selection | Author an espresso/latte enum and nested Full Choice recipes with distinct numeric size bounds |

The source's HTTP forms, URI variables, scheduling data, behavioural defaults
and outOfResource Event are not imported. Maintenance reasons, resource Event
output, millilitre size bounds, calibration limitation, all model prose, Check
plans, samples and expectations are newly authored. No simulator code or
original Thing Model bytes are imported, and no automatic conversion is used.

The original project notice is retained unchanged in
[LICENSE-UPSTREAM](LICENSE-UPSTREAM), from
[LICENSE.md at the same revision](https://github.com/eclipse-thingweb/test-things/blob/34c7548aeaa6109306a9192785689b81cecd7fc1/LICENSE.md).
Its Git blob identity is `bf655e69d332af02f5fc4486ac7da052490287fd`.
It identifies Copyright (c) 2018 Contributors to the Eclipse Foundation and
`EPL-2.0 OR W3C-20150513`, linking both licence texts. No additional NOTICE file
is present in the source tree at this revision. Newly authored content uses the
repository's [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE) licence.

The models use Esdieff's recorded RFC 9880 baseline. They import no historical
SDF dialect or protocol extension. The optional calibration identifier uses the
reserved example.test domain and is never retrieved. Ordinary checks require
only local immutable content and no upstream installation.
