//! End-to-end tests of the compiled binary: stdin, files, subcommands, the
//! clispec error envelope, and the process exit-code contract.

use std::io::Write;
use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_dotpick");

struct Output {
    code: i32,
    stdout: String,
    stderr: String,
}

/// Run the binary with `args`, feeding `stdin`, and capture the result.
fn run(args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(BIN)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn dotpick");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    Output {
        code: out.status.code().unwrap(),
        stdout: String::from_utf8(out.stdout).unwrap(),
        stderr: String::from_utf8(out.stderr).unwrap(),
    }
}

/// The `error` object from the last line of stderr (the clispec envelope).
fn error_envelope(stderr: &str) -> serde_json::Value {
    let last = stderr.lines().last().expect("stderr has an error line");
    let v: serde_json::Value = serde_json::from_str(last).expect("error envelope is JSON");
    v["error"].clone()
}

#[test]
fn projects_from_stdin_with_a_trailing_newline() {
    let out = run(&[".a.b"], r#"{"a":{"b":1,"c":2}}"#);
    assert_eq!(out.code, 0);
    assert_eq!(out.stdout, "{\"a\":{\"b\":1}}\n");
}

#[test]
fn output_flag_selects_format() {
    let out = run(
        &[".spec.replicas", "-o", "raw"],
        r#"{"spec":{"replicas":3}}"#,
    );
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    assert_eq!(out.stdout, "3\n");
}

#[test]
fn to_is_an_alias_for_output() {
    let out = run(
        &[".spec.replicas", "--to", "raw"],
        r#"{"spec":{"replicas":3}}"#,
    );
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    assert_eq!(out.stdout, "3\n");
}

#[test]
fn project_subcommand_matches_the_bare_default() {
    let doc = r#"{"a":{"b":1,"c":2}}"#;
    let bare = run(&[".a.b"], doc);
    let explicit = run(&["project", ".a.b"], doc);
    assert_eq!(explicit.code, 0, "stderr: {}", explicit.stderr);
    assert_eq!(explicit.stdout, bare.stdout);
}

#[test]
fn schema_subcommand_is_clispec_v0_2() {
    let out = run(&["schema"], "");
    assert_eq!(out.code, 0);
    let v: serde_json::Value = serde_json::from_str(&out.stdout).expect("valid JSON");
    assert_eq!(v["clispec"], "0.2");
    assert_eq!(v["name"], "dotpick");
}

#[test]
fn help_mentions_the_schema_command() {
    let out = run(&["--help"], "");
    assert_eq!(out.code, 0);
    assert!(out.stdout.contains("schema"), "help: {}", out.stdout);
}

#[test]
fn detects_format_from_file_extension() {
    let dir = std::env::temp_dir().join(format!("dotpick-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("conf.toml");
    std::fs::write(&path, "name = \"web\"\nreplicas = 3\n").unwrap();

    let out = run(&[".replicas", path.to_str().unwrap(), "-o", "raw"], "");
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    assert_eq!(out.stdout, "3\n");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn missing_path_emits_structured_error_and_exits_1() {
    let out = run(&[".nope"], r#"{"a":1}"#);
    assert_eq!(out.code, 1);
    assert!(
        out.stdout.is_empty(),
        "stdout should be clean: {}",
        out.stdout
    );
    let err = error_envelope(&out.stderr);
    assert_eq!(err["kind"], "path_not_found");
    assert_eq!(err["exit_code"], 1);
}

#[test]
fn bad_argument_emits_usage_error_envelope() {
    // clispec: even a bad invocation is machine-parseable on the last stderr line.
    let out = run(&[".a", "--no-such-flag"], r#"{"a":1}"#);
    assert_eq!(out.code, 3);
    let err = error_envelope(&out.stderr);
    assert_eq!(err["kind"], "usage");
}

#[test]
fn no_paths_exits_3() {
    let out = run(&[], "");
    assert_eq!(out.code, 3);
    assert_eq!(error_envelope(&out.stderr)["kind"], "usage");
}
