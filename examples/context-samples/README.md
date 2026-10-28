# Check Context values

These values use the [Context reuse model](../context-reuse/README.md). The asset tag is a simple string constrained to `"A-17"`. The location is a map with a required nonempty room and an integer floor of at least 2 after refinement.

From the repository root, check both values with the saved plan:

```sh
esdieff check --plan examples/context-samples/check-plan.json
```

Both cases pass. Follow [Check Context values](../../docs/context-contract.md#check-context-values) for the individual sample commands and their outcomes. Change the tag to `"A-18"`, omit the room or reduce the floor to 1 to see a rejection.

These are offline checks against final model descriptions. They do not retrieve metadata from a device or make Context available through a Property endpoint.
