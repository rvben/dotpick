//! End-to-end tests of the compiled binary: stdin, files, subcommands and
//! the process exit-code contract.

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

#[test]
fn projects_from_stdin_with_a_trailing_newline() {
    let out = run(&[".a.b"], r#"{"a":{"b":1,"c":2}}"#);
    assert_eq!(out.code, 0);
    assert_eq!(out.stdout, "{\"a\":{\"b\":1}}\n");
}

#[test]
fn schema_subcommand_prints_valid_json() {
    let out = run(&["schema"], "");
    assert_eq!(out.code, 0);
    let v: serde_json::Value = serde_json::from_str(&out.stdout).expect("valid JSON");
    assert_eq!(v["name"], "dotpick");
}

#[test]
fn detects_format_from_file_extension() {
    let dir = std::env::temp_dir().join(format!("dotpick-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("conf.toml");
    std::fs::write(&path, "name = \"web\"\nreplicas = 3\n").unwrap();

    let out = run(&[".replicas", path.to_str().unwrap(), "--to", "raw"], "");
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    assert_eq!(out.stdout, "3\n");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn missing_path_exits_1() {
    let out = run(&[".nope"], r#"{"a":1}"#);
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("not found"));
}

#[test]
fn no_paths_exits_3() {
    let out = run(&[], "");
    assert_eq!(out.code, 3);
}
