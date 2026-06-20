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
use std::collections::BTreeMap;

/// A sparse intermediate used while pruning so that array positions survive the
/// merging of separately-selected indices (`.items[0].a,.items[1].b` must keep
/// element 0 and element 1 distinct). Compacted to a `Value` at the very end,
/// dropping unselected indices while preserving relative order.
enum Slice {
    /// A fully-selected subtree, copied verbatim.
    Leaf(Value),
    Object(BTreeMap<String, Slice>),
    Array(BTreeMap<usize, Slice>),
}

impl Slice {
    fn into_value(self) -> Value {
        match self {
            Slice::Leaf(v) => v,
            Slice::Object(m) => {
                Value::Object(m.into_iter().map(|(k, s)| (k, s.into_value())).collect())
            }
            Slice::Array(m) => Value::Array(m.into_values().map(Slice::into_value).collect()),
        }
    }
}

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
    let mut acc: Option<Slice> = None;
    for p in paths {
        if let Some(pruned) = prune(root, &p.segments, &p.display, allow_missing)? {
            acc = Some(match acc {
                Some(existing) => merge(existing, pruned),
                None => pruned,
            });
        }
    }
    Ok(acc.map(Slice::into_value).unwrap_or(Value::Null))
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

/// Prune `root` to the single path, preserving structure as a [`Slice`].
/// Returns `None` when the path is absent and `allow_missing` is set.
fn prune(
    root: &Value,
    segs: &[Segment],
    display: &str,
    allow_missing: bool,
) -> Result<Option<Slice>, DotpickError> {
    let Some((head, rest)) = segs.split_first() else {
        return Ok(Some(Slice::Leaf(root.clone())));
    };
    match head {
        Segment::Key(k) => match root {
            Value::Object(m) => match m.get(k) {
                Some(child) => Ok(prune(child, rest, display, allow_missing)?.map(|s| {
                    let mut obj = BTreeMap::new();
                    obj.insert(k.clone(), s);
                    Slice::Object(obj)
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
                Some(child) => Ok(prune(child, rest, display, allow_missing)?.map(|s| {
                    let mut arr = BTreeMap::new();
                    arr.insert(*i, s);
                    Slice::Array(arr)
                })),
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
                let mut arr = BTreeMap::new();
                for (idx, el) in a.iter().enumerate() {
                    if let Some(s) = prune(el, rest, display, allow_missing)? {
                        arr.insert(idx, s);
                    }
                }
                Ok(Some(Slice::Array(arr)))
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
) -> Result<Option<Slice>, DotpickError> {
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

/// Deep merge of two pruned slices that came from the same root. Objects merge
/// by key and arrays merge by true index, so separately-selected positions stay
/// aligned.
fn merge(a: Slice, b: Slice) -> Slice {
    match (a, b) {
        (Slice::Object(mut ma), Slice::Object(mb)) => {
            for (k, sb) in mb {
                let merged = match ma.remove(&k) {
                    Some(sa) => merge(sa, sb),
                    None => sb,
                };
                ma.insert(k, merged);
            }
            Slice::Object(ma)
        }
        (Slice::Array(mut ma), Slice::Array(mb)) => {
            for (i, sb) in mb {
                let merged = match ma.remove(&i) {
                    Some(sa) => merge(sa, sb),
                    None => sb,
                };
                ma.insert(i, merged);
            }
            Slice::Array(ma)
        }
        // A whole-subtree selection supersedes a narrower one (e.g. `.a,.a.b`).
        (Slice::Leaf(v), _) | (_, Slice::Leaf(v)) => Slice::Leaf(v),
        // Mismatched containers cannot arise from one root; keep the later one.
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
