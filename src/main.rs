//! dotpick CLI: read a document from a file or stdin, project the given
//! dotpaths, and write the smallest valid slice to stdout.
//!
//! Follows The CLI Spec (clispec.dev): structured output on stdout, structured
//! error envelopes on the last line of stderr, a `schema` subcommand, and
//! non-interactive, read-only behavior.

use std::io::{Read, Write};
use std::path::Path;
use std::process::ExitCode;

use clap::error::ErrorKind as ClapErrorKind;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use dotpick::{DotpickError, InputFormat, OutputFormat, Request, run, schema};
use serde_json::json;

#[derive(Parser)]
#[command(
    name = "dotpick",
    version,
    about = "Token-minimal field projection over JSON, YAML, TOML and NDJSON.",
    long_about = "Select fields by dotpath and emit the smallest valid slice. \
                  Pure projection and format conversion: no computation, no mutation.\n\n\
                  Dotpaths: .key  [\"quoted.key\"]  [0]  []  (chain freely; comma-separate multiple).\n\n\
                  Run `dotpick schema` for the machine-readable contract (clispec.dev).",
    args_conflicts_with_subcommands = true,
    subcommand_negates_reqs = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Comma-separated dotpaths, e.g. '.metadata.name,.spec.replicas'.
    #[arg(value_name = "PATHS")]
    paths: Option<String>,

    /// Input file; omit to read stdin.
    #[arg(value_name = "FILE")]
    file: Option<String>,

    /// Force the input format (default: auto-detect, or from the file extension).
    #[arg(long, value_enum, global = true)]
    from: Option<CliInputFormat>,

    /// Output format; auto = json, or ndjson when the input is ndjson.
    #[arg(
        long,
        short = 'o',
        visible_alias = "to",
        value_enum,
        default_value = "auto",
        global = true
    )]
    output: CliOutputFormat,

    /// Key each selected leaf by its final name instead of preserving structure.
    #[arg(long, global = true)]
    flat: bool,

    /// Skip absent paths instead of erroring.
    #[arg(long, global = true)]
    allow_missing: bool,

    /// Pretty-print JSON output.
    #[arg(long, global = true)]
    pretty: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Project dotpaths from a document (also the default with no subcommand).
    Project {
        /// Comma-separated dotpaths, e.g. '.metadata.name,.spec.replicas'.
        #[arg(value_name = "PATHS")]
        paths: String,
        /// Input file; omit to read stdin.
        #[arg(value_name = "FILE")]
        file: Option<String>,
    },
    /// Print the machine-readable contract (clispec.dev) as JSON.
    Schema,
    /// Generate a shell completion script.
    Completions {
        /// Target shell.
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum CliInputFormat {
    Json,
    Yaml,
    Toml,
    Ndjson,
}

#[derive(Clone, Copy, ValueEnum)]
enum CliOutputFormat {
    /// JSON, or NDJSON when the input is NDJSON.
    Auto,
    Json,
    Yaml,
    Toml,
    Ndjson,
    Raw,
}

impl From<CliInputFormat> for InputFormat {
    fn from(f: CliInputFormat) -> Self {
        match f {
            CliInputFormat::Json => InputFormat::Json,
            CliInputFormat::Yaml => InputFormat::Yaml,
            CliInputFormat::Toml => InputFormat::Toml,
            CliInputFormat::Ndjson => InputFormat::Ndjson,
        }
    }
}

impl CliOutputFormat {
    /// `auto` defers to the input-driven default; everything else is explicit.
    fn resolve(self) -> Option<OutputFormat> {
        match self {
            CliOutputFormat::Auto => None,
            CliOutputFormat::Json => Some(OutputFormat::Json),
            CliOutputFormat::Yaml => Some(OutputFormat::Yaml),
            CliOutputFormat::Toml => Some(OutputFormat::Toml),
            CliOutputFormat::Ndjson => Some(OutputFormat::Ndjson),
            CliOutputFormat::Raw => Some(OutputFormat::Raw),
        }
    }
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => return handle_clap_error(e),
    };

    // Subcommands that produce their own output and return early.
    let (paths, file) = match &cli.command {
        Some(Command::Schema) => {
            println!("{}", schema::contract_json());
            return ExitCode::SUCCESS;
        }
        Some(Command::Completions { shell }) => {
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            clap_complete::generate(*shell, &mut cmd, name, &mut std::io::stdout());
            return ExitCode::SUCCESS;
        }
        Some(Command::Project { paths, file }) => (Some(paths.clone()), file.clone()),
        None => (cli.paths.clone(), cli.file.clone()),
    };

    match project(&cli, paths, file) {
        Ok(output) => {
            // write! so a broken pipe (e.g. `| head`) exits cleanly.
            let _ = writeln!(std::io::stdout(), "{output}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            emit_error(err.kind(), &err.to_string(), err.exit_code(), err.hint());
            ExitCode::from(err.exit_code() as u8)
        }
    }
}

/// Help and version print normally and exit 0; every other clap failure becomes
/// a structured `usage` error envelope (so a bad invocation is still
/// machine-parseable, per clispec).
fn handle_clap_error(e: clap::Error) -> ExitCode {
    match e.kind() {
        ClapErrorKind::DisplayHelp
        | ClapErrorKind::DisplayVersion
        | ClapErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            let _ = e.print();
            ExitCode::SUCCESS
        }
        _ => {
            let err = DotpickError::Usage {
                message: e.to_string().trim().to_string(),
            };
            emit_error(err.kind(), &err.to_string(), err.exit_code(), err.hint());
            ExitCode::from(err.exit_code() as u8)
        }
    }
}

/// Run a projection from the resolved `paths`/`file` and global flags.
/// `paths` is checked here because the bare-default form makes it optional.
fn project(cli: &Cli, paths: Option<String>, file: Option<String>) -> Result<String, DotpickError> {
    let paths = paths.ok_or_else(|| DotpickError::Usage {
        message: "no paths given".to_string(),
    })?;

    let input = read_input(file.as_deref()).map_err(|e| DotpickError::Io {
        message: e.to_string(),
    })?;

    let from = cli
        .from
        .map(InputFormat::from)
        .or_else(|| file.as_deref().and_then(format_from_extension));

    let request = Request {
        input,
        from,
        paths,
        flat: cli.flat,
        to: cli.output.resolve(),
        allow_missing: cli.allow_missing,
        pretty: cli.pretty,
    };

    run(&request)
}

/// Write the clispec error envelope as the last line of stderr.
fn emit_error(kind: &str, message: &str, exit_code: i32, hint: Option<&str>) {
    let mut error = serde_json::Map::new();
    error.insert("kind".into(), json!(kind));
    error.insert("message".into(), json!(message));
    error.insert("exit_code".into(), json!(exit_code));
    if let Some(hint) = hint {
        error.insert("hint".into(), json!(hint));
    }
    let envelope = json!({ "error": error });
    eprintln!("{envelope}");
}

fn read_input(file: Option<&str>) -> std::io::Result<String> {
    match file {
        Some(path) => std::fs::read_to_string(path),
        None => {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            Ok(buf)
        }
    }
}

fn format_from_extension(file: &str) -> Option<InputFormat> {
    match Path::new(file)
        .extension()
        .and_then(|e| e.to_str())?
        .to_ascii_lowercase()
        .as_str()
    {
        "json" => Some(InputFormat::Json),
        "yaml" | "yml" => Some(InputFormat::Yaml),
        "toml" => Some(InputFormat::Toml),
        "ndjson" | "jsonl" => Some(InputFormat::Ndjson),
        _ => None,
    }
}
