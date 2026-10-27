# 0001: Base SDF as the semantic foundation

Esdieff's core target is the abstract interaction and data model of base SDF in [RFC 9880](https://www.rfc-editor.org/rfc/rfc9880.html), including its extension mechanisms. Separately specified extensions, protocol bindings, and ecosystem conversions are additional scope rather than prerequisites for the core toolkit. This gives model authors a common semantic foundation without tying their models to one ecosystem's wire representation.

## Considered Options

Starting with one ecosystem's integration could provide a concrete deployment path sooner, but would give that ecosystem's requirements priority over the common SDF model. The core instead prioritizes model checking, composition, and inspection across base SDF.
