# Follow a shared definition through a model change

Use this walkthrough to find which Property declarations reuse a data definition,
explain a local override, and compare the client-visible effects of changing the
shared definition. It assumes Esdieff is [installed](../getting-started/README.md)
and you are working from the repository root.

## Identify the definitions and qualities

The [shared-definition example](../../examples/shared-definitions/shared.sdf.json)
defines `Temperature` in an `sdfData` group. Its data qualities specify numeric
values with a minimum of -20 and a maximum of 100.

The [referencing document](../../examples/shared-definitions/devices.sdf.json)
contains the `sdfObject` definitions `Normal` and `Cool`. Each declares a Property
with the Given Name `temperature`. Both Property definitions carry an `sdfRef`
name reference to `Temperature`. `Cool` also supplies a local `maximum` of 40.

An `sdfData` definition describes reusable data constraints and semantics. The
Property declarations describe elements of state within the enclosing groupings.
These roles follow [RFC 9880 Section 2.2.2](https://www.rfc-editor.org/rfc/rfc9880.html#section-2.2.2).
The `maximum` entry is a quality, while `temperature` names a Property.

Composition processes `sdfRef` by copying the referenced definition and applying
the referencing definition's JSON Merge Patch, as specified in
[RFC 9880 Section 4.4](https://www.rfc-editor.org/rfc/rfc9880.html#section-4.4).
Consequently, `Normal` reuses the shared maximum and `Cool` overrides it. The
namespace URI identifies the shared names. Esdieff resolves them from supplied
documents without fetching that URI.

## Save the earlier revision

Create two directories for the revisions, then copy the existing example into
the earlier one:

```sh
mkdir -p reuse-walkthrough/earlier reuse-walkthrough/later
cp examples/shared-definitions/shared.sdf.json examples/shared-definitions/devices.sdf.json reuse-walkthrough/earlier/
```

Generate a reference for the shared definition and its effective uses:

```sh
esdieff reference --uses --document reuse-walkthrough/earlier/shared.sdf.json --pointer /sdfData/Temperature reuse-walkthrough/earlier/shared.sdf.json reuse-walkthrough/earlier/devices.sdf.json > reuse-walkthrough/earlier-uses.md
```

Open `reuse-walkthrough/earlier-uses.md`. In **Shared definition uses**, find the
effective occurrences ending in `/temperature/maximum`:

| Effective pointer in `devices.sdf.json` | Effective value | Origin shown by Esdieff |
| --- | --- | --- |
| `/sdfObject/Normal/sdfProperty/temperature/maximum` | 100 | `inherited`, from `shared.sdf.json` |
| `/sdfObject/Cool/sdfProperty/temperature/maximum` | 40 | `override`, from `devices.sdf.json` |

Follow the source links for each value. **Previous source** on the override leads
to the replaced shared quality. **Importing consumer** leads to the authored
`sdfRef` location. These source coordinates are separate from the effective
pointers where composition presents the values.

The reference view uses Esdieff's nearest retained source explanations. It covers
supplied documents and retained contributors. Multiple overrides can replace
earlier contributors, leaving some transitive uses unavailable. See the
[model reference contract](../model-reference.md#explore-shared-definitions)
for its completion and provenance limits.

## Change one shared quality

Keep the referencing document unchanged and raise the shared maximum to 120:

```sh
cp reuse-walkthrough/earlier/devices.sdf.json reuse-walkthrough/later/
sed 's/"maximum": 100/"maximum": 120/' reuse-walkthrough/earlier/shared.sdf.json > reuse-walkthrough/later/shared.sdf.json
```

Explore the later revision independently:

```sh
esdieff reference --uses --document reuse-walkthrough/later/shared.sdf.json --pointer /sdfData/Temperature reuse-walkthrough/later/shared.sdf.json reuse-walkthrough/later/devices.sdf.json > reuse-walkthrough/later-uses.md
```

`Normal` now has an effective maximum of 120. `Cool` still has 40, because its
composition patch continues to override that quality. Both retain a minimum of
-20. For each reference, supply the documents from the revision you are
exploring.

## Compare the effects on clients

Compare the two directories so each revision includes both the shared definition
and its referencing document:

```sh
esdieff diff --old reuse-walkthrough/earlier --new reuse-walkthrough/later --color never
```

The command reports:

```text
Revision comparison esdieff.revision-comparison/4: breaking
Client interactions only. Context metadata compatibility is not assessed.
Documents: 2 compared, 0 removed, 0 added
Differences: 1 breaking, 0 undetermined, 0 compatible
Breaking differences:
  changed quality maximum (client-supplied and thing-supplied data)
    witness 120 for thing-supplied data: earlier rejected, later admitted
    earlier devices.sdf.json /sdfObject/Normal/sdfProperty/temperature/maximum
      from shared.sdf.json /sdfData/Temperature/maximum
    later devices.sdf.json /sdfObject/Normal/sdfProperty/temperature/maximum
      from shared.sdf.json /sdfData/Temperature/maximum
```

An exit status of 1 is expected for this breaking comparison. Documents correspond
by their paths relative to the revision directories. Keeping `shared.sdf.json`
and `devices.sdf.json` at the same relative paths lets Esdieff compare their
effective definitions.

The Property declarations omit `readable` and `writable`, whose specified defaults
are both true under [RFC 9880 Section 5.2](https://www.rfc-editor.org/rfc/rfc9880.html#section-5.2).
A client can therefore receive Property data from the Thing and supply data to
it. Esdieff calls these directions **Thing-supplied data** and
**Client-supplied data** in its [revision comparison vocabulary](../../GLOSSARY.md#revision-comparison).

Under [Esdieff's comparison rules](../revision-comparison-contract.md), allowing
the Thing to supply 120 can break a client built for the earlier maximum of 100.
The witness demonstrates that difference. These compatibility classifications
are Esdieff's rules. RFC 9880 defines composition and Property characteristics
without defining a revision-compatibility procedure.

The report locates the difference at `Normal`'s Property, then identifies the
shared quality as its source. `Cool` has no difference because its effective
maximum remains 40. This connects the source explanation to the compatibility
effect, even though `devices.sdf.json` itself did not change.

If the change is intentional, continue with
[reviewing and acknowledging intended differences](revision-comparison.md#acknowledge-the-changes-you-intend).
