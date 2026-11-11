# 0061: Public assessments and bundled registry evidence

Esdieff offers Initial, FullBaseline, ExpandedNumeric and ExpandedConsistency
assessments. FullBaseline includes the modified-date, unit, content-format and
pattern-syntax checks. Their separate public presets are removed so users can
choose basic or comprehensive model checking without learning intermediate
combinations. Numeric and consistency advice remain separate because they add
checks outside the FullBaseline requirements.

Registry checks always use the bundled recorded evidence. The public CLI and
library no longer offer withholding. This removes a policy choice that could
make a check unfinished without providing an ordinary model-checking benefit.
Reports still identify actual consultation and preserve operational failures.
The normal `policy.recorded_registry` value remains `use_recorded`.

This supersedes the standalone assessment choices in ADR 0021, ADR 0027,
ADR 0028 and ADR 0029, and the withholding choice in ADR 0027, ADR 0029,
ADR 0033 and ADR 0038. Their validation rules and recorded facts remain in use.
Removed names are rejected rather than silently mapped to broader checks.
