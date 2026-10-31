# How Esdieff works

Follow a model from supplied files to checks, comparisons and test values. Esdieff
uses the same model operations in its command-line tool and Rust library. A
**catalogue** retains the documents you supply and their acquisition attempts.
An **effective view** presents content after supported name-reference composition
and refinements. Established implicit defaults remain separate explanations and
typed characteristics. They are not inserted into the composed JSON.

[Explore the interactive process map](https://esdieff.com/docs/guides/how-esdieff-works).
Choose a workflow and an example. The address keeps your selection, so you can
share a link to the case you are discussing.

<div id="esdieff-process-map"></div>

<p id="esdieff-process-static"><img src="../images/process-overview.svg" alt="Local model files or explicitly acquired snapshots form a catalogue. Source inspection reads original content. Effective views resolve supplied definitions. Model checks, revision comparison and value testing produce results with evidence."></p>

## Supply the files

Supply the model and the shared definitions it uses as local files. The optional
`acquire` command retrieves one explicitly selected public GitHub file at a
pinned commit and verifies its bytes against an independently established
SHA-256. Acquisition does not parse or validate the model.

The subsequent model operations run offline. Namespace URIs identify supplied
definitions and do not trigger downloads. The catalogue retains the original
content, exact numbers and source locations so findings can point back to what
you supplied.

See [supplied source documents](../source-workflow.md) and
[pinned document acquisition](../acquisition-contract.md).

## Check and inspect

Choose the check's scope and assessment. The scope is either every supplied
model document or selected root documents and their needed dependencies. The assessment selects the
requested rules, from the initial checks to expanded profiles and the full
recorded model baseline.

Esdieff assesses source structure, establishes effective definitions and checks
the meaning it can establish. Findings retain source or effective locations and
the origins behind composed content.

- **Passed** means no failing finding and all requested work completed.
- **Failed** means a definite error, or a warning when strict warnings are enabled.
- **Undetermined** means requested work is unfinished and there is no failing finding.

Completeness is reported separately. A failed check can also contain unfinished
work. A passed initial assessment does not establish every RFC obligation.

Inspection is a separate operation. The source view reads a definition as
written, without requiring a passed model check. The effective view shows the
composed definition and where its qualities came from.

Try [inspect and check a model](inspect-and-check.md). The
[model meaning contract](../model-meaning-contract.md) defines the assessments.

## Compare revisions

Supply the earlier and later revisions. Esdieff matches documents by source key
and corresponding definitions by their positions, then compares their effective
content. A change in a shared definition appears where the interface uses it.

Comparison takes the side of a client built against the earlier model. Client
writes and action inputs carry data toward the device. Readings, action outputs
and event outputs carry data toward the client. A change can be safe in one
direction and breaking in the other.

Each difference is **breaking**, **compatible** or **undetermined**. For example,
raising a readable temperature's maximum from 30 to 35 allows the device to send
35 to an old client that expects at most 30. That value is a breaking witness.
An undetermined difference has neither an established break nor a compatibility
proof.

Optional acknowledgements record intended differences. They retain each
classification while leaving only unacknowledged differences to decide the
command's pass, fail or undetermined disposition.

Try [compare two revisions](revision-comparison.md), or read the
[revision comparison contract](../revision-comparison-contract.md).

## Test values

Three workflows use Sample-value checking against effective data descriptions:

- `sample` checks one supplied JSON value.
- `generate` proposes representative, boundary and invalid candidates, then
  checks each one. The settled admission determines the generated expectation.
- `trace` checks each entry of recorded or generated JSON Lines independently,
  against a model composed once for the request.

A value's admission is **admitted**, **rejected** or **undetermined**. A trace
entry compares that admission with its expectation, which defaults to admitted.
For example, 31 is rejected by a temperature description capped at 30. If the
entry expects rejection, the test passes.

Generated JSON Lines can be checked against another revision to find changed
expectations. Trace checks add no timing, sequence or protocol meaning.

Try [test values against a model](testing-values.md). The
[sample](../sample-contract.md), [generation](../generation-contract.md) and
[trace](../trace-contract.md) contracts define the results.

## Author and edit

`author` creates a new `sdfThing` from supplied reusable definitions, keeping
references and applying the requested refinements. `edit` replaces or removes
specified qualities in an existing document. Both produce a candidate while
leaving the original inputs unchanged.

Esdieff establishes **fidelity**, meaning the intended references, refinements or
edits were preserved correctly. If you request a model check, it runs through the
normal assessment engine on the generated result. Document output requires
complete generation, established fidelity and a passed operation outcome.
Candidate availability alone does not establish fidelity or model validity.

The resulting SDF can enter the same inspection, checking and comparison
workflows.

Try [compose and edit models](authoring-and-editing.md), or read the
[authoring](../authoring-contract.md) and [editing](../editing-contract.md) contracts.
