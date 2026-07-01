//! The clispec v0.2 contract emitted by `dotpick schema`.
//!
//! Conforms to <https://clispec.dev/schema/v0.2.json> (validated by a test
//! against the vendored copy in `schemas/clispec-v0.2.json`).

use serde_json::{Value, json};

/// The version of The CLI Spec this document conforms to.
pub const CLISPEC_VERSION: &str = "0.2";

/// Build the clispec contract as a JSON value.
pub fn contract() -> Value {
    json!({
        "clispec": CLISPEC_VERSION,
        "name": "dotpick",
        "version": env!("CARGO_PKG_VERSION"),
        "description": env!("CARGO_PKG_DESCRIPTION"),
        "global_args": [
            {
                "name": "--output",
                "type": "string",
                "enum": ["auto", "json", "yaml", "toml", "ndjson", "raw"],
                "default": "auto",
                "description": "Output format. auto = JSON, or NDJSON when the input is NDJSON. Alias: --to / -o."
            },
            {
                "name": "--from",
                "type": "string",
                "enum": ["json", "yaml", "toml", "ndjson"],
                "description": "Force the input format (default: auto-detect, or from the file extension)."
            },
            {
                "name": "--flat",
                "type": "boolean",
                "default": false,
                "description": "Key each selected leaf by its final name instead of preserving structure."
            },
            {
                "name": "--allow-missing",
                "type": "boolean",
                "default": false,
                "description": "Skip absent paths instead of erroring."
            },
            {
                "name": "--pretty",
                "type": "boolean",
                "default": false,
                "description": "Pretty-print JSON output."
            }
        ],
        "commands": [
            {
                "name": "schema",
                "description": "Print this clispec contract as JSON.",
                "mutating": false,
                "stability": "stable",
                "output_fields": [
                    {"name": "clispec", "type": "string", "description": "The CLI Spec version this document conforms to."},
                    {"name": "name", "type": "string", "description": "The tool's invocation name."},
                    {"name": "version", "type": "string", "description": "The tool's version."},
                    {"name": "global_args", "type": "array", "description": "Flags accepted by the projection command."},
                    {"name": "commands", "type": "array", "description": "The commands the tool exposes."},
                    {"name": "errors", "type": "array", "description": "The finite set of error kinds with exit codes."}
                ]
            },
            {
                "name": "project",
                "description": "Project the comma-separated dotpaths from a file or stdin. Also the default command, invoked as `dotpick <paths> [file]`. Output mirrors the selected paths, so it has no fixed shape; object keys are emitted in sorted order.",
                "mutating": false,
                "stability": "stable",
                "args": [
                    {
                        "name": "paths",
                        "type": "string",
                        "required": true,
                        "description": "Comma-separated dotpaths, e.g. '.metadata.name,.spec.replicas'. Grammar: .key, [\"quoted.key\"], [0], [], . (root)."
                    },
                    {
                        "name": "file",
                        "type": "path",
                        "required": false,
                        "description": "Input file; omit to read stdin."
                    }
                ],
                "example": {"args": ["."], "stdin": "{\"x\":1}"}
            },
            {
                "name": "completions",
                "description": "Generate a shell completion script.",
                "mutating": false,
                "stability": "stable",
                "args": [
                    {
                        "name": "shell",
                        "type": "string",
                        "required": true,
                        "enum": ["bash", "zsh", "fish", "powershell", "elvish"],
                        "description": "Target shell."
                    }
                ]
            }
        ],
        "errors": [
            {"kind": "usage", "exit_code": 3, "retryable": false, "description": "Invalid command-line arguments."},
            {"kind": "path_syntax", "exit_code": 3, "retryable": false, "description": "A dotpath is syntactically invalid."},
            {"kind": "name_collision", "exit_code": 3, "retryable": false, "description": "Two leaves collapse to the same name under --flat."},
            {"kind": "raw_non_scalar", "exit_code": 3, "retryable": false, "description": "--output raw was used but a selected value is not a scalar."},
            {"kind": "path_not_found", "exit_code": 1, "retryable": false, "description": "A selected path is absent. The message includes a nearest-key hint."},
            {"kind": "type_mismatch", "exit_code": 1, "retryable": false, "description": "A path segment was applied to the wrong kind of value."},
            {"kind": "parse", "exit_code": 2, "retryable": false, "description": "Input could not be parsed in the detected or forced format."},
            {"kind": "serialize", "exit_code": 2, "retryable": false, "description": "The result cannot be represented in the requested output format (e.g. a non-table TOML root)."},
            {"kind": "io", "exit_code": 2, "retryable": false, "description": "The input file could not be read."}
        ]
    })
}

/// The contract as a pretty-printed JSON string.
pub fn contract_json() -> String {
    serde_json::to_string_pretty(&contract()).expect("contract serializes")
}
