# 0060: Link sdfType draft support

## Context

[An sdfType for Links, revision 02](https://www.ietf.org/archive/id/draft-ietf-asdf-sdftype-link-02.txt),
`draft-ietf-asdf-sdftype-link-02`, dated 23 June 2026, registers `sdfType: "link"`.
A link value is a JSON object whose members are named after Web Link
([RFC 8288](https://www.rfc-editor.org/rfc/rfc8288.html)) target attributes,
with `href` carrying the target. Its Section 2 says members are strings, while
its Appendix A examples declare integer members, give one link no `href` and
spell ranges as `minvalue` and `maxvalue`. The Context draft adopted in
[ADR 0046: Inline Context draft support](0046-inline-context-draft-support.md)
uses links in its Figure 6, and the digital twin draft uses the same pattern.
Before this decision every link-bearing description was inspectable, but its
meaning was unavailable. This record supersedes the statement in ADR 0046 that
general `sdfType: "link"` meaning remains unavailable. The rest of ADR 0046,
including its nullability boundary, is unchanged.

## Decision

Esdieff records `draft-ietf-asdf-sdftype-link-02`, whose text has SHA-256
`27c42209d743d020a26c6d5a35b0c70b80e07beeb149db674081c9809929c95d`, as
deliberately adopted draft support alongside the
[recorded standards baseline](0002-recorded-standards-baseline.md). The meaning
applies to every full data description, including Context, Properties, `sdfData`
and nested `properties` entries. It adopts these readings, which author feedback
can revise later.

1. **Link description.** Link meaning applies when the description declares
   `type: "object"`, as the registration in Section 5 gives. With another or
   absent `type`, or together with `format` or `contentFormat`, link meaning
   stays unavailable. Value conclusions are then undetermined, and so are model
   conclusions that need the meaning, as for the other `sdfType` conventions.
2. **`href` is optional.** Link meaning does not require `href`. It is required
   only when the description's `required` lists it, as for any other entry.
   Appendix A.2's `objlink` declares no `href`, and Figure 6 of the Context draft
   does not require it.
3. **Other members are admitted.** A link value may carry members other than
   `href` and the declared ones, as any map value may. RFC 8288 target
   attributes are open-ended, so closing a link to its declared members would
   reject ordinary links.
4. **Untyped members are strings.** A member whose effective description gives
   no `type`, including a member with no description at all, must be a JSON
   string, following Section 2. A declared `type` governs its member instead,
   following the integer members of Appendix A.
5. **Member names are lowercase.** RFC 8288 attribute names are
   case-insensitive, and Section 2 requires their lowercase form. A member name
   containing an ASCII uppercase letter is rejected. Attribute values are not
   checked for case, because whether a value is case-insensitive depends on the
   attribute and, for relation types, on whether the type is registered. Reports
   list this limit.
6. **`minvalue` and `maxvalue` are undeclared qualities.** They receive the usual
   `quality.unfamiliar` warning and no range meaning under
   [ADR 0037: Undeclared extension vocabulary](0037-undeclared-extension-vocabulary.md).
   Neither SDF nor the draft defines them.
7. **Nullability is unchanged.** Link meaning constrains nonnull values. A null
   value follows the description's nullability, so a Context link with absent
   `nullable` keeps the unresolved default of ADR 0046, and a Property link keeps
   the base-SDF default of
   [ADR 0005: Nullability and presence](0005-nullability-and-presence.md).

Selected-value checking, generation and trace replay share these rules. Link
conclusions belong to the existing `sample-abstract-values/1` area, beside
`byte-string` and `unix-time`. Revision comparison classifies changed
constraints inside a link description like other data constraints, except that
an untyped member is a string. Adding or removing a member's `type` or
description therefore changes between string and that type, and witnesses settle
it. A change
between `link` and another convention is breaking, and adding or removing `link`
is breaking with a Breaking witness, a map with an uppercase member name.
Contract `esdieff.revision-comparison/10` supersedes `/9` for these
classifications, and old undetermined acknowledgements do not match the newly
classified rows. Reports
identify the draft revision, digest and limits in `baseline.extensions`, beside
the Context draft. Following links, resolving `href` targets and the undefined
`oma-object` URI scheme remain outside this support.

## Considered options

Requiring `href` in every value would reject Appendix A.2's `objlink`. Admitting
only declared members would reject the target attributes RFC 8288 leaves open.
Applying the string rule to every member would reject Appendix A's integers,
while dropping it would ignore Section 2. Reading `minvalue` and `maxvalue` as
`minimum` and `maximum` would invent an alias neither specification defines, and
treating them as a blocking draft error would withhold the link meaning the rest
of the example supports. Checking the case of every attribute value would reject
case-sensitive `href` and extension relation URIs. Making link values nonnull,
or applying a default to Context links, would resolve a nullability question the
link draft does not address.
