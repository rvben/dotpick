//! Serialization of projected documents to the chosen output format.
//!
//! Output never has a trailing newline; the CLI adds one. Object keys are
//! emitted in sorted order for stable, diff-friendly output.

use crate::OutputFormat;
use crate::error::DotpickError;
use crate::path::Path;
use crate::project::{self, is_scalar, type_name};
use serde_json::Value;

/// Render projected documents (one per input record) in `format`.
///
/// Only the document-shaped formats (`json`, `yaml`, `toml`) go through here;
/// the streaming formats (`ndjson`, `raw`) are rendered from the selected
/// values via [`render_ndjson`] and [`render_raw`].
pub fn render(docs: &[Value], format: OutputFormat, pretty: bool) -> Result<String, DotpickError> {
    match format {
        OutputFormat::Json => render_json(docs, pretty),
        OutputFormat::Yaml => render_yaml(docs),
        OutputFormat::Toml => render_toml(docs),
        OutputFormat::Ndjson | OutputFormat::Raw => {
            unreachable!("streaming formats are rendered from selected values")
        }
    }
}

/// A single value: the lone document, or the documents wrapped in an array.
fn collapse(docs: &[Value]) -> Value {
    match docs {
        [one] => one.clone(),
        many => Value::Array(many.to_vec()),
    }
}

fn render_json(docs: &[Value], pretty: bool) -> Result<String, DotpickError> {
    let value = collapse(docs);
    let result = if pretty {
        serde_json::to_string_pretty(&value)
    } else {
        serde_json::to_string(&value)
    };
    result.map_err(|e| DotpickError::Serialize {
        format: "json".into(),
        message: e.to_string(),
    })
}

fn render_yaml(docs: &[Value]) -> Result<String, DotpickError> {
    let value = collapse(docs);
    serde_norway::to_string(&value)
        .map(|s| s.trim_end().to_string())
        .map_err(|e| DotpickError::Serialize {
            format: "yaml".into(),
            message: e.to_string(),
        })
}

fn render_toml(docs: &[Value]) -> Result<String, DotpickError> {
    let value = collapse(docs);
    if !value.is_object() {
        return Err(DotpickError::Serialize {
            format: "toml".into(),
            message: format!(
                "TOML root must be a table, but the result is {}",
                type_name(&value)
            ),
        });
    }
    toml::to_string(&value)
        .map(|s| s.trim_end().to_string())
        .map_err(|e| DotpickError::Serialize {
            format: "toml".into(),
            message: e.to_string(),
        })
}

/// Render `--to ndjson`: one compact JSON value per selected match, one per
/// line. `[]` in a path controls granularity (`.items[]` streams elements,
/// `.items[].name` streams names).
pub fn render_ndjson(
    records: &[Value],
    paths: &[Path],
    allow_missing: bool,
) -> Result<String, DotpickError> {
    let mut lines = Vec::new();
    for record in records {
        for (_path, value) in project::select_all(record, paths, allow_missing)? {
            lines.push(compact(&value)?);
        }
    }
    Ok(lines.join("\n"))
}

/// Render `--to raw`: bare scalar values, one per line.
pub fn render_raw(
    records: &[Value],
    paths: &[Path],
    allow_missing: bool,
) -> Result<String, DotpickError> {
    let mut lines = Vec::new();
    for record in records {
        for (path, value) in project::select_all(record, paths, allow_missing)? {
            if !is_scalar(&value) {
                return Err(DotpickError::RawNonScalar {
                    path,
                    found: type_name(&value).to_string(),
                });
            }
            lines.push(scalar_to_raw(&value));
        }
    }
    Ok(lines.join("\n"))
}

fn scalar_to_raw(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "null".to_string(),
        other => other.to_string(),
    }
}

fn compact(v: &Value) -> Result<String, DotpickError> {
    serde_json::to_string(v).map_err(|e| DotpickError::Serialize {
        format: "ndjson".into(),
        message: e.to_string(),
    })
}
