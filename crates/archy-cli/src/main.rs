use archy_core::{ArchyError, ErrorCode};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
mod init;
mod validate;

#[derive(Parser)]
#[command(
    name = "archy",
    version,
    about = "Local architectural massing briefs; no regulatory certification"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Create a new project directory; refuse all occupied targets.
    Init {
        #[arg(long)]
        name: String,
        #[arg(long)]
        dir: PathBuf,
    },
    /// Validate a YAML or JSON manifest file and normalize it to SI.
    Validate {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        json: bool,
    },
}
fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    let json = matches!(&cli.command, Command::Validate { json: true, .. });
    let result = match cli.command {
        Command::Init { name, dir } => {
            init::run(&name, &dir).map(|()| serde_json::json!({"project":dir.join("project.yaml")}))
        }
        Command::Validate { project, .. } => validate::run(&project),
    };
    match result {
        Ok(value) => match serde_json::to_string_pretty(&value) {
            Ok(text) => {
                println!("{text}");
                std::process::ExitCode::SUCCESS
            }
            Err(e) => {
                emit_error(
                    ArchyError::new(ErrorCode::InternalError, "$", e.to_string()),
                    json,
                );
                std::process::ExitCode::FAILURE
            }
        },
        Err(e) => {
            emit_error(e, json);
            std::process::ExitCode::FAILURE
        }
    }
}
fn emit_error(error: ArchyError, json: bool) {
    if json {
        match serde_json::to_string(&error) {
            Ok(text) => println!("{text}"),
            Err(_) => println!(
                "{{\"code\":\"INTERNAL_ERROR\",\"message\":\"Error serialization failed\",\"field\":\"$\",\"retriable\":false}}"
            ),
        }
    } else {
        eprintln!("{error}");
    }
}
pub(crate) fn io_error(path: &std::path::Path, e: std::io::Error) -> ArchyError {
    let code = match e.kind() {
        std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::AlreadyExists => {
            ErrorCode::PermissionDenied
        }
        _ => ErrorCode::InvalidSchema,
    };
    ArchyError::new(code, path.display().to_string(), e.to_string())
}
