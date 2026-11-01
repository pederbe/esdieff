# 0057: Supplied models without network acquisition

Esdieff works with supplied local model files and caller-provided bytes. Obtaining
those files belongs to the caller's existing tools. The `acquire` command and
its receipt format are removed, superseding
[ADR 0035: Explicit pinned document acquisition](0035-explicit-pinned-document-acquisition.md).
Downloading and verifying a public file requires no SDF knowledge, and no
established model workflow justifies maintaining a transport inside Esdieff.

Local file input and descriptive source metadata remain useful to model work.
They do not retrieve URIs or unsupplied references. A proposed capability should
identify the SDF-specific work it performs and the recurring user task it serves
before adding general-purpose infrastructure or an implementation ticket.
