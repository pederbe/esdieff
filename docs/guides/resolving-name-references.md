# Resolve missing and ambiguous name references

Follow an `sdfRef` name reference from its authored source to the definition it
needs, then correct the supplied catalogue. This walkthrough assumes Esdieff is
[installed](../getting-started/README.md) and you are at the repository root.

## Identify the name being referenced

Copy the existing shared-definition example into a working directory:

```sh
mkdir -p reference-walkthrough
cp examples/shared-definitions/devices.sdf.json examples/shared-definitions/shared.sdf.json reference-walkthrough/
```

In `devices.sdf.json`, the `sdfObject` definitions `Normal` and `Cool` each declare
a Property named `temperature`. Their `sdfRef` quality contains the CURIE
reference `shared:#/sdfData/Temperature`. The `shared` namespace prefix maps to
`https://example.test/shared`, so the referenced global name is:

```text
https://example.test/shared#/sdfData/Temperature
```

In `shared.sdf.json`, `defaultNamespace` selects that namespace for contributed
global names, including the `Temperature` definition in its `sdfData` group.
These terms follow [RFC 9880 Section 3.2](https://www.rfc-editor.org/rfc/rfc9880.html#section-3.2)
and [Section 4.3](https://www.rfc-editor.org/rfc/rfc9880.html#section-4.3).
Esdieff looks for supplied contributors to the global name. It never fetches the
namespace URI.

## Diagnose an unavailable target

Request a check with only the referencing document supplied:

```sh
esdieff check --color never --root reference-walkthrough/devices.sdf.json reference-walkthrough/devices.sdf.json
```

The result is `undetermined`, with exit status 3. The report includes this
excerpt for `Normal` and a separate finding for `Cool`:

```text
reference.unavailable: supplied global target unavailable
effective 1 "/sdfObject/Normal/sdfProperty/temperature"
  related source 1 "/sdfObject/Normal/sdfProperty/temperature/sdfRef"
  related source 1 "/sdfObject/Normal/sdfProperty/temperature"
```

The effective location identifies the Property whose composition could not
finish. The related source location ending in `/sdfRef` identifies the authored
name reference. Source 1 is the first supplied document, as listed in the report.

Inspect that source entry without requesting reference resolution:

```sh
esdieff inspect --color never --view source --document reference-walkthrough/devices.sdf.json --pointer /sdfObject/Normal/sdfProperty/temperature/sdfRef reference-walkthrough/devices.sdf.json
```

The inspection shows `"shared:#/sdfData/Temperature"`. A completed source
inspection establishes the retained entry, independently of whether its target
can be resolved.

For an unavailable target, check the following against the authored reference:

- The prefix maps to the intended namespace URI in the document that wrote it.
- The contributing document selects the intended `defaultNamespace`.
- The JSON Pointer selects the intended definition, including the exact,
  case-sensitive Given Name.
- The catalogue contains the contributing document and the dependencies needed
  to resolve that definition.

Here the contributor is simply missing. Supply it explicitly:

```sh
esdieff check --color never --root reference-walkthrough/devices.sdf.json reference-walkthrough/devices.sdf.json reference-walkthrough/shared.sdf.json
```

The requested initial assessment now passes with exit status 0 and no incomplete
checks. The example still has a `metadata.info_absent` warning because it omits
an information block. With the default warning policy, that warning leaves the
check passed. This result covers the requested assessment, as described in
[Inspect and check a model](inspect-and-check.md#choose-how-much-to-check).

## Diagnose competing contributors

Create a second copy of the shared document, then supply both contributors:

```sh
cp reference-walkthrough/shared.sdf.json reference-walkthrough/duplicate.sdf.json
esdieff check --color never --root reference-walkthrough/devices.sdf.json reference-walkthrough/devices.sdf.json reference-walkthrough/shared.sdf.json reference-walkthrough/duplicate.sdf.json
```

The result is again `undetermined`, with exit status 3. The report now identifies
both conflicting contributions:

```text
reference.ambiguous: multiple supplied contributors address this global definition
effective 1 "/sdfObject/Normal/sdfProperty/temperature"
  related source 1 "/sdfObject/Normal/sdfProperty/temperature/sdfRef"
  related source 1 "/sdfObject/Normal/sdfProperty/temperature"
  related source 2 "/sdfData/Temperature"
  related source 3 "/sdfData/Temperature"
```

Sources 2 and 3 contribute the same addressed definition. Esdieff keeps that
lookup ambiguous even when the contents match. It gives neither input-order
precedence nor automatic deduplication. These are Esdieff's resolution boundaries,
recorded in [Scoped reference failures](../adr/0006-scoped-reference-failures.md).

Multiple documents may contribute to one SDF namespace. Sharing the namespace
URI alone is permitted by RFC 9880 Section 3.2. The ambiguity here concerns two
contributors to the same global name. Renaming a prefix in both the namespace
mapping and the reference, while retaining the URI, still selects that same name.

Choose the intended contributor for the model revision and supply it once. In
this example, leave the duplicate out of the command:

```sh
esdieff check --color never --root reference-walkthrough/devices.sdf.json reference-walkthrough/devices.sdf.json reference-walkthrough/shared.sdf.json
```

The check passes again. Keeping the duplicate file on disk does not affect this
explicit-input command. A [directory-based check](../directory-discovery.md)
needs corresponding file selection or exclusion so it supplies the intended
catalogue too.

## Confirm the effective definition

Inspect the Property with its dependency available:

```sh
esdieff inspect --color never --view effective --document reference-walkthrough/devices.sdf.json --pointer /sdfObject/Normal/sdfProperty/temperature reference-walkthrough/devices.sdf.json reference-walkthrough/shared.sdf.json
```

The effective inspection contains `type: "number"`, `minimum: -20` and
`maximum: 100`. Its source explanations identify the shared definition and the
importing `sdfRef`. Continue with
[following a shared definition through a change](shared-definition-changes.md)
to explore other uses and compare revisions.

If the report instead contains `reference.prefix_unmapped`, fix the namespace
mapping in the document that wrote the reference. That is a definite authored
reference error. The unavailable and ambiguous target cases above describe
resolution Esdieff could not finish. The
[supplied-source contract](../source-workflow.md) details the distinction and
other reference limits.
