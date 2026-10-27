# 0004: Supplied catalogues and checking scope

Esdieff initially works with explicitly supplied SDF documents. A [catalogue check](../../CONTEXT.md#model-work) covers every supplied document, including unused definitions; a selected-model check starts from chosen root documents and covers the dependencies needed for its checks. Reports identify their scope so that success for one model cannot be mistaken for success across a catalogue.

Catalogue checking is the default. Selected-model checking requires explicit supplied root documents; other supplied documents remain available for dependency lookup. The CLI accepts explicit file paths, one standard-input document and explicitly supplied local directories, while library callers supply document content and source identities directly. Input order does not assign a document's role, and acquisition failures remain visible.

[Directory discovery](../directory-discovery.md) prepares ordinary Source inputs privately in the CLI. It selects immediate regular `.sdf.json` files by default, with explicit recursion and literal relative file or subtree exclusions. Descendant links and special files are skipped. Each directory must select a nonempty set, and discovery overlap with another discovered or explicit file is refused by native filesystem identity. Explicitly repeated file inputs remain separate attempts. Sorting gives repeatable source presentation, never namespace precedence or an implied Root document.

The entire finite selection, identity checks, root selection and source-metadata preflight complete before content acquisition. A preparation failure is an invocation error without a partial model report. After preparation, unavailable or malformed selected files remain ordinary acquisition results. Discovery observes the local filesystem without promising an atomic snapshot. These limits follow [ADR 0009: Bounded processing and reporting](0009-bounded-processing-and-reporting.md), while reports retain [ADR 0013: Shared workflow and report guarantees](0013-shared-workflow-and-report-guarantees.md). Saved Check plans retain literal version-1 sources.

No automatic document retrieval is required for this initial model workflow. Namespace URIs identify names rather than document acquisition locations, and multiple documents can contribute to one namespace ([RFC 9880, Sections 3.2 and 4](https://www.rfc-editor.org/rfc/rfc9880.html#section-3.2)). Additional acquisition workflows can be considered separately.

## Considered Options

Implicit retrieval would couple semantic processing to network availability and an acquisition convention that base SDF does not require. Checking only reachable content would also miss mistakes in unused documents during a catalogue check. Explicit document supply and scope make those differences visible.
