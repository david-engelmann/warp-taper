use std::path::PathBuf;
use std::time::Duration;

use clap::{Parser, Subcommand};
use warp_taper_core::{
    assertion::{Assertion, ShellScriptAssertion},
    NoOpRecorder, Pipeline, RecordTrigger, Scenario,
};

#[derive(Parser, Debug)]
#[command(
    name = "warp-taper",
    version,
    about = "Record Warp behavior into PR-ready evidence bundles."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run the full pipeline against a scenario directory.
    Run {
        /// Path to the scenario directory (contains metadata.yaml and optionally assertions.sh).
        scenario_dir: PathBuf,

        /// Where to land the tape bundle. Defaults to `tapes/<slug>` under the cwd.
        #[arg(long)]
        tape_dir: Option<PathBuf>,

        /// Warp source checkout (cargo workspace root that produces warp-oss).
        #[arg(long, env = "WARP_SOURCE")]
        warp_source: PathBuf,

        /// Cargo package name inside `--warp-source`.
        #[arg(long, default_value = "warp")]
        package: String,

        /// Compiled binary name inside `<warp-source>/target/debug/`.
        #[arg(long, default_value = "warp-oss")]
        binary_name: String,

        /// Path to warp-oss's log file. Defaults to ~/Library/Logs/warp-oss.log.
        #[arg(long)]
        warp_log: Option<PathBuf>,

        /// Skip the real macOS screen recorder and use the no-op recorder.
        /// Useful for CI / scripted runs.
        #[arg(long)]
        no_screencapture: bool,

        /// Stop the recorder after this many milliseconds instead of waiting
        /// for stdin (the interactive mode the bash pipeline uses).
        #[arg(long)]
        duration_ms: Option<u64>,
    },

    /// Run a built-in (Rust-authored) scenario by name.
    /// Use `list-builtins` to see what's available.
    RunBuiltin {
        /// Built-in scenario slug (e.g. `mcp-log-rotation`).
        name: String,

        #[arg(long)]
        tape_dir: Option<PathBuf>,

        #[arg(long, env = "WARP_SOURCE")]
        warp_source: PathBuf,

        #[arg(long, default_value = "warp")]
        package: String,

        #[arg(long, default_value = "warp-oss")]
        binary_name: String,

        #[arg(long)]
        warp_log: Option<PathBuf>,

        #[arg(long)]
        no_screencapture: bool,

        #[arg(long)]
        duration_ms: Option<u64>,
    },

    /// List built-in scenarios that can be run via `run-builtin`.
    ListBuiltins,

    /// Print version.
    Version,
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Version => {
            println!("warp-taper {}", env!("CARGO_PKG_VERSION"));
            std::process::ExitCode::SUCCESS
        }
        Command::ListBuiltins => {
            for name in warp_taper_core::scenarios::names() {
                println!("{name}");
            }
            std::process::ExitCode::SUCCESS
        }
        Command::Run {
            scenario_dir,
            tape_dir,
            warp_source,
            package,
            binary_name,
            warp_log,
            no_screencapture,
            duration_ms,
        } => dispatch(run_yaml_pipeline(YamlRunArgs {
            scenario_dir,
            tape_dir,
            warp_source,
            package,
            binary_name,
            warp_log,
            no_screencapture,
            duration_ms,
        })),
        Command::RunBuiltin {
            name,
            tape_dir,
            warp_source,
            package,
            binary_name,
            warp_log,
            no_screencapture,
            duration_ms,
        } => dispatch(run_builtin_pipeline(BuiltinRunArgs {
            name,
            tape_dir,
            warp_source,
            package,
            binary_name,
            warp_log,
            no_screencapture,
            duration_ms,
        })),
    }
}

fn dispatch(result: warp_taper_core::Result<bool>) -> std::process::ExitCode {
    match result {
        Ok(true) => std::process::ExitCode::SUCCESS,
        Ok(false) => std::process::ExitCode::from(1),
        Err(e) => {
            eprintln!("warp-taper: {e}");
            std::process::ExitCode::from(2)
        }
    }
}

struct YamlRunArgs {
    scenario_dir: PathBuf,
    tape_dir: Option<PathBuf>,
    warp_source: PathBuf,
    package: String,
    binary_name: String,
    warp_log: Option<PathBuf>,
    no_screencapture: bool,
    duration_ms: Option<u64>,
}

struct BuiltinRunArgs {
    name: String,
    tape_dir: Option<PathBuf>,
    warp_source: PathBuf,
    package: String,
    binary_name: String,
    warp_log: Option<PathBuf>,
    no_screencapture: bool,
    duration_ms: Option<u64>,
}

fn run_builtin_pipeline(args: BuiltinRunArgs) -> warp_taper_core::Result<bool> {
    let factory = warp_taper_core::scenarios::by_name(&args.name).ok_or_else(|| {
        warp_taper_core::Error::ScenarioInvalid(format!(
            "unknown built-in scenario {:?}; try `warp-taper list-builtins`",
            args.name
        ))
    })?;
    let (scenario, assertions) = factory()?;
    let tape_dir = args
        .tape_dir
        .unwrap_or_else(|| PathBuf::from("tapes").join(&scenario.slug));
    drive_pipeline(
        scenario,
        assertions,
        args.warp_source,
        tape_dir,
        args.package,
        args.binary_name,
        args.warp_log,
        args.no_screencapture,
        args.duration_ms,
    )
}

fn run_yaml_pipeline(args: YamlRunArgs) -> warp_taper_core::Result<bool> {
    let metadata_path = args.scenario_dir.join("metadata.yaml");
    let scenario = Scenario::from_yaml_file(&metadata_path)?;

    let tape_dir = args
        .tape_dir
        .unwrap_or_else(|| PathBuf::from("tapes").join(&scenario.slug));

    let mut assertions: Vec<Box<dyn Assertion>> = Vec::new();
    let assertions_sh = args.scenario_dir.join("assertions.sh");
    if assertions_sh.is_file() {
        assertions.push(Box::new(
            ShellScriptAssertion::new("assertions.sh", &assertions_sh)
                .with_working_dir(&args.scenario_dir),
        ));
    }

    drive_pipeline(
        scenario,
        assertions,
        args.warp_source,
        tape_dir,
        args.package,
        args.binary_name,
        args.warp_log,
        args.no_screencapture,
        args.duration_ms,
    )
}

#[allow(clippy::too_many_arguments)]
fn drive_pipeline(
    scenario: Scenario,
    assertions: Vec<Box<dyn Assertion>>,
    warp_source: PathBuf,
    tape_dir: PathBuf,
    package: String,
    binary_name: String,
    warp_log: Option<PathBuf>,
    no_screencapture: bool,
    duration_ms: Option<u64>,
) -> warp_taper_core::Result<bool> {
    let mut pipeline = Pipeline::new(scenario, warp_source, tape_dir)
        .with_assertions(assertions)
        .with_package(package)
        .with_binary_name(binary_name);
    if let Some(p) = warp_log {
        pipeline = pipeline.with_warp_log_path(p);
    }

    let trigger = match duration_ms {
        Some(ms) => RecordTrigger::Duration(Duration::from_millis(ms)),
        None => RecordTrigger::Interactive,
    };

    let tape = if no_screencapture {
        pipeline.run(NoOpRecorder::new(), trigger)?
    } else {
        run_with_real_recorder(&pipeline, trigger)?
    };

    eprintln!("warp-taper: tape at {}", tape.dir.display());
    eprintln!(
        "warp-taper: {} pass, {} fail",
        tape.evaluation.pass_count, tape.evaluation.fail_count
    );
    Ok(tape.evaluation.passed())
}

#[cfg(target_os = "macos")]
fn run_with_real_recorder(
    pipeline: &Pipeline,
    trigger: RecordTrigger,
) -> warp_taper_core::Result<warp_taper_core::Tape> {
    pipeline.run(warp_taper_core::MacOsScreencapture::new(), trigger)
}

#[cfg(not(target_os = "macos"))]
fn run_with_real_recorder(
    pipeline: &Pipeline,
    trigger: RecordTrigger,
) -> warp_taper_core::Result<warp_taper_core::Tape> {
    eprintln!("warp-taper: no real recorder on this platform; falling back to no-op.");
    pipeline.run(NoOpRecorder::new(), trigger)
}
