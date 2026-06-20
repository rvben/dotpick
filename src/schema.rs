//! Machine-readable contract for agents, emitted by `dotpick schema`.

use serde_json::{Value, json};

/// The schema version of this contract (bumped on breaking contract changes).
pub const SCHEMA_VERSION: &str = "1";

/// Build the full agent contract as a JSON value.
pub fn contract() -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "name": "dotpick",
        "version": env!("CARGO_PKG_VERSION"),
        "description": env!("CARGO_PKG_DESCRIPTION"),
        "summary": "Select fields from JSON/YAML/TOML/NDJSON by dotpath and emit the smallest valid slice. Pure projection and format conversion: no computation, no mutation.",
        "mutating": false,
        "reads_stdin": true,
        "commands": [
            {
                "name": "<paths> [file]",
                "summary": "Project the given comma-separated dotpaths from a file or stdin.",
                "mutating": false,
                "args": [
                    {"name": "paths", "required": true, "description": "Comma-separated dotpaths, e.g. '.metadata.name,.spec.replicas'."},
                    {"name": "file", "required": false, "description": "Input file; omit to read stdin."}
                ]
            },
            {
                "name": "schema",
                "summary": "Print this contract as JSON.",
                "mutating": false,
                "args": []
            }
        ],
        "options": [
            {"flag": "--from", "values": ["json", "yaml", "toml", "ndjson"], "description": "Force the input format (default: auto-detect, or from the file extension)."},
            {"flag": "--to", "values": ["json", "yaml", "toml", "ndjson", "raw"], "description": "Force the output format (default: json, or ndjson when input is ndjson)."},
            {"flag": "--flat", "description": "Key each selected leaf by its final name instead of preserving structure."},
            {"flag": "--allow-missing", "description": "Skip absent paths instead of erroring."},
            {"flag": "--pretty", "description": "Pretty-print JSON output."}
        ],
        "dotpath_grammar": {
            "key": ".name (bareword [A-Za-z0-9_-]+)",
            "quoted_key": "[\"any.key with spaces\"] for keys with dots/spaces/brackets",
            "index": "[0] (non-negative array index)",
            "iterate": "[] (every element of an array)",
            "root": ". (the whole document; useful for format conversion)",
            "chaining": ".a.b[0].c[].d",
            "multiple": "comma-separated: .a.b,.c[].d"
        },
        "output_shapes": {
            "structured": "default: smallest sub-document preserving nesting",
            "flat": "--flat: object keyed by each path's final name",
            "raw": "--to raw: bare scalar values, one per line"
        },
        "output_fields": {
            "note": "Output mirrors the selected paths; there is no fixed envelope. Object keys are emitted in sorted order for stable output."
        },
        "error_kinds": [
            {"kind": "parse", "exit_code": 2, "description": "Input could not be parsed in the detected/forced format."},
            {"kind": "serialize", "exit_code": 2, "description": "Result cannot be represented in the requested output format (e.g. non-table TOML root)."},
            {"kind": "path_not_found", "exit_code": 1, "description": "A selected path is absent (includes a nearest-key hint)."},
            {"kind": "type_mismatch", "exit_code": 1, "description": "A path segment was applied to the wrong kind of value."},
            {"kind": "path_syntax", "exit_code": 3, "description": "A dotpath is syntactically invalid."},
            {"kind": "name_collision", "exit_code": 3, "description": "Two leaves collapse to the same name under --flat."},
            {"kind": "raw_non_scalar", "exit_code": 3, "description": "--to raw was used but a selected value is not a scalar."}
        ],
        "exit_codes": {
            "0": "success",
            "1": "no match (selected path absent or empty result)",
            "2": "parse or serialize failure",
            "3": "usage error (bad dotpath, name collision, raw on non-scalar)"
        },
        "examples": [
            {"cmd": "dotpick '.metadata.name,.spec.replicas' deploy.yaml", "note": "Pruned sub-document of two fields."},
            {"cmd": "dotpick .spec.replicas deploy.yaml --to raw", "note": "Just the scalar value, unquoted."},
            {"cmd": "cat pods.json | dotpick '.items[]' --to ndjson", "note": "Stream each array element as one JSON object per line."},
            {"cmd": "dotpick '.items[].metadata.name' pods.json --to raw", "note": "Stream one bare name per line."},
            {"cmd": "dotpick . config.toml --to json", "note": "Convert formats with the root path."}
        ]
    })
}

/// The contract as a pretty-printed JSON string.
pub fn contract_json() -> String {
    serde_json::to_string_pretty(&contract()).expect("contract serializes")
}
