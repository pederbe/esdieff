# Civil date and time sample contract

This specifies the conventional representation and established temporal meaning portion of [the selected-sample operation](sample-contract.md), under [ADR 0018: Civil date and time sample conventions](adr/0018-civil-date-time-sample-conventions.md). It applies after the selected effective data description, applicable format, conventional JSON value and decoded scalar text have been established. It does not select an alternative, acquire model data, infer a date from neighboring sample fields, or certify the whole model. [RFC 9880 Appendix C.2](https://www.rfc-editor.org/rfc/rfc9880.html#appendix-C.2) supplies abstract data meanings; [RFC 3339 Section 5.6](https://www.rfc-editor.org/rfc/rfc3339.html#section-5.6) names their conventional productions.

FullBaseline can establish the known `date`, `date-time` and `time` model
interpretations without a supplied value. This M049 observation neither invokes
the temporal evaluator nor consults leap-second knowledge. Actual constants and
selected values still require their own established denotation under the rules
below. An unavailable exceptional slot can leave constant meaning unfinished
while the named model interpretation remains established.

## Conventional values and calendar

| SDF format | Conventional decoded JSON string | Established meaning |
| --- | --- | --- |
| `date-time` | `YYYY-MM-DD` + T/t + full-time | Absolute point in civil time with its supplied offset annotation. |
| `date` | `YYYY-MM-DD` | Gregorian civil calendar date, without a time or timezone. |
| `time` | `HH:MM:SS[.digits]` + offset | Date-free clock coordinate with its supplied offset annotation, not a unique dated instant or a recurring schedule. |

Every numeric component uses ASCII digits and the displayed fixed widths. Source year is 0000..9999; month 01..12; day 01 through its Gregorian month limit; clock hour 00..23 and minute 00..59. February has 29 days when the year is divisible by 4, except a century not divisible by 400. The same rule applies to source year 0000, as the published [RFC 3339 Section 2 and Appendix C arithmetic](https://www.rfc-editor.org/rfc/rfc3339.html#appendix-C) specifies. Gregorian arithmetic is used across the four-digit range without a historical local-calendar cutover. Offset subtraction may use an adjacent internal year outside that textual range; that intermediate result alone does not reject an otherwise permitted source date.

Seconds are 00..59 ordinarily, with 60 and a missing 59 governed below. A fraction is a dot followed by one or more ASCII digits. All digits remain exact; there is no fixed decimal scale, float conversion, nanosecond rounding or truncation. A long fraction may exhaust supported work, which gives undetermined rather than an invalid-format finding. Conventional source bytes and their JSON escape spelling remain retained separately from decoded text and temporal projection.

Z/z and signed offsets `+HH:MM` or `-HH:MM` are admitted, including both zero signs and offset hours through 23 with minutes through 59. The UTC coordinate is local clock minus offset, following [RFC 3339 Section 4.2](https://www.rfc-editor.org/rfc/rfc3339.html#section-4.2); a real-world timezone database is unnecessary. -00:00 denotes known UTC time while the local offset is unknown. Under the recorded RFC 3339 Section 4.3 baseline, Z/z and +00:00 carry the preferred-UTC-reference annotation. Preserve these annotations even though all three spellings use the same zero coordinate for leap-slot checking.

[RFC 9557 Section 2](https://www.rfc-editor.org/rfc/rfc9557.html#section-2) changes the Z annotation, while leaving UTC coordinate and this admission unchanged. That update is not implicitly adopted; a later deliberate baseline update must state it. Bracketed IXDTF timezone/calendar suffixes are not selected representations here. Neither an absent offset nor a timezone name supplies the RFC 3339 `full-time` offset. The `date` format has no offset suffix.

T/t is the selected date-time separator. The RFC 3339 note permits applications to choose another separator, so a space-separated timestamp fails this selected conventional representation without a claim that its abstract instant cannot exist or that every application forbids that syntax. ISO 8601 week dates, ordinal dates, expanded years, compact forms, comma fractions, hour 24, reduced precision and offset seconds are likewise outside these selected named productions. Malformed JSON, A002 duplicate-key refusal and unsupported escaped scalar decoding remain their existing input/context outcomes; temporal checking does not reclassify them as invalid dates. A supplied nonstring is not coerced. Null and conflicting effective type are evaluated under the existing nullable/type/choice contract rather than an unconditional nonstring format rejection.

The metadata quality `modified` has its own RFC 9880 Appendix A grammar allowing full-date or Z-terminated date-time, described by [RFC 9880 Section 3.1](https://www.rfc-editor.org/rfc/rfc9880.html#section-3.1). It does not make these sample formats UTC-only. A model metadata timestamp is not a default date or timezone for a sample.

## Exceptional seconds and recorded knowledge

[RFC 3339 Section 5.7](https://www.rfc-editor.org/rfc/rfc3339.html#section-5.7) makes seconds conditional on leap adjustments and shifts the insertion point by the supplied offset. UTC 23:59 on the final UTC day of a month is the possible adjustment minute. June and December are historical and preferred opportunities, not a silently hardcoded universal sample grammar. Negative adjustments remove second 59; positive adjustments insert second 60. Fractions refer to the represented second and never roll it into the following minute.

For a date-time, derive the UTC minute and UTC date by subtracting the offset, including a change of day, month or year. The source date can be the following local day, such as `2017-01-01T05:44:60+05:45`, whose leap slot is 2016-12-31 UTC.

| Dated slot and recorded knowledge | Second 59 | Second 60 |
| --- | --- | --- |
| Away from a possible UTC month-end adjustment minute | Admitted temporal meaning | Rejected as an impossible insertion slot |
| Known normal minute | Admitted | Rejected, no inserted second |
| Known positive adjustment | Admitted | Admitted, including its fractional portion |
| Known negative adjustment | Rejected, deleted second | Rejected, no inserted second |
| Possible adjustment minute with no established knowledge | Undetermined | Undetermined |

Seconds 00..58 remain available independently of adjustment knowledge. A syntactic second greater than 60 rejects. An impossible insertion slot rejects even outside the recorded knowledge interval because the independent slot rule is established.

Initial deliberately recorded leap knowledge is IERS Bulletin C72, issued 2026-07-06, together with its [Leap_Second.dat transition table](https://hpiers.obspm.fr/iers/bul/bulc/Leap_Second.dat) and [Bulletin C72](https://hpiers.obspm.fr/iers/bul/bulc/bulletinc.dat) text. The inclusive knowledge interval is 1972-01-01 through 2026-12-31 UTC. Each listed +1 transition identifies an insertion on the preceding date; the initial 1972-01-01 offset is not an insertion event. No negative adjustment is recorded there. The bulletin explicitly establishes no insertion at the end of December 2026. The snapshot's 2027-06-28 expiry is not a no-leap promise through that date. Before the interval and after it, needed adjustment facts remain unavailable. Processing never fetches a replacement implicitly. Record the snapshot's byte identity in the standards knowledge basis; adopting new knowledge changes that basis deliberately, following [ADR 0002: Recorded standards baseline](adr/0002-recorded-standards-baseline.md).

The recorded knowledge identity is `iers-bulletin-c72/2026-07-06`. Snapshot SHA256 identities are `6cb6f5d4b819f2e568e25db4b0b26d89dedf031fdffb18bc94d40f4e94e268d7` for Leap_Second.dat and `310e172eadacacca3adf92cb9bd646fcc0d6320e35e996a106d2644ede3b52f0` for Bulletin C72. Its 27 positive insertion dates are:

```text
1972-06-30  1972-12-31  1973-12-31  1974-12-31  1975-12-31
1976-12-31  1977-12-31  1978-12-31  1979-12-31  1981-06-30
1982-06-30  1983-06-30  1985-06-30  1987-12-31  1989-12-31
1990-12-31  1992-06-30  1993-06-30  1994-06-30  1995-12-31
1997-06-30  1998-12-31  2005-12-31  2008-12-31  2012-06-30
2015-06-30  2016-12-31
```

A date-free `time` has no UTC date by design. Ordinary seconds 00..59 are admitted as date-free clock coordinates without asserting occurrence on every calendar day. A possible inserted-second label 60 at UTC 23:59 is undetermined because its eligible leap-label interpretation is unestablished. This is not a missing required date field and does not establish invalidity. Second 60 at any other offset-adjusted UTC minute rejects. This is an explicitly selected conservative interpretation of RFC 3339 `full-time` as imported by SDF. An existential some-valid-date interpretation could admit eligible 60, but the RFC does not select that quantifier, so that alternative is deliberately deferred. The evaluator does not use a host date, a model metadata date or an unstated sample sibling. Date-free comparison, normalization modulo a day and recurring-schedule semantics are not established here.

## Sample results, locations and finite work

The temporal obligation belongs to `sample-abstract-values/1`, keeping the immutable nine-area `esdieff.sample/1` inventory. It is assessed when temporal admission or an independent representation/calendar/slot rejection is established. An unavailable needed interpretation or capability leaves that area undetermined. Applicable decoded JSON, scalar text length and patterns, const equality, nullable/type and alternatives keep their own areas. An established ordinary JSON string type does not complete abstract-values by itself. One complete admitting alternative can supply a witness; rejection still needs proof covering every potentially admitting alternative.

A rejected temporal form contributes an error in the sample domain with primary sample location and a related effective format constraint location, with source contributor locations when available. Its basis distinguishes selected representation convention from an established calendar/slot requirement and recorded leap knowledge. The space-convention condition must not assert an RFC-invalid abstract date-time. Undetermined leap or work contributes incompleteness with its reason and affected locations, not an invented rejection error. The versioned condition catalogue below preserves these distinctions.

For a supplied root JSON string, sample pointer is `""` and its half-open span is the complete original JSON token, including quotes. Nested values use their actual sample pointer and original byte span. Effective location is the selected effective description's applicable `/format` path; it is not a fabricated source location or byte span. Typed JSON inputs have a pointer and `span:null` unless genuine original bytes exist. Inherited or overridden format source locations are related explanations, not replacements for the sample identity. Presentation omissions preserve available identity/span and aggregate conclusions.

The implemented condition codes and bases are:

| Code | Domain / severity | Basis |
| --- | --- | --- |
| `sample.temporal_representation` | sample / error | Interpretation `ADR0018-selected-representation`, supported by RFC 9880 C.2 and RFC 3339 5.6. |
| `sample.temporal_calendar` | sample / error | RFC 3339 5.7 and Appendix C, RFC 9880 C.2. |
| `sample.temporal_clock` | sample / error | RFC 3339 5.6 and 5.7, RFC 9880 C.2. |
| `sample.temporal_exceptional_slot` | sample / error | RFC 3339 5.7, RFC 9880 C.2. |
| `sample.temporal_leap_second` | sample / error | The same slot basis, with recorded C72 identity. |
| `sample.temporal_knowledge_unavailable` | operation / incomplete | Operational `temporal-knowledge`, supported by the slot basis. |
| `sample.temporal_meaning_unavailable` | operation / incomplete | Operational `sample-establishment`, supported by ADR0018 and the selected productions. |
| `operation.limit` | operation / incomplete | Operational `sample-establishment`, with affected abstract-values area. |

Each specialized incomplete condition identifies `sample-abstract-values/1`. It replaces the generic coverage detail for that same cause, including when optional detail is omitted. Independent const, alternative or other unavailable meanings retain their own details; each requested area is still counted once. Incomplete conditions never supply rejection witnesses or sample errors.

`baseline.interpretations` records `ADR0018-selected-representation` when the convention is consumed. `baseline.temporal_knowledge` carries the C72 identity, interval and both snapshot digests only after an actual adjustment query, including outside-interval queries and discarded branches. Dates, ordinary seconds 00..58, second 59 away from a boundary, date-free time and independently impossible insertion slots do not consult C72. Established null and wrong-kind constraints exclude temporal grammar and record neither use flag.

The evaluator precharges `2 * decoded_length + 256` original semantic-work units for the borrowed representation scan, bounded calendar/offset arithmetic and at most 27 recorded insertion comparisons. Both const operands and every potentially relevant branch use that same remaining request allowance. No additional temporal counter, refill, owned fraction or normalized temporal value exists. Metadata `modified` keeps its separate Z-only wrapper and existing precharge.

| Independent isolated-format result | Area counters | Aggregate / admission |
| --- | --- | --- |
| Established good form, all other applicable work complete | assessed 4, not_applicable 5, unassessed 0, undetermined 0 | complete true, errors 0, incomplete_checks 0, passed, admitted |
| Established bad form, all other applicable work complete | assessed 4, not_applicable 5, unassessed 0, undetermined 0 | complete true, errors 1, incomplete_checks 0, failed, rejected |
| Unavailable temporal meaning, other work complete | assessed 3, not_applicable 5, unassessed 0, undetermined 1 | complete false, errors 0, incomplete_checks 1, undetermined, undetermined |

These projections assume exactly one effective full description with `type:"string"`, `nullable:false`, one temporal format and no additional constraints. The four assessed areas are context, type/null, text/Boolean and abstract-values; the five remaining areas are established not applicable. The work-limit projection assumes only the temporal consumer's budget is exhausted after the independent areas complete. They are expected contract observations, not executed product reports. A definite independent constraint rejection plus unavailable temporal meaning gives admission rejected, failed, complete false only when the unknown temporal meaning cannot change that proof. Strict warnings can fail disposition with admitted temporal/sample meaning. Detail truncation never changes admission, counters or aggregate errors.

All source, decoded text, parsed nodes, recursive alternatives, semantic work, location and report limits remain shared under [ADR 0012: Exact numbers and JSON input policies](adr/0012-exact-numbers-and-json-input-policies.md), [ADR 0013: Shared workflow and report guarantees](adr/0013-shared-workflow-and-report-guarantees.md) and [ADR 0016: Selected samples with explicit admission and coverage](adr/0016-selected-sample-contract.md). Charge before scanning, copying or building exact fractions, offset projections and locations. Format capability is bounded independently of JSON source acquisition; raising one cap removes no other bound. No additional unbounded precision, live lookup, timezone database or calendar dependency is selected.

## Representative observations and remaining scope

Under a common established civil format, null equals null and differs from an
established nonnull civil value in either operand order. Malformed or
uninterpreted nonnull values leave comparison undetermined. Null/null consults
neither the nonnull representation convention nor leap knowledge; mixed pairs
record only the nonnull interpretation and knowledge actually used. Null
admission remains independent, including when `nullable:false` excludes it.
Two nonnull dates with established Gregorian denotation compare by calendar date.
The same date is equal and different dates differ, including day, month and year
boundaries across the four-digit source range. The fixed-width convention permits
validated decoded text to supply this comparison without a normalized retained
value. JSON escape spelling does not matter; identical invalid dates do not
establish equality and differing invalid dates do not establish a calendar
disagreement.

Two nonnull date-times with established denotation compare by UTC instant under
ADR 0018. Subtract the numeric offset with Gregorian day/year rollover, keeping
UTC dates outside the source-year edges representable internally. Compare exact
fractional digits after removing trailing zeros; absent and all-zero fractions
are equal. An inserted second keeps its own slot, distinct from second 59 and
the following day's second 00. Offset annotations, letter case and JSON escapes
do not distinguish instants. The RFC 3339 example `1996-12-19T16:39:57-08:00`
equals `1996-12-20T00:39:57Z`; `.1` equals `.100`; an established Z value equals
its +00:00 and -00:00 spellings. Invalid forms and absent needed adjustment
knowledge prevent either an equality or a difference conclusion. Differing
nonnull date-free time representations retain unavailable comparison meaning.

| Format | Admitted temporal representation | Rejected representation / meaning | Undetermined needed work |
| --- | --- | --- | --- |
| `date` | `"0000-02-29"`, `"2000-02-29"`, `"9999-12-31"` | `"1900-02-29"`, `"2024-04-31"`, `"20240229"` | `"2024-02-29"` when required temporal work cannot run within capability |
| `date-time` | `"1996-12-19T16:39:57-08:00"`, `"1996-12-20t00:39:57z"`, `"2017-01-01T05:44:60+05:45"` | `"2024-01-01T24:00:00Z"`, `"2024-06-30T23:59:60Z"`, `"2024-01-01 12:00:00Z"` under the selected convention | `"2030-06-30T23:59:60Z"`, `"2030-06-30T23:59:59.9Z"` with this recorded knowledge |
| `time` | `"12:34:56.000000000000000000001+05:45"`, `"23:59:59Z"`, `"12:34:56-00:00"` | `"24:00:00Z"`, `"12:34:56"`, `"12:34:60Z"` | `"23:59:60Z"`, `"05:44:60+05:45"` with no date |

Temporal denotation alone does not establish sample admission when other constraints remain unfinished. [ADR 0015: Bounded value equality in a resolved data context](adr/0015-value-equality.md) applies separately: invalid or unresolved strings do not denote a temporal value merely because their representations match. Gregorian-date and dated date-time equality supply constant checks, supported compound comparisons, uniqueness and declared-value consistency through that one contract. Actual date comparison records the selected convention without consulting leap knowledge; dated date-time comparison records only actual slot consultations. Model-only comparator capability uses no operands and records no representation consultation. Nonnull date-free time comparison beyond established identical representations, epoch conversion, ordering, duration arithmetic, physical clock correctness, recurring schedules and additional representations remain outside this scope.
