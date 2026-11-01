use serde_json::Value;
const SENSOR: &str = "/sdfThing/Installation/sdfObject/sensor";
fn cli(args: &[&str], status: i32) -> Value {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_esdieff"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(args)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(status),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn cli_examples_check_and_inspect_final_context_values_and_origins() {
    let base = "examples/context-reuse/base.sdf.json";
    let device = "examples/context-reuse/device.sdf.json";
    let wire = cli(
        &[
            "check",
            "--root",
            device,
            "--assessment",
            "full-baseline",
            "--format",
            "json",
            base,
            device,
        ],
        0,
    );
    assert_eq!(wire["outcome"]["complete"], true);
    let wire = cli(
        &[
            "inspect",
            "--view",
            "effective",
            "--document",
            device,
            "--pointer",
            SENSOR,
            "--format",
            "json",
            base,
            device,
        ],
        0,
    );
    assert_eq!(
        wire["result"]["json"]["sdfContext"]["calibration"]["maximum"],
        5
    );
    assert_eq!(
        wire["result"]["json"]["sdfContext"]["assetTag"]["minLength"],
        1
    );
    assert!(
        wire["result"]["json"]["sdfContext"]
            .get("retired")
            .is_none()
    );
    for (tail, kind, source_pointer, consumer) in [
        (
            "location/properties/room/type",
            "inherited",
            "/sdfData/Location/properties/room/type",
            "/sdfObject/Sensor/sdfContext/location/sdfRef",
        ),
        (
            "calibration/maximum",
            "override",
            "/sdfThing/Installation/sdfObject/sensor/sdfData/Offset/maximum",
            "/sdfObject/Sensor/sdfContext/calibration/sdfRef",
        ),
        (
            "calibration/description",
            "deleted",
            "/sdfThing/Installation/sdfObject/sensor/sdfContext/calibration/description",
            "/sdfObject/Sensor/sdfContext/calibration/sdfRef",
        ),
    ] {
        let explanation = wire["result"]["explanations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["effective"]["pointer"] == format!("{SENSOR}/sdfContext/{tail}"))
            .unwrap();
        assert_eq!(explanation["kind"], kind);
        assert_eq!(explanation["sources"][0]["pointer"], source_pointer);
        assert_eq!(explanation["consumer"]["pointer"], consumer);
    }
    let unavailable = "examples/context-reuse/unavailable.sdf.json";
    let check = cli(&["check", "--format", "json", unavailable], 3);
    assert_eq!(check["outcome"]["complete"], false);
    assert_eq!(check["sources"].as_array().unwrap().len(), 1);
    assert!(
        check["diagnostics"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["code"] == "reference.unavailable")
    );
    let wire = cli(
        &[
            "inspect",
            "--view",
            "effective",
            "--document",
            unavailable,
            "--pointer",
            "/sdfObject/Device/sdfContext/assetTag",
            "--format",
            "json",
            unavailable,
        ],
        0,
    );
    assert_eq!(wire["result"]["json"]["const"], "A-17");
}
