//! The agent contract must be valid, stable JSON.

#[test]
fn schema_is_valid_json_with_the_expected_shape() {
    let text = dotpick::schema::contract_json();
    let v: serde_json::Value = serde_json::from_str(&text).expect("schema is valid JSON");

    assert_eq!(v["schema_version"], "1");
    assert_eq!(v["name"], "dotpick");
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(v["mutating"], false);

    assert!(v["commands"].as_array().is_some_and(|a| !a.is_empty()));
    assert!(v["options"].as_array().is_some_and(|a| !a.is_empty()));
    assert!(v["dotpath_grammar"].is_object());

    // Every documented error kind carries an exit code.
    for kind in v["error_kinds"].as_array().unwrap() {
        assert!(kind["kind"].is_string());
        assert!(kind["exit_code"].is_number());
    }

    // The four documented process exit codes are present.
    for code in ["0", "1", "2", "3"] {
        assert!(
            v["exit_codes"][code].is_string(),
            "missing exit code {code}"
        );
    }
}
