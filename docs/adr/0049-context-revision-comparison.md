# 0049: Context revision comparison

## Context

The [Context draft 06, Sections 4.1, 4.4 and 4.5](https://www.ietf.org/archive/id/draft-ietf-asdf-sdf-nonaffordance-06.txt) separates descriptive instance metadata from interactive affordances. Revision comparison under [ADR 0039: Client-side revision comparison](0039-client-side-revision-comparison.md) assesses client interactions. Consumers of Context metadata can have different compatibility requirements.

## Decision

Corresponding Context definitions are paired by Given Name within corresponding Objects and Things, after the existing effective composition. Each addition, removal or structural content change is a separate `sdfContext` difference. Structural comparison retains exact-number equality and ignores map ordering. It does not infer renames or equate different representations of an abstract metadata value.

Available Context differences are compatible within the client-interaction scope. They carry no client data directions, affordance requiredness or endpoint-presence rules. A changed metadata constraint can break a metadata consumer even when this comparison is compatible. Unsupported abstract metadata meaning and unresolved draft defaults gain no compatibility proof. The baseline records the adopted Context draft. The JSON policy names `compatibility_scope: "client_interactions"`, and human and Markdown reports state that Context metadata compatibility is not assessed. SARIF's compatible rule states the same scope.

Context and Property definitions with the same Given Name remain separate. Moving a Property to Context removes the old affordance and retains that breaking difference. A shared `sdfData` definition is compared independently through every Property that uses it, with the usual directions, witnesses and proofs. Direct reuse from Context into a non-Context role remains unavailable under [ADR 0047: Context reuse with existing composition](0047-context-reuse-with-existing-composition.md). This decision adds no new reference capability or default policy.

Before classifying a Context difference, comparison checks its effective subtree for unavailable content. Missing references and unknown critical features retain scoped undetermined results, including when another structural difference is already known. Composition, traversal, exact-number comparison and retained records share the existing operation allowance. Independent completed affordance differences survive unavailable Context and exhausted work.

The comparison identity becomes `esdieff.revision-comparison/3`, superseding `/2` for Context classification. The diff report becomes `1.6`, adding the required compatibility scope and the `sdfContext` subject. Existing direct comparison, Check plans and report projections share the same result.

## Considered options

Treating Context as a Property would invent client interactions and mark metadata-only changes as breaking. Leaving every Context difference unclassified would withhold useful interaction results. Declaring metadata-consumer compatibility would require new directions and policy beyond the existing comparison. Comparing authored Context alone would miss inherited and refined content. Stopping structural comparison at the first difference could conceal an unavailable reference elsewhere in that Context.
