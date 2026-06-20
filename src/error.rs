//! Error type, the stable error `kind` set, and the exit-code contract.
//!
//! Errors are reported as a clispec structured envelope on the last line of
//! stderr: `{"error":{"kind":...,"message":...,"exit_code":...,"hint":...}}`.
//! The `kind` values here are the finite set declared in `dotpick schema`.
//!
//! Exit codes (also declared in the schema):
//! - `1` no match (a selected path is absent, or a segment hits the wrong type)
//! - `2` parse / serialize / IO failure
//! - `3` usage error (bad arguments, bad dotpath, name collision, raw on non-scalar)

use thiserror::Error;

/// All failure modes of a projection run.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DotpickError {
    /// Invalid command-line arguments (also used for wrapped clap errors).
    #[error("{message}")]
    Usage { message: String },

    /// The input file could not be read.
    #[error("could not read input: {message}")]
    Io { message: String },

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

    /// `--output raw` was requested but a selected value is not a scalar.
    #[error("raw output requires scalar values, but {path} is {found}")]
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
    /// Stable snake_case identifier consumers branch on (the schema `errors` set).
    pub fn kind(&self) -> &'static str {
        match self {
            DotpickError::Usage { .. } => "usage",
            DotpickError::Io { .. } => "io",
            DotpickError::Parse { .. } => "parse",
            DotpickError::PathSyntax { .. } => "path_syntax",
            DotpickError::PathNotFound { .. } => "path_not_found",
            DotpickError::TypeMismatch { .. } => "type_mismatch",
            DotpickError::NameCollision { .. } => "name_collision",
            DotpickError::RawNonScalar { .. } => "raw_non_scalar",
            DotpickError::Serialize { .. } => "serialize",
        }
    }

    /// Actionable remediation, when there is one.
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            DotpickError::Usage { .. } => Some("see `dotpick --help` or `dotpick schema`"),
            DotpickError::PathNotFound { .. } => Some("pass --allow-missing to skip absent paths"),
            DotpickError::RawNonScalar { .. } => Some("use --output json to keep structure"),
            _ => None,
        }
    }

    /// The process exit code associated with this error.
    pub fn exit_code(&self) -> i32 {
        match self {
            DotpickError::Io { .. }
            | DotpickError::Parse { .. }
            | DotpickError::Serialize { .. } => 2,
            DotpickError::PathNotFound { .. } | DotpickError::TypeMismatch { .. } => 1,
            DotpickError::Usage { .. }
            | DotpickError::PathSyntax { .. }
            | DotpickError::NameCollision { .. }
            | DotpickError::RawNonScalar { .. } => 3,
        }
    }
}
