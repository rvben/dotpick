//! dotpick CLI: read a document from a file or stdin, project the given
//! dotpaths, and write the smallest valid slice to stdout.

use std::io::{Read, Write};
use std::path::Path;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use dotpick::{InputFormat, OutputFormat, Request, run, schema};

#[derive(Parser)]
#[command(
    name = "dotpick",
    version,
    about = "Token-minimal field projection over JSON, YAML, TOML and NDJSON.",
    long_about = "Select fields by dotpath and emit the smallest valid slice. \
                  Pure projection and format conversion: no computation, no mutation.\n\n\
                  Dotpaths: .key  [\"quoted.key\"]  [0]  []  (chain freely; comma-separate multiple).",
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
    #[arg(long, value_enum)]
    from: Option<CliInputFormat>,

    /// Force the output format (default: json, or ndjson when input is ndjson).
    #[arg(long, value_enum)]
    to: Option<CliOutputFormat>,

    /// Key each selected leaf by its final name instead of preserving structure.
    #[arg(long)]
    flat: bool,

    /// Skip absent paths instead of erroring.
    #[arg(long)]
    allow_missing: bool,

    /// Pretty-print JSON output.
    #[arg(long)]
    pretty: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Print the machine-readable contract (for agents) as JSON.
    Schema,
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

impl From<CliOutputFormat> for OutputFormat {
    fn from(f: CliOutputFormat) -> Self {
        match f {
            CliOutputFormat::Json => OutputFormat::Json,
            CliOutputFormat::Yaml => OutputFormat::Yaml,
            CliOutputFormat::Toml => OutputFormat::Toml,
            CliOutputFormat::Ndjson => OutputFormat::Ndjson,
            CliOutputFormat::Raw => OutputFormat::Raw,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if let Some(Command::Schema) = cli.command {
        println!("{}", schema::contract_json());
        return ExitCode::SUCCESS;
    }

    match project(&cli) {
        Ok(output) => {
            // write! so a broken pipe (e.g. `| head`) exits cleanly.
            let mut stdout = std::io::stdout();
            let _ = writeln!(stdout, "{output}");
            ExitCode::SUCCESS
        }
        Err(code) => ExitCode::from(code),
    }
}

/// Run the projection, mapping any failure to a process exit code after
/// printing a diagnostic to stderr. Returns the rendered output on success.
fn project(cli: &Cli) -> Result<String, u8> {
    let Some(paths) = cli.paths.clone() else {
        eprintln!("dotpick: no paths given (try `dotpick --help` or `dotpick schema`)");
        return Err(3);
    };

    let input = match read_input(cli.file.as_deref()) {
        Ok(input) => input,
        Err(e) => {
            eprintln!("dotpick: {e}");
            return Err(2);
        }
    };

    let from = cli
        .from
        .map(InputFormat::from)
        .or_else(|| cli.file.as_deref().and_then(format_from_extension));

    let request = Request {
        input,
        from,
        paths,
        flat: cli.flat,
        to: cli.to.map(OutputFormat::from),
        allow_missing: cli.allow_missing,
        pretty: cli.pretty,
    };

    run(&request).map_err(|err| {
        eprintln!("dotpick: {err}");
        err.exit_code() as u8
    })
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
