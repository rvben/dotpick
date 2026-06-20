//! Projection: turn a document plus a set of dotpaths into the smallest
//! value that contains exactly the selected leaves.
//!
//! Two shapes are produced:
//! - [`structured`] keeps the original nesting (a pruned sub-document).
//! - [`flat`] keys each selected leaf by its final name.
//!
//! [`select`] returns the bare leaf values and backs both `--flat` and
//! `--to raw`.

use crate::error::DotpickError;
use crate::path::{Path, Segment};
use serde_json::{Map, Value};

/// Human-readable type name for error messages.
pub fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// Whether a value is a scalar (valid for `--to raw`).
pub fn is_scalar(v: &Value) -> bool {
    !matches!(v, Value::Array(_) | Value::Object(_))
}

/// Structure-preserving projection across all paths, merged into one value.
pub fn structured(
    root: &Value,
    paths: &[Path],
    allow_missing: bool,
) -> Result<Value, DotpickError> {
    let mut acc: Option<Value> = None;
    for p in paths {
        if let Some(pruned) = prune(root, &p.segments, &p.display, allow_missing)? {
            acc = Some(match acc {
                Some(existing) => merge(existing, pruned),
                None => pruned,
            });
        }
    }
    Ok(acc.unwrap_or(Value::Null))
}

/// Flat projection: an object mapping each path's leaf name to its value.
pub fn flat(root: &Value, paths: &[Path], allow_missing: bool) -> Result<Value, DotpickError> {
    let mut obj = Map::new();
    for p in paths {
        let values = select(root, &p.segments, &p.display, allow_missing)?;
        let name = p.leaf_name().ok_or_else(|| DotpickError::PathSyntax {
            path: p.display.clone(),
            message: "--flat needs a named field, but this path has no key".into(),
        })?;
        let value = if p.has_iter() {
            Value::Array(values)
        } else {
            match values.len() {
                0 => continue, // absent under --allow-missing
                1 => values.into_iter().next().unwrap(),
                _ => Value::Array(values),
            }
        };
        if obj.contains_key(name) {
            return Err(DotpickError::NameCollision { name: name.into() });
        }
        obj.insert(name.to_string(), value);
    }
    Ok(Value::Object(obj))
}

/// Bare leaf values for every match of every path, in path then document order.
pub fn select_all(
    root: &Value,
    paths: &[Path],
    allow_missing: bool,
) -> Result<Vec<(String, Value)>, DotpickError> {
    let mut out = Vec::new();
    for p in paths {
        for v in select(root, &p.segments, &p.display, allow_missing)? {
            out.push((p.display.clone(), v));
        }
    }
    Ok(out)
}

/// Prune `root` to the single path, preserving structure.
/// Returns `None` when the path is absent and `allow_missing` is set.
fn prune(
    root: &Value,
    segs: &[Segment],
    display: &str,
    allow_missing: bool,
) -> Result<Option<Value>, DotpickError> {
    let Some((head, rest)) = segs.split_first() else {
        return Ok(Some(root.clone()));
    };
    match head {
        Segment::Key(k) => match root {
            Value::Object(m) => match m.get(k) {
                Some(child) => Ok(prune(child, rest, display, allow_missing)?.map(|p| {
                    let mut obj = Map::new();
                    obj.insert(k.clone(), p);
                    Value::Object(obj)
                })),
                None if allow_missing => Ok(None),
                None => Err(DotpickError::PathNotFound {
                    path: display.to_string(),
                    hint: nearest_key(m, k),
                }),
            },
            other => mismatch(allow_missing, display, "an object", other),
        },
        Segment::Index(i) => match root {
            Value::Array(a) => match a.get(*i) {
                Some(child) => {
                    Ok(prune(child, rest, display, allow_missing)?.map(|p| Value::Array(vec![p])))
                }
                None if allow_missing => Ok(None),
                None => Err(DotpickError::PathNotFound {
                    path: display.to_string(),
                    hint: Some(format!("array has {} element(s)", a.len())),
                }),
            },
            other => mismatch(allow_missing, display, "an array", other),
        },
        Segment::Iter => match root {
            Value::Array(a) => {
                let mut out = Vec::new();
                for el in a {
                    if let Some(p) = prune(el, rest, display, allow_missing)? {
                        out.push(p);
                    }
                }
                Ok(Some(Value::Array(out)))
            }
            other => mismatch(allow_missing, display, "an array", other),
        },
    }
}

/// Collect bare leaf values for one path (expanding every `[]`).
fn select(
    root: &Value,
    segs: &[Segment],
    display: &str,
    allow_missing: bool,
) -> Result<Vec<Value>, DotpickError> {
    let Some((head, rest)) = segs.split_first() else {
        return Ok(vec![root.clone()]);
    };
    match head {
        Segment::Key(k) => match root {
            Value::Object(m) => match m.get(k) {
                Some(child) => select(child, rest, display, allow_missing),
                None if allow_missing => Ok(vec![]),
                None => Err(DotpickError::PathNotFound {
                    path: display.to_string(),
                    hint: nearest_key(m, k),
                }),
            },
            other => mismatch_vec(allow_missing, display, "an object", other),
        },
        Segment::Index(i) => match root {
            Value::Array(a) => match a.get(*i) {
                Some(child) => select(child, rest, display, allow_missing),
                None if allow_missing => Ok(vec![]),
                None => Err(DotpickError::PathNotFound {
                    path: display.to_string(),
                    hint: Some(format!("array has {} element(s)", a.len())),
                }),
            },
            other => mismatch_vec(allow_missing, display, "an array", other),
        },
        Segment::Iter => match root {
            Value::Array(a) => {
                let mut out = Vec::new();
                for el in a {
                    out.extend(select(el, rest, display, allow_missing)?);
                }
                Ok(out)
            }
            other => mismatch_vec(allow_missing, display, "an array", other),
        },
    }
}

fn mismatch(
    allow_missing: bool,
    display: &str,
    expected: &str,
    found: &Value,
) -> Result<Option<Value>, DotpickError> {
    if allow_missing {
        Ok(None)
    } else {
        Err(DotpickError::TypeMismatch {
            path: display.to_string(),
            expected: expected.to_string(),
            found: type_name(found).to_string(),
        })
    }
}

fn mismatch_vec(
    allow_missing: bool,
    display: &str,
    expected: &str,
    found: &Value,
) -> Result<Vec<Value>, DotpickError> {
    if allow_missing {
        Ok(vec![])
    } else {
        Err(DotpickError::TypeMismatch {
            path: display.to_string(),
            expected: expected.to_string(),
            found: type_name(found).to_string(),
        })
    }
}

/// Deep merge of two pruned values that came from the same root.
fn merge(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Object(mut ma), Value::Object(mb)) => {
            for (k, vb) in mb {
                let merged = match ma.remove(&k) {
                    Some(va) => merge(va, vb),
                    None => vb,
                };
                ma.insert(k, merged);
            }
            Value::Object(ma)
        }
        (Value::Array(aa), Value::Array(ba)) => {
            let mut a_it = aa.into_iter();
            let mut b_it = ba.into_iter();
            let mut out = Vec::new();
            loop {
                match (a_it.next(), b_it.next()) {
                    (Some(x), Some(y)) => out.push(merge(x, y)),
                    (Some(x), None) => out.push(x),
                    (None, Some(y)) => out.push(y),
                    (None, None) => break,
                }
            }
            Value::Array(out)
        }
        // Disjoint leaves: the later path wins (paths select distinct leaves).
        (_, b) => b,
    }
}

/// The existing key closest to `missing`, for a "did you mean" hint.
fn nearest_key(map: &Map<String, Value>, missing: &str) -> Option<String> {
    map.keys()
        .map(|k| (levenshtein(k, missing), k))
        .filter(|(d, _)| *d <= 3)
        .min_by_key(|(d, _)| *d)
        .map(|(_, k)| k.clone())
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}
