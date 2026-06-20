//! dotpick: token-minimal field projection over JSON, YAML, TOML and NDJSON.
//!
//! Select fields by dotpath and emit the smallest valid slice. The whole
//! pipeline is reachable through [`run`], which the CLI and the tests both use.

mod error;
mod input;
mod output;
mod path;
mod project;
pub mod schema;

pub use error::DotpickError;

/// Input formats dotpick can read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    Json,
    Yaml,
    Toml,
    Ndjson,
}

/// Output formats dotpick can write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Yaml,
    Toml,
    Ndjson,
    /// Bare scalar values, one per line.
    Raw,
}

/// A complete projection request.
#[derive(Debug, Clone)]
pub struct Request {
    /// The raw document text.
    pub input: String,
    /// Forced input format, or `None` to auto-detect.
    pub from: Option<InputFormat>,
    /// Comma-separated dotpaths.
    pub paths: String,
    /// Key each leaf by its final name instead of preserving structure.
    pub flat: bool,
    /// Forced output format, or `None` for the default (NDJSON in, JSON otherwise).
    pub to: Option<OutputFormat>,
    /// Skip absent paths instead of erroring.
    pub allow_missing: bool,
    /// Pretty-print JSON output.
    pub pretty: bool,
}

/// Run a projection and return the rendered output (without a trailing newline).
pub fn run(req: &Request) -> Result<String, DotpickError> {
    let paths = path::parse_paths(&req.paths)?;
    let records = input::parse(&req.input, req.from)?;
    let format = req.to.unwrap_or_else(|| default_output(req.from));

    match format {
        OutputFormat::Raw => return output::render_raw(&records, &paths, req.allow_missing),
        OutputFormat::Ndjson => return output::render_ndjson(&records, &paths, req.allow_missing),
        _ => {}
    }

    let mut docs = Vec::with_capacity(records.len());
    for record in &records {
        let doc = if req.flat {
            project::flat(record, &paths, req.allow_missing)?
        } else {
            project::structured(record, &paths, req.allow_missing)?
        };
        docs.push(doc);
    }
    output::render(&docs, format, req.pretty)
}

fn default_output(from: Option<InputFormat>) -> OutputFormat {
    match from {
        Some(InputFormat::Ndjson) => OutputFormat::Ndjson,
        _ => OutputFormat::Json,
    }
}
