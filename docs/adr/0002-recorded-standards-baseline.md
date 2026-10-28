# 0002: Recorded standards baseline

Esdieff uses a recorded [standards baseline](../../GLOSSARY.md#conformance): [RFC 9880](https://www.rfc-editor.org/rfc/rfc9880.html), explicitly adopted verified corrections, and a dated snapshot of relevant registry knowledge. Normative RFC prose and Appendices A and C govern; informative schemas and examples provide supporting evidence. Corrections and registry updates enter the baseline deliberately so that a check's standards basis remains reproducible.

RFC interpretations, agreed deviations, and unimplemented requirements remain distinct. Registry snapshots record available knowledge. A name missing from them is not proved unregistered, and its absence does not justify rejecting otherwise permitted extension content. A recorded extension definition can still establish that a value at one of its own positions is excluded, following [ADR 0038: Recorded extension vocabulary](0038-recorded-extension-vocabulary.md).

## Considered Options

Consulting changing online sources during every check could change conclusions without a corresponding toolkit change. Ignoring later corrections and registrations would instead discard relevant standards evidence. A recorded, deliberately updated baseline preserves both reproducibility and a path to updates.
