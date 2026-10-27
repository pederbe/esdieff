# 0006: Scoped reference failures

Esdieff reports a cycle in effective reference expansion as an inability to complete the affected resolution, with its reference path. [RFC 9880, Section 4.4](https://www.rfc-editor.org/rfc/rfc9880.html#section-4.4) supplies no general recursive-model contract, so a syntactic cycle alone is not classified as a definite RFC violation. A reference eliminated by an override does not block the selected model's resolution; a catalogue check still assesses the supplied definitions independently.

Multiple contributors to the same addressed global definition make that lookup ambiguous. Esdieff applies no input-order precedence or automatic deduplication, and reports which contributions conflict. Same-document references, separately addressed unambiguous definitions, and independent checks remain assessable; sharing a namespace is permitted by [Section 3.2](https://www.rfc-editor.org/rfc/rfc9880.html#section-3.2).

A definite reference error, such as malformed reference syntax or a prefix the writing document never mapped, follows from the written reference alone. It is one finding at the reference as written, however many copies, root chains or checked documents reach it. An unavailable target instead leaves each effective position it reaches unfinished, so it is reported at each of them.

## Considered Options

Treating every cycle as an invalid model would impose a prohibition not stated in base SDF. Eagerly expanding content later deleted by a consumer's override could also turn an irrelevant reference into a selected-model failure. Choosing the first or last contributor, or deduplicating identical text, would make meaning depend on input ordering or disregard different reference contexts. Reporting a definite reference error once per reach would count one authored mistake several times.
