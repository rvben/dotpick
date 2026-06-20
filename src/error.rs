//! Error type and the stable exit-code contract.
//!
//! Exit codes are part of the public contract (see `dotpick schema`):
//! - `0` success
//! - `1` no match (a selected path is absent, or the projection is empty)
//! - `2` parse or serialize failure (malformed input, or output format cannot
//!   represent the result)
//! - `3` usage error (bad dotpath syntax, conflicting options, raw on non-scalar)

use thiserror::Error;

/// All failure modes of a projection run.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DotpickError {
    /// The input could not be parsed in the (detected or forced) format.
    #[error("could not parse input as {format}: {message}")]
    Parse { format: String, message: String },

    /// A dotpath string is syntactically invalid.
    #[error("invalid dotpath {path:?}: {message}")]
    PathSyntax { path: String, message: String },

    /// A selected path does not exist in the document.
    #[error("path {path} not found{}", hint_suffix(.hint))]
    PathNotFound { path: String, hint: Option<String> },

    /// A path segment was applied to the wrong kind of value
    /// (e.g. indexing an object, or keying into a scalar).
    #[error("path {path}: expected {expected} but found {found}")]
    TypeMismatch {
        path: String,
        expected: String,
        found: String,
    },

    /// Two selected leaves collapse to the same name in `--flat` output.
    #[error(
        "flat output has a name collision on {name:?}; use the default structured output or rename with a quoted path"
    )]
    NameCollision { name: String },

    /// `--to raw` was requested but a selected value is not a scalar.
    #[error("raw output requires scalar values, but {path} is {found}; use --to json")]
    RawNonScalar { path: String, found: String },

    /// The result cannot be serialized in the requested output format.
    #[error("cannot serialize result as {format}: {message}")]
    Serialize { format: String, message: String },
}

fn hint_suffix(hint: &Option<String>) -> String {
    match hint {
        Some(h) => format!("; nearest existing: {h}"),
        None => String::new(),
    }
}

impl DotpickError {
    /// The process exit code associated with this error.
    pub fn exit_code(&self) -> i32 {
        match self {
            DotpickError::Parse { .. } | DotpickError::Serialize { .. } => 2,
            DotpickError::PathNotFound { .. } | DotpickError::TypeMismatch { .. } => 1,
            DotpickError::PathSyntax { .. }
            | DotpickError::NameCollision { .. }
            | DotpickError::RawNonScalar { .. } => 3,
        }
    }
}
