# Explicit numeric contradiction advice

Select `Assessment::ExpandedNumeric` or CLI `--assessment expanded-numeric-v1`.
The request, inventory and requested-coverage identity are
`esdieff.expanded.numeric/1`. It contains the Initial eight areas plus
`advisory-numeric-three/1`, counted once across the selected model scope.
This follows [ADR 0030: Bounded numeric advice](adr/0030-bounded-numeric-advice.md).

| Warning code | Sufficient local contradiction |
| --- | --- |
| `proof.integer_interval_gap` | Established integer type, finite lower/upper bounds and null exclusion leave no integer between the adjusted endpoints. |
| `proof.numeric_const_bound` | A sole ordinary numeric const is excluded by an applicable ordinary bound. All four surviving bound qualities are checked independently. Established absent type is permitted, with no invented type argument or origin. |
| `proof.step_interval_gap` | Established number/integer type, a positive ordinary numeric multipleOf, finite bounds and null exclusion leave no lattice point. Integer type intersects the step lattice with integers. |

Conditions have Warning severity, Advice basis, their warning code as basis ID,
and RFC 9880 Appendix C.1 as the constraint citation. Strict warnings change
disposition while preserving severity, counts and completion. Failure to find a
warning establishes no general satisfiability, sample admission or restrictive
refinement. Initial, other expanded profiles and Full keep their existing scopes.

## Final meaning and applicability

Only established effective Full data descriptions are evaluated: Property and
sdfData definitions, Action input/output, Event output, properties entries and
derived Choice alternatives. Direct items themselves are excluded. A receiving
Full role does not upgrade unresolved Direct donor meaning. Surviving established
overrides supply their own meaning and original token provenance. Enum text
occurrences remain distinct existing alternatives; they are not parsed as numbers.
Choice and Enum owners receive no union emptiness proof.

| Quality or context | Numeric boundary |
| --- | --- |
| type | Integer enables integer-gap; number/integer enables step-gap and typed const-bound. Proven absence enables const-bound only. Known string/Boolean/array/object types are outside these classes; unknown or malformed relevant type leaves work undetermined. |
| const | A final ordinary numeric root enables const-bound. Any independently established ordinary nonnull sole root kind can exclude null for interval proofs, including a compound root with unavailable deep payload. Unknown identity, donor or interpretation cannot supply that guard. Null remains an authored value where composition has not deleted it. |
| minimum, maximum, exclusiveMinimum, exclusiveMaximum | All surviving ordinary numeric bounds apply. Strongest known endpoints are selected; exclusivity wins equal ties. Unavailable magnitude leaves affected work incomplete while independent known subset contradictions can remain. Boolean exclusivity is not interpreted using older JSON Schema rules. |
| multipleOf | Positive ordinary values enable step-gap. Zero, negative or wrong-kind values retain original parameter findings and are outside that class. Unavailable magnitude leaves the step obligation incomplete without removing an independent integer/const contradiction. |
| nullable | False excludes null. True or default true admits it unless an independent nonnull const root excludes it. Unknown flag leaves dependent interval work incomplete; const-bound needs no false flag. default is never inserted or used as const. |
| sdfType | Any present selector is outside ordinary numeric support and leaves otherwise relevant work undetermined. |
| format | Any present format leaves interval denotation unsupported. For an actual ordinary numeric const only, uri, uri-reference, uuid, date, time and date-time have independently irrelevant text meaning. Other, malformed or unavailable format selectors leave relevant work undetermined. |
| items, properties | Present structure must have independently established applicable array/map meaning. Numeric type or const does not license ignoring it. Established array/map containers are outside numeric classes; separately visited Full descendants can qualify. |
| required | Established object-only data presence is irrelevant after an ordinary numeric domain/root establishes nonobject applicability. It does not establish properties structure. |
| minItems, maxItems, uniqueItems | Established Full array-only constraints are irrelevant after ordinary numeric applicability excludes arrays. Grouping-size meanings belong to grouping roles outside numeric classes. |
| minLength, maxLength, pattern | Established text-only meaning is irrelevant after ordinary numeric applicability excludes text. Pattern matching is not performed here. |
| default, unit, contentFormat | Established documentary or representation-only meanings do not change numeric equality or bounds. Unavailable payload is independent only when meaning and role are established. |
| description, label, $comment | Established metadata is independent in its legal role. label is not admitted as a Direct-item quality. Information metadata, including title/version/modified/copyright/license, is not whitelisted in Full data maps. |
| readable, writable, observable | Established Property permission meaning is independent. Other Full placements leave relevant numeric work undetermined. |
| sdfRequired | Established Property/affordance/group declaration-presence meaning is independent. Data-only, IO, properties-entry and Choice placements remain unsupported for otherwise relevant numeric work. sdfRequiredInputData keeps the historical vocabulary refusal. |
| sdfThing, sdfObject, sdfProperty, sdfAction, sdfEvent, sdfData, sdfInputData, sdfOutputData | Existing legal parent roles govern traversal. Grouping and Action/Event owners are outside the classes; each independently established Full descendant is evaluated. Unavailable child identity/traversal leaves work incomplete. |
| info, namespace, defaultNamespace, features | Existing feature and reference preflight establishes needed context before proofs. Unsupported critical features and unavailable needed bindings cannot be bypassed by numeric spelling. |
| Unclassified qualities, unsupported placement, unknown member/role/donor identity | No presumed irrelevance or restrictive meaning. Dependent work is undetermined; independent established descriptions keep their findings. Optional pointer/span omission alone does not change semantic eligibility. |

## Exact arithmetic and limits

The original exact number token is retained. Signed zero and equivalent decimal
notation compare by mathematical value. Endpoint quotient rounding accounts for
sign, remainder and exclusive integral endpoints. For positive decimal step p/q
in lowest terms, integer values use the common lattice pZ. For example, integer
[1,2] with step 1.5 has a gap, while number [1,2] includes 1.5.

Virtual long-division numerator/denominator widths are bounded at 1024 digits;
adjusted quotient buffers allow 1025, and finite coefficient factor storage 513.
A small-magnitude quotient shortcut precedes virtual-width admission. Large
matching exponents can cancel; a large unmatched exponent can be unsupported.
The integer intersection uses the numerator of the reduced decimal step. Thus
integer [1,2] with step 0.6 has no admitted value, while step 0.2 admits both
integers. A valid source number can exceed the derived quotient width without
being a malformed model parameter.
These are implementation capabilities, not restrictions on the standard's
mathematical numbers. No exponent-sized buffer or exponent-counting factor loop
is used. The original numeric-work and derived-allocation remainders are shared
across classes, models and alternatives, including input work already consumed.

Relevant unavailable meaning, magnitude, work or storage makes the one area
Undetermined. A supported attempt is Assessed even without a warning; complete
traversal with no eligible class is NotApplicable. An operational
`proof.numeric_capability_unavailable` condition uses basis
`expanded-numeric-capability`, its class and reason, and no citation key. The
undetermined area uses `expanded-numeric-establishment`; completed and
not-applicable area rows use Advice basis `advisory-numeric-three/1`.

The additional mandatory report reserve is zero. Coverage rows and located
conditions can be omitted at the original report admission floor; exact nine-area
counts, statuses and outcome remain available. Four operand positions preserve
the existing first three meanings and add step/null-guard provenance where
needed. Presentation omissions do not discard established warnings or hide
unfinished siblings. See the [report contract](report-contract.md).
