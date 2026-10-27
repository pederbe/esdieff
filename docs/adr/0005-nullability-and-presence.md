# 0005: Nullability and presence

Esdieff interprets `nullable` as admitting a null value, with the default `true` specified by [RFC 9880, Section 4.7, Table 4](https://www.rfc-editor.org/rfc/rfc9880.html#section-4.7). This is separate from mandatory presence: `required` constrains entries in map data, and `sdfRequired` constrains declared components. The registry description in Section 7.5.1 instead refers to omission, which conflicts with the data-quality definition.

The [original specification source](https://github.com/ietf-wg-asdf/SDF/blob/b02944c1b2b4d8b92d4d1a10e01b191a90f5b579/sdf.md#L344) already describes null availability with default `true`. The [later registry addition](https://github.com/ietf-wg-asdf/SDF/commit/2fd3763f393b9284f53923e82883e3ad6081113c) introduces the conflicting description without changing that meaning. This history supports the data-quality definition as the intended semantics.

Null admission extends the effective type; other applicable constraints still apply, including `const`. Thus a plain string description admits null by default, while `const: "on"` excludes it. For `sdfChoice`, Esdieff evaluates each effective alternative after inherited qualities and explicit overrides; a value is admitted when an effective alternative admits it, and an alternative may explicitly admit null. `enum` follows its RFC expansion into `const` alternatives.

The inspectable alternative and characteristic states are specified in [ADR 0024: Effective alternatives and nullable characteristics](0024-effective-alternatives-and-nullable-characteristics.md). Inherited authored values retain their donor's established meaning and uncertainty. A Full alternative defaults nullable to true only after its final inherited/overridden quality set establishes absence. These views expose the interpretation without performing sample admission or general consistency proofs.

[Appendix C](https://www.rfc-editor.org/rfc/rfc9880.html#appendix-C) restricts `const` to one value, and [Section 4.7.2](https://www.rfc-editor.org/rfc/rfc9880.html#section-4.7.2) defines choice inheritance and enum expansion. These support this constraint interpretation without explicitly settling null precedence. Dependent conclusions remain incomplete when an effective alternative cannot be established; stronger adopted standards evidence can change the interpretation.

## Considered Options

Treating nullability as optional presence would conflate distinct constraints and leave null values without the meaning explicitly given in Table 4. Esdieff records the interpretation separately from agreed deviations.

Allowing null to bypass every constraint would contradict the single-value meaning of `const` and the explicit enum expansion. Leaving every interaction unresolved would postpone useful definite checks despite the available supporting evidence.
