use archy_cli::{EvaluateInput, GenerateInput, GenerationMode, MutationOptions, VariantSelection};
use archy_core::{ArchyError, ErrorCode};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
mod doctor;
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
    #[command(about = "Read bounded desktop contracts and selected project status")]
    Desktop {
        #[command(subcommand)]
        command: DesktopCommand,
    },
    #[command(about = "Inspect native Omarchy prerequisites without changing the host")]
    Doctor {
        #[arg(long)]
        json: bool,
    },
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
    /// Generate sampled variants or edit one variant from an accepted run.
    Generate {
        #[arg(long)]
        project: PathBuf,
        #[arg(long, requires = "seed", conflicts_with = "from_run")]
        count: Option<u32>,
        #[arg(long, requires = "count", conflicts_with = "from_run")]
        seed: Option<u64>,
        #[arg(long, requires_all = ["variant", "params"], conflicts_with_all = ["count", "seed"])]
        from_run: Option<String>,
        #[arg(long, requires = "from_run")]
        variant: Option<String>,
        #[arg(long, requires = "from_run")]
        params: Option<String>,
        #[arg(long)]
        idempotency_key: Option<String>,
        #[arg(long)]
        expected_project_revision: Option<u64>,
        #[arg(long, default_value = "runs")]
        destination: String,
        #[arg(long)]
        json: bool,
    },
    /// Compare every candidate in an accepted generation or edit batch.
    Compare {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        run: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Evaluate variants from an accepted immutable run.
    Evaluate {
        #[arg(long)]
        project: PathBuf,
        #[arg(long, conflicts_with = "all", required_unless_present = "all")]
        variant: Option<String>,
        #[arg(long, conflicts_with = "variant", required_unless_present = "variant")]
        all: bool,
        #[arg(long)]
        run: Option<String>,
        #[arg(long)]
        idempotency_key: Option<String>,
        #[arg(long)]
        expected_project_revision: Option<u64>,
        #[arg(long, default_value = "runs")]
        destination: String,
        #[arg(long)]
        json: bool,
    },
}
#[derive(Subcommand)]
enum DesktopCommand {
    Capabilities {
        #[arg(long, required = true)]
        json: bool,
    },
    Status {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        run: Option<String>,
        #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u32).range(1..=100))]
        limit: u32,
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long, required = true)]
        json: bool,
    },
    Open {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        run: String,
        #[arg(long)]
        artifact: String,
        #[arg(long, value_enum)]
        kind: OpenKind,
        #[arg(long, required = true)]
        json: bool,
    },
}
#[derive(Clone, Copy, clap::ValueEnum)]
enum OpenKind {
    Report,
    Preview,
    Blender,
}
fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    if let Command::Doctor { json } = &cli.command {
        return run_doctor(*json);
    }
    if let Command::Compare { project, run, json } = &cli.command {
        return match archy_cli::compare(project, run.as_deref()) {
            Ok(artifacts) => {
                let bytes = if *json {
                    artifacts.json
                } else {
                    artifacts.markdown
                };
                match std::io::Write::write_all(&mut std::io::stdout(), &bytes) {
                    Ok(()) => {
                        if *json {
                            println!();
                        }
                        std::process::ExitCode::SUCCESS
                    }
                    Err(e) => {
                        emit_error(
                            ArchyError::new(ErrorCode::InternalError, "stdout", e.to_string()),
                            *json,
                        );
                        std::process::ExitCode::FAILURE
                    }
                }
            }
            Err(e) => {
                emit_error(e, *json);
                std::process::ExitCode::FAILURE
            }
        };
    }
    let json = matches!(
        &cli.command,
        Command::Desktop { .. }
            | Command::Validate { json: true, .. }
            | Command::Generate { json: true, .. }
            | Command::Evaluate { json: true, .. }
    );
    let desktop = matches!(&cli.command, Command::Desktop { .. });
    let result = match cli.command {
        Command::Desktop { command } => match command {
            DesktopCommand::Capabilities { .. } => {
                serde_json::to_value(archy_cli::desktop_capabilities())
                    .map_err(|e| ArchyError::new(ErrorCode::InternalError, "$", e.to_string()))
            }
            DesktopCommand::Status {
                project,
                run,
                limit,
                cursor,
                ..
            } => archy_cli::desktop_status(archy_cli::DesktopStatusInput {
                project,
                run,
                limit,
                cursor,
            })
            .and_then(|value| {
                serde_json::to_value(value)
                    .map_err(|e| ArchyError::new(ErrorCode::InternalError, "$", e.to_string()))
            }),
            DesktopCommand::Open {
                project,
                run,
                artifact,
                kind,
                ..
            } => archy_cli::desktop_open(archy_cli::DesktopOpenInput {
                project,
                run,
                artifact,
                kind: match kind {
                    OpenKind::Report => archy_cli::DesktopOpenKind::Report,
                    OpenKind::Preview => archy_cli::DesktopOpenKind::Preview,
                    OpenKind::Blender => archy_cli::DesktopOpenKind::Blender,
                },
            })
            .and_then(|value| {
                serde_json::to_value(value)
                    .map_err(|e| ArchyError::new(ErrorCode::InternalError, "$", e.to_string()))
            }),
        },
        Command::Doctor { .. } | Command::Compare { .. } => {
            unreachable!("handled before legacy dispatch")
        }
        Command::Init { name, dir } => {
            init::run(&name, &dir).map(|()| serde_json::json!({"project":dir.join("project.yaml")}))
        }
        Command::Validate { project, .. } => validate::run(&project),
        Command::Generate {
            project,
            count,
            seed,
            from_run,
            variant,
            params,
            idempotency_key,
            expected_project_revision,
            destination,
            ..
        } => {
            let mode = match (from_run, variant, params, count, seed) {
                (Some(from_run), Some(variant), Some(params), None, None) => GenerationMode::Edit {
                    from_run,
                    variant,
                    params,
                },
                (None, None, None, Some(count), Some(seed)) => {
                    GenerationMode::Sample { count, seed }
                }
                _ => {
                    return invalid_arguments(
                        "generate requires --count and --seed, or --from-run, --variant and --params",
                        json,
                    );
                }
            };
            archy_cli::generate(GenerateInput {
                project,
                mode,
                mutation: MutationOptions {
                    idempotency_key,
                    expected_project_revision,
                    destination,
                },
            })
            .and_then(|receipt| {
                serde_json::to_value(receipt)
                    .map_err(|e| ArchyError::new(ErrorCode::InternalError, "$", e.to_string()))
            })
        }
        Command::Evaluate {
            project,
            variant,
            all,
            run,
            idempotency_key,
            expected_project_revision,
            destination,
            ..
        } => {
            let selection = match (variant, all) {
                (Some(id), false) => VariantSelection::One(id),
                (None, true) => VariantSelection::All,
                _ => return invalid_arguments("evaluate requires --variant or --all", json),
            };
            archy_cli::evaluate(EvaluateInput {
                project,
                source_run: run,
                selection,
                mutation: MutationOptions {
                    idempotency_key,
                    expected_project_revision,
                    destination,
                },
            })
            .and_then(|receipt| {
                serde_json::to_value(receipt)
                    .map_err(|e| ArchyError::new(ErrorCode::InternalError, "$", e.to_string()))
            })
        }
    };
    match result {
        Ok(value) => match if desktop {
            archy_cli::ascii_json_pretty(&value)
        } else {
            serde_json::to_string_pretty(&value)
        } {
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
fn invalid_arguments(message: &str, json: bool) -> std::process::ExitCode {
    emit_error(
        ArchyError::new(ErrorCode::InvalidSchema, "$", message),
        json,
    );
    std::process::ExitCode::FAILURE
}
fn run_doctor(json: bool) -> std::process::ExitCode {
    let policy = match doctor::CompatibilityPolicy::checked_in() {
        Ok(policy) => policy,
        Err(e) => {
            emit_error(ArchyError::new(ErrorCode::InternalError, "$", e), json);
            return std::process::ExitCode::from(2);
        }
    };
    let report = doctor::evaluate(&doctor::collect(), &policy);
    if json {
        match archy_cli::ascii_json_pretty(&report.json()) {
            Ok(text) => println!("{text}"),
            Err(e) => {
                emit_error(
                    ArchyError::new(ErrorCode::InternalError, "$", e.to_string()),
                    true,
                );
                return std::process::ExitCode::from(2);
            }
        }
    } else {
        print!("{}", report.text());
    }
    std::process::ExitCode::from(report.exit_code())
}
fn emit_error(error: ArchyError, json: bool) {
    if json {
        match archy_cli::ascii_json_compact(&error) {
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
