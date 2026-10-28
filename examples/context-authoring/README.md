# Author and edit Context metadata

This example assembles a sensor installation while keeping its calibration and
site name as non-interactive Context metadata. Run the commands from the
repository root with `esdieff` installed and a new output directory:

```sh
mkdir context-output
esdieff author --source base examples/context-authoring/base.sdf.json --plan examples/context-authoring/recipe.json --document-output context-output/installation.sdf.json --check full-baseline
esdieff inspect --view effective --document context-output/installation.sdf.json --pointer /sdfThing/Installation/sdfObject/sensor --format json examples/context-authoring/base.sdf.json context-output/installation.sdf.json
```

The generated Thing references the reusable Sensor Object. Its recipe narrows
the calibration maximum from 10 to `2.50`, removes its description and deletes
the retired Context entry. The site name and temperature Property remain
unchanged. Inspection shows the final constraints and the source of each change.
The Context definitions acquire no readable, writable or observable Property
characteristics.

Edit the authored site-name definition in the reusable document:

```sh
esdieff edit --source base examples/context-authoring/base.sdf.json --plan examples/context-authoring/edits.json --document-output context-output/revised-base.sdf.json --check full-baseline
```

This changes its description and minimum length, removes its label and constant
value, and preserves all other content. The original document remains unchanged.
You can use the revised source with the same recipe:

```sh
esdieff author --source base context-output/revised-base.sdf.json --plan examples/context-authoring/recipe.json --document-output context-output/revised-installation.sdf.json --check full-baseline
```

Editing a Context inside the generated Object's `sdfRef` boundary is refused:

```sh
esdieff edit --source installation context-output/installation.sdf.json --plan examples/context-authoring/outside-boundary.json --document-output context-output/refused.sdf.json
```

This command exits with code 2 and explains the composition boundary. No refused
document is written. Change the reusable authored definition or use the recipe's
refinement instead.

The library uses the same plans with `AuthorRequest::from_plan` and
`EditRequest::from_plan`. Their results expose candidate bytes, source locations
and request origins independently of generation, fidelity and requested
checking outcomes. See the [authoring contract](../../docs/authoring-contract.md),
[editing contract](../../docs/editing-contract.md) and
[Context draft policy](../../docs/context-contract.md) for the supported
qualities and remaining draft boundaries.
