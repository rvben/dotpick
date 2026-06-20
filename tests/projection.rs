//! Behavior tests for dotpick, exercised through the public `run` API only.

use dotpick::{DotpickError, InputFormat, OutputFormat, Request, run};

/// A structured-JSON projection request with default options.
fn req(input: &str, paths: &str) -> Request {
    Request {
        input: input.to_string(),
        from: None,
        paths: paths.to_string(),
        flat: false,
        to: None,
        allow_missing: false,
        pretty: false,
    }
}

// ---- structure-preserving projection ------------------------------------

#[test]
fn projects_a_single_nested_key() {
    let out = run(&req(r#"{"a":{"b":1,"c":2},"d":3}"#, ".a.b")).unwrap();
    assert_eq!(out, r#"{"a":{"b":1}}"#);
}

#[test]
fn merges_multiple_paths() {
    let out = run(&req(r#"{"a":{"b":1,"c":2},"d":3}"#, ".a.b,.d")).unwrap();
    assert_eq!(out, r#"{"a":{"b":1},"d":3}"#);
}

#[test]
fn indexes_into_arrays() {
    let out = run(&req(r#"{"items":[{"n":1},{"n":2}]}"#, ".items[0].n")).unwrap();
    assert_eq!(out, r#"{"items":[{"n":1}]}"#);
}

#[test]
fn iterates_arrays_preserving_structure() {
    let out = run(&req(r#"{"items":[{"n":1},{"n":2}]}"#, ".items[].n")).unwrap();
    assert_eq!(out, r#"{"items":[{"n":1},{"n":2}]}"#);
}

#[test]
fn merges_two_iter_fields_element_wise() {
    let out = run(&req(
        r#"{"items":[{"n":1,"a":9},{"n":2,"a":8}]}"#,
        ".items[].n,.items[].a",
    ))
    .unwrap();
    assert_eq!(out, r#"{"items":[{"a":9,"n":1},{"a":8,"n":2}]}"#);
}

#[test]
fn root_path_is_identity() {
    let out = run(&req(r#"{"b":2,"a":1}"#, ".")).unwrap();
    assert_eq!(out, r#"{"a":1,"b":2}"#);
}

// ---- flat projection ----------------------------------------------------

#[test]
fn flat_keys_each_leaf_by_name() {
    let r = Request {
        flat: true,
        ..req(
            r#"{"metadata":{"name":"x"},"spec":{"replicas":3}}"#,
            ".metadata.name,.spec.replicas",
        )
    };
    assert_eq!(run(&r).unwrap(), r#"{"name":"x","replicas":3}"#);
}

#[test]
fn flat_collects_iter_into_array() {
    let r = Request {
        flat: true,
        ..req(r#"{"items":[{"name":"a"},{"name":"b"}]}"#, ".items[].name")
    };
    assert_eq!(run(&r).unwrap(), r#"{"name":["a","b"]}"#);
}

#[test]
fn flat_name_collision_is_an_error() {
    let r = Request {
        flat: true,
        ..req(r#"{"a":{"name":1},"b":{"name":2}}"#, ".a.name,.b.name")
    };
    let err = run(&r).unwrap_err();
    assert!(matches!(err, DotpickError::NameCollision { .. }));
    assert_eq!(err.exit_code(), 3);
}

// ---- raw streaming ------------------------------------------------------

#[test]
fn raw_emits_bare_scalar() {
    let r = Request {
        to: Some(OutputFormat::Raw),
        ..req(r#"{"spec":{"replicas":3}}"#, ".spec.replicas")
    };
    assert_eq!(run(&r).unwrap(), "3");
}

#[test]
fn raw_strings_are_unquoted() {
    let r = Request {
        to: Some(OutputFormat::Raw),
        ..req(r#"{"msg":"hello world"}"#, ".msg")
    };
    assert_eq!(run(&r).unwrap(), "hello world");
}

#[test]
fn raw_streams_one_value_per_line() {
    let r = Request {
        to: Some(OutputFormat::Raw),
        ..req(r#"{"items":[{"name":"a"},{"name":"b"}]}"#, ".items[].name")
    };
    assert_eq!(run(&r).unwrap(), "a\nb");
}

#[test]
fn raw_on_non_scalar_is_an_error() {
    let r = Request {
        to: Some(OutputFormat::Raw),
        ..req(r#"{"a":{"b":1}}"#, ".a")
    };
    let err = run(&r).unwrap_err();
    assert!(matches!(err, DotpickError::RawNonScalar { .. }));
    assert_eq!(err.exit_code(), 3);
}

// ---- ndjson streaming ---------------------------------------------------

#[test]
fn ndjson_streams_each_element() {
    let r = Request {
        to: Some(OutputFormat::Ndjson),
        ..req(r#"{"items":[{"name":"a"},{"name":"b"}]}"#, ".items[]")
    };
    assert_eq!(run(&r).unwrap(), "{\"name\":\"a\"}\n{\"name\":\"b\"}");
}

#[test]
fn ndjson_input_is_projected_per_record() {
    let r = Request {
        from: Some(InputFormat::Ndjson),
        ..req("{\"a\":1,\"x\":9}\n{\"a\":2,\"x\":8}", ".a")
    };
    // default output for ndjson input is ndjson
    assert_eq!(run(&r).unwrap(), "1\n2");
}

#[test]
fn root_array_streams_to_ndjson() {
    let r = Request {
        to: Some(OutputFormat::Ndjson),
        ..req(r#"[{"a":1},{"a":2}]"#, ".[]")
    };
    assert_eq!(run(&r).unwrap(), "{\"a\":1}\n{\"a\":2}");
}

// ---- format detection and conversion ------------------------------------

#[test]
fn detects_and_reads_yaml() {
    let out = run(&req("a:\n  b: 1\n", ".a.b")).unwrap();
    assert_eq!(out, r#"{"a":{"b":1}}"#);
}

#[test]
fn detects_and_reads_toml() {
    let out = run(&req("a = 1\nb = 2\n", ".a")).unwrap();
    assert_eq!(out, r#"{"a":1}"#);
}

#[test]
fn toml_datetime_becomes_a_plain_string() {
    let r = Request {
        from: Some(InputFormat::Toml),
        ..req("when = 2026-06-20T10:00:00Z\n", ".when")
    };
    assert_eq!(run(&r).unwrap(), r#"{"when":"2026-06-20T10:00:00Z"}"#);
}

#[test]
fn converts_toml_to_yaml_via_root() {
    let r = Request {
        from: Some(InputFormat::Toml),
        to: Some(OutputFormat::Yaml),
        ..req("x = 1\n", ".")
    };
    assert_eq!(run(&r).unwrap(), "x: 1");
}

#[test]
fn renders_toml_output() {
    let r = Request {
        to: Some(OutputFormat::Toml),
        ..req(r#"{"a":1,"b":2}"#, ".a")
    };
    assert_eq!(run(&r).unwrap(), "a = 1");
}

#[test]
fn toml_non_table_root_is_an_error() {
    let r = Request {
        to: Some(OutputFormat::Toml),
        ..req(r#"[1,2]"#, ".[]")
    };
    let err = run(&r).unwrap_err();
    assert!(matches!(err, DotpickError::Serialize { .. }));
    assert_eq!(err.exit_code(), 2);
}

#[test]
fn pretty_prints_json() {
    let r = Request {
        pretty: true,
        ..req(r#"{"a":{"b":1}}"#, ".a.b")
    };
    assert_eq!(run(&r).unwrap(), "{\n  \"a\": {\n    \"b\": 1\n  }\n}");
}

// ---- error contract -----------------------------------------------------

#[test]
fn missing_path_errors_with_nearest_hint() {
    let err = run(&req(r#"{"spec":{"replic":3}}"#, ".spec.replicas")).unwrap_err();
    assert!(matches!(err, DotpickError::PathNotFound { .. }));
    assert_eq!(err.exit_code(), 1);
    assert!(err.to_string().contains("nearest existing: replic"));
}

#[test]
fn type_mismatch_errors() {
    let err = run(&req(r#"{"a":1}"#, ".a.b")).unwrap_err();
    assert!(matches!(err, DotpickError::TypeMismatch { .. }));
    assert_eq!(err.exit_code(), 1);
}

#[test]
fn allow_missing_skips_absent_paths() {
    let r = Request {
        allow_missing: true,
        ..req(r#"{"a":1}"#, ".a,.b")
    };
    assert_eq!(run(&r).unwrap(), r#"{"a":1}"#);
}

#[test]
fn invalid_dotpath_is_a_usage_error() {
    let err = run(&req(r#"{"a":1}"#, ".a.")).unwrap_err();
    assert!(matches!(err, DotpickError::PathSyntax { .. }));
    assert_eq!(err.exit_code(), 3);
}

#[test]
fn malformed_input_is_a_parse_error() {
    let r = Request {
        from: Some(InputFormat::Json),
        ..req("{not json", ".a")
    };
    let err = run(&r).unwrap_err();
    assert!(matches!(err, DotpickError::Parse { .. }));
    assert_eq!(err.exit_code(), 2);
}
