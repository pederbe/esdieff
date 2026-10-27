# 0038: Recorded extension vocabulary

## Context

[RFC 9880 Section 7.5.2](https://www.rfc-editor.org/rfc/rfc9880.html#section-7.5.2)
lets organizations register a Quality Name Prefix and define qualified Quality
Names under it. The IANA SDF Quality Name Prefixes registry recorded its first
prefix, `oma`, on 12 May 2026. The Open Mobile Alliance defines one quality
with it in its OMNA SDF Quality Names registry. `oma:id` is the LwM2M Object or
Resource ID of a definition, and its CDDL adds `"oma:id" => uint` to the RFC
9880 sockets `$$SDF-EXTENSION-OBJECT`, `$$SDF-EXTENSION-DATA` and
`$$SDF-EXTENSION-ACTION`. The `oma2sdf` converter emits it on every converted
Object and Property, and its output for object 3303 writes the Object ID as a
string.

## Decision

Recorded extension vocabulary is a qualified Quality Name whose definition
Esdieff records as dated Recorded registry evidence, following
[ADR 0002: Recorded standards baseline](0002-recorded-standards-baseline.md)
and [ADR 0027: Unit names and recorded registry guidance](0027-unit-names-and-recorded-registry-guidance.md).
The first entry is `oma:id`, recorded from the IANA prefix registry and the
OMNA registry page as accessed on 16 October 2026.

`oma:id` belongs where its registered CDDL sockets apply. That is an sdfObject
definition, an sdfAction definition and every full data description: sdfData,
sdfProperty, Action input and output data, Event output data, object property
entries and choice alternatives. Its value must be a CDDL `uint` under the
formal admission rules of
[ADR 0032: Formal model admission](0032-formal-model-admission.md), so `3303`
and `3303.0` are admitted while `"3303"`, `-1` and `3303.5` are not. An excluded
value at one of these positions is an error whose basis is the OMNA
registration rather than RFC 9880. Reports give it a new `registration` basis
kind, with the registration's citation and the Quality Name as its identifier.
The kind joins report version 1.0 in place, because no report reader predates
the first release. Full assesses it as M028 in its
critical-extension area, so the 54-obligation inventory stays unchanged and the
Initial assessment is unaffected.

At any other position, such as an sdfEvent or sdfThing definition or a Direct
item, `oma:id` is undeclared extension vocabulary under
[ADR 0037: Undeclared extension vocabulary](0037-undeclared-extension-vocabulary.md),
with its usual notice. So is any other `oma:` name. A dated record establishes
what it contains, not that nothing else has been registered since.

The recorded meaning covers value admission only. Esdieff does not check that an
ID names a real LwM2M object, or that IDs are unique within an Object, because
the registration states neither. A caller that withholds recorded registry
evidence leaves the `oma:id` conclusions unavailable, as it does for units.
Base conclusions still proceed past `oma:id` at its positions in every
assessment. Where a report did not consult the recorded evidence, as in a Sample
report, an assessment other than Full or a Full check that withholds it, those
conclusions rely on ADR 0037 and name `undeclared-extensions-ignorable/1`.
Further recorded extensions take the same shape: a registered or published
definition, the positions its CDDL names and its value domain.

## Considered Options

A warning for an excluded value would let a catalogue with broken IDs pass
unless strict warnings were requested, although the registered type makes the
defect definite. Following the four classes in the OMNA table instead of its
CDDL would leave placements its own grammar admits, such as Action input data,
unchecked. Treating other `oma:` names or other positions as errors would turn a
dated record into a claim that nothing else was registered. Reusing the `rfc`
basis kind would avoid a report change but name RFC 9880 as the reason, and a
report version 2.0 would protect no existing reader.
