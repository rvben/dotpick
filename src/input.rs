//! Input format detection and parsing into one or more JSON-shaped records.
//!
//! All formats are parsed into `serde_json::Value`, the universal in-memory
//! model. NDJSON yields one record per non-empty line; every other format
//! yields a single record.

use crate::InputFormat;
use crate::error::DotpickError;
use serde_json::Value;

/// Parse `input` into records, detecting the format when `forced` is `None`.
pub fn parse(input: &str, forced: Option<InputFormat>) -> Result<Vec<Value>, DotpickError> {
    let format = forced.unwrap_or_else(|| detect(input));
    match format {
        InputFormat::Json => Ok(vec![parse_json(input)?]),
        InputFormat::Yaml => Ok(vec![parse_yaml(input)?]),
        InputFormat::Toml => Ok(vec![parse_toml(input)?]),
        InputFormat::Ndjson => parse_ndjson(input),
    }
}

/// Best-effort format detection by trial parsing.
///
/// JSON is attempted first (it is a strict subset of YAML), then TOML (whose
/// `key = value` lines YAML would silently misread as scalars), then YAML as
/// the permissive fallback. Ambiguous inputs should be disambiguated with
/// `--from`; the CLI sets it from a file extension when reading a file.
fn detect(input: &str) -> InputFormat {
    if serde_json::from_str::<Value>(input).is_ok() {
        InputFormat::Json
    } else if toml::from_str::<Value>(input).is_ok() {
        InputFormat::Toml
    } else {
        InputFormat::Yaml
    }
}

fn parse_json(input: &str) -> Result<Value, DotpickError> {
    serde_json::from_str(input).map_err(|e| DotpickError::Parse {
        format: "json".into(),
        message: e.to_string(),
    })
}

fn parse_yaml(input: &str) -> Result<Value, DotpickError> {
    serde_norway::from_str(input).map_err(|e| DotpickError::Parse {
        format: "yaml".into(),
        message: e.to_string(),
    })
}

fn parse_toml(input: &str) -> Result<Value, DotpickError> {
    let toml_value: toml::Value = toml::from_str(input).map_err(|e| DotpickError::Parse {
        format: "toml".into(),
        message: e.to_string(),
    })?;
    Ok(toml_to_json(toml_value))
}

/// Convert a `toml::Value` into the universal JSON model.
///
/// TOML datetimes have no JSON counterpart, so they become RFC 3339 strings
/// rather than leaking the `toml` crate's internal representation.
fn toml_to_json(value: toml::Value) -> Value {
    use serde_json::{Map, Number};
    match value {
        toml::Value::String(s) => Value::String(s),
        toml::Value::Integer(i) => Value::Number(i.into()),
        toml::Value::Float(f) => Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        toml::Value::Boolean(b) => Value::Bool(b),
        toml::Value::Datetime(dt) => Value::String(dt.to_string()),
        toml::Value::Array(a) => Value::Array(a.into_iter().map(toml_to_json).collect()),
        toml::Value::Table(t) => Value::Object(
            t.into_iter()
                .map(|(k, v)| (k, toml_to_json(v)))
                .collect::<Map<_, _>>(),
        ),
    }
}

fn parse_ndjson(input: &str) -> Result<Vec<Value>, DotpickError> {
    let mut records = Vec::new();
    for (lineno, line) in input.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let value = serde_json::from_str(line).map_err(|e| DotpickError::Parse {
            format: "ndjson".into(),
            message: format!("line {}: {e}", lineno + 1),
        })?;
        records.push(value);
    }
    Ok(records)
}
