//! `dotpick schema` must validate against the published clispec v0.2 JSON
//! Schema (vendored at schemas/clispec-v0.2.json).

#[test]
fn schema_conforms_to_clispec_v0_2() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/clispec-v0.2.json"))
            .expect("vendored clispec schema is valid JSON");

    let instance = dotpick::schema::contract();
    let validator = jsonschema::validator_for(&schema).expect("compile clispec schema");

    if !validator.is_valid(&instance) {
        let errors: Vec<String> = validator
            .iter_errors(&instance)
            .map(|e| format!("{} at {}", e, e.instance_path()))
            .collect();
        panic!(
            "dotpick schema does not conform to clispec v0.2:\n{}",
            errors.join("\n")
        );
    }
}

#[test]
fn schema_declares_required_clispec_fields() {
    let v = dotpick::schema::contract();
    assert_eq!(v["clispec"], "0.2");
    assert_eq!(v["name"], "dotpick");
    assert!(v["commands"].as_array().is_some_and(|c| !c.is_empty()));
    assert!(v["global_args"].as_array().is_some_and(|g| !g.is_empty()));
    assert!(v["errors"].as_array().is_some_and(|e| !e.is_empty()));

    // The default command is declared read-only (a trust contract).
    let project = v["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "project")
        .expect("project command present");
    assert_eq!(project["mutating"], false);
}
