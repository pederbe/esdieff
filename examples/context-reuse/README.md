# Reuse Context metadata

The [base document](base.sdf.json) defines reusable Location and AssetTag data descriptions, a Sensor sdfObject and a Site sdfThing. The [device document](device.sdf.json) reuses both groupings, refines the installation floor and calibration limit, adds an asset tag and deletes a description and retired Context entry.

From the repository root, check the device with its supplied dependency and inspect the final Context values and explanations:

```sh
esdieff check --root examples/context-reuse/device.sdf.json --assessment full-baseline examples/context-reuse/base.sdf.json examples/context-reuse/device.sdf.json
esdieff inspect --view effective --document examples/context-reuse/device.sdf.json --pointer /sdfThing/Installation/sdfObject/sensor --format json examples/context-reuse/base.sdf.json examples/context-reuse/device.sdf.json
cargo run --example context_reuse
```

The resulting calibration maximum is `5`. Its explanation points to the device's Offset refinement and the Sensor's importing calibration reference. The deleted description and retired entry keep deletion explanations. The original documents retain their authored content.

The selected check completes because the device deletes the retired entry before its missing reference needs resolution. Checking the entire Catalogue also checks the base document's unused retired entry and reports the unavailable target.

The [unavailable example](unavailable.sdf.json) keeps a missing Location reference beside an independently inspectable asset tag:

```sh
esdieff check examples/context-reuse/unavailable.sdf.json
esdieff inspect --view effective --document examples/context-reuse/unavailable.sdf.json --pointer /sdfObject/Device/sdfContext/assetTag examples/context-reuse/unavailable.sdf.json
```

The check exits with status `3` for incomplete work. Inspection of the independent asset tag succeeds. The namespace URL does not trigger retrieval. See the [Context contract](../../docs/context-contract.md) for draft defaults and requiredness boundaries.
