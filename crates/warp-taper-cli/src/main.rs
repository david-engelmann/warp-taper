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

    /// Print version (preserved for back-compat with the bash CLI shim).
    Version,
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Version => {
            println!("warp-taper {}", env!("CARGO_PKG_VERSION"));
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
        } => match run_pipeline(RunArgs {
            scenario_dir,
            tape_dir,
            warp_source,
            package,
            binary_name,
            warp_log,
            no_screencapture,
            duration_ms,
        }) {
            Ok(passed) => {
                if passed {
                    std::process::ExitCode::SUCCESS
                } else {
                    std::process::ExitCode::from(1)
                }
            }
            Err(e) => {
                eprintln!("warp-taper: {e}");
                std::process::ExitCode::from(2)
            }
        },
    }
}

struct RunArgs {
    scenario_dir: PathBuf,
    tape_dir: Option<PathBuf>,
    warp_source: PathBuf,
    package: String,
    binary_name: String,
    warp_log: Option<PathBuf>,
    no_screencapture: bool,
    duration_ms: Option<u64>,
}

fn run_pipeline(args: RunArgs) -> warp_taper_core::Result<bool> {
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

    let mut pipeline = Pipeline::new(scenario, args.warp_source, tape_dir)
        .with_assertions(assertions)
        .with_package(args.package)
        .with_binary_name(args.binary_name);
    if let Some(p) = args.warp_log {
        pipeline = pipeline.with_warp_log_path(p);
    }

    let trigger = match args.duration_ms {
        Some(ms) => RecordTrigger::Duration(Duration::from_millis(ms)),
        None => RecordTrigger::Interactive,
    };

    // The recorder is chosen at the call site so the type stays static and
    // we avoid trait-object machinery for a small, stable surface.
    let tape = if args.no_screencapture {
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
