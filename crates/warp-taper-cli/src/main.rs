use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
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

        #[command(flatten)]
        common: CommonRunArgs,
    },

    /// Run a built-in (Rust-authored) scenario by name.
    /// Use `list-builtins` to see what's available.
    RunBuiltin {
        /// Built-in scenario slug (e.g. `mcp-log-rotation`).
        name: String,

        #[command(flatten)]
        common: CommonRunArgs,
    },

    /// List built-in scenarios available to `run-builtin`.
    ListBuiltins,

    /// Print metadata for a built-in scenario without running it.
    Describe {
        /// Scenario slug.
        name: String,
    },

    /// Print a starter Rust scenario file to stdout.
    Init {
        /// Slug for the new scenario (e.g. `12345-fancy-fix`).
        slug: String,

        /// One-line title for the scenario.
        #[arg(long, default_value = "TODO: scenario title")]
        title: String,

        /// Optional ticket reference (e.g. `warpdotdev/warp#10874`).
        #[arg(long)]
        ticket: Option<String>,
    },

    /// Print version.
    Version,
}

#[derive(clap::Args, Debug)]
struct CommonRunArgs {
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
    #[arg(long)]
    no_screencapture: bool,

    /// Stop the recorder after this many milliseconds instead of waiting
    /// for stdin (the interactive mode).
    #[arg(long)]
    duration_ms: Option<u64>,

    /// Abort the build stage if cargo takes longer than this. Useful for
    /// CI where a hung build would otherwise wedge the whole pipeline.
    #[arg(long)]
    build_timeout_seconds: Option<u64>,

    /// Override the detected branch (defaults to `git -C <warp-source> rev-parse --abbrev-ref HEAD`).
    #[arg(long)]
    branch: Option<String>,

    /// Override the detected head SHA (defaults to `git -C <warp-source> rev-parse --short HEAD`).
    #[arg(long)]
    head: Option<String>,
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
        Command::Describe { name } => dispatch_int(describe(&name)),
        Command::Init {
            slug,
            title,
            ticket,
        } => {
            print!(
                "{}",
                render_scenario_template(&slug, &title, ticket.as_deref())
            );
            std::process::ExitCode::SUCCESS
        }
        Command::Run {
            scenario_dir,
            common,
        } => dispatch(run_yaml_pipeline(scenario_dir, common)),
        Command::RunBuiltin { name, common } => dispatch(run_builtin_pipeline(name, common)),
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

fn dispatch_int(result: warp_taper_core::Result<()>) -> std::process::ExitCode {
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("warp-taper: {e}");
            std::process::ExitCode::from(2)
        }
    }
}

fn describe(name: &str) -> warp_taper_core::Result<()> {
    let factory = warp_taper_core::scenarios::by_name(name).ok_or_else(|| {
        warp_taper_core::Error::ScenarioInvalid(format!(
            "unknown built-in scenario {name:?}; try `warp-taper list-builtins`"
        ))
    })?;
    let (scenario, assertions) = factory()?;
    println!("slug:      {}", scenario.slug);
    println!("title:     {}", scenario.metadata.title);
    if let Some(t) = &scenario.metadata.ticket {
        println!("ticket:    {t}");
    }
    if let Some(e) = &scenario.metadata.expected {
        println!("expected:");
        for line in e.lines() {
            println!("  {line}");
        }
    }
    if !scenario.mcp_log_paths.is_empty() {
        println!("mcp_log_paths:");
        for p in &scenario.mcp_log_paths {
            println!("  - {}", p.display());
        }
    }
    println!("assertions ({}):", assertions.len());
    for a in &assertions {
        println!("  - {}", a.name());
    }
    Ok(())
}

fn run_builtin_pipeline(name: String, common: CommonRunArgs) -> warp_taper_core::Result<bool> {
    let factory = warp_taper_core::scenarios::by_name(&name).ok_or_else(|| {
        warp_taper_core::Error::ScenarioInvalid(format!(
            "unknown built-in scenario {name:?}; try `warp-taper list-builtins`"
        ))
    })?;
    let (scenario, assertions) = factory()?;
    drive_pipeline(scenario, assertions, common)
}

fn run_yaml_pipeline(
    scenario_dir: PathBuf,
    common: CommonRunArgs,
) -> warp_taper_core::Result<bool> {
    let metadata_path = scenario_dir.join("metadata.yaml");
    let scenario = Scenario::from_yaml_file(&metadata_path)?;

    let mut assertions: Vec<Box<dyn Assertion>> = Vec::new();
    let assertions_sh = scenario_dir.join("assertions.sh");
    if assertions_sh.is_file() {
        assertions.push(Box::new(
            ShellScriptAssertion::new("assertions.sh", &assertions_sh)
                .with_working_dir(&scenario_dir),
        ));
    }

    drive_pipeline(scenario, assertions, common)
}

fn drive_pipeline(
    scenario: Scenario,
    assertions: Vec<Box<dyn Assertion>>,
    args: CommonRunArgs,
) -> warp_taper_core::Result<bool> {
    install_shutdown_handler();

    let tape_dir = args
        .tape_dir
        .unwrap_or_else(|| PathBuf::from("tapes").join(&scenario.slug));

    let (branch, head) = match (args.branch.clone(), args.head.clone()) {
        (Some(b), Some(h)) => (b, h),
        (b_override, h_override) => {
            let (detected_b, detected_h) = detect_branch_head(&args.warp_source);
            (
                b_override.unwrap_or(detected_b),
                h_override.unwrap_or(detected_h),
            )
        }
    };

    let mut pipeline = Pipeline::new(scenario, args.warp_source, tape_dir)
        .with_assertions(assertions)
        .with_package(args.package)
        .with_binary_name(args.binary_name)
        .with_branch(branch)
        .with_head(head)
        .with_deploy_spawned_callback(track_deploy_pid);
    if let Some(p) = args.warp_log {
        pipeline = pipeline.with_warp_log_path(p);
    }
    if let Some(secs) = args.build_timeout_seconds {
        pipeline = pipeline.with_build_timeout(Duration::from_secs(secs));
    }

    let trigger = match args.duration_ms {
        Some(ms) => RecordTrigger::Duration(Duration::from_millis(ms)),
        None => RecordTrigger::Interactive,
    };

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

/// Run `git -C <warp-source> rev-parse ...` to detect branch + short HEAD.
/// Returns `("<unknown>", "<unknown>")` on any failure — git isn't
/// load-bearing, just a convenience for the bundle README.
fn detect_branch_head(warp_source: &Path) -> (String, String) {
    let branch = run_git(warp_source, &["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap_or_else(|| "<unknown>".to_string());
    let head = run_git(warp_source, &["rev-parse", "--short", "HEAD"])
        .unwrap_or_else(|| "<unknown>".to_string());
    (branch, head)
}

fn run_git(cwd: &Path, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// PID of the currently-deployed warp-oss process, set by the pipeline's
/// deploy-spawned callback and cleared when the pipeline exits normally.
/// The SIGINT handler reads this to kill the child before exiting.
static DEPLOY_PID: OnceLock<Mutex<Option<u32>>> = OnceLock::new();

fn deploy_pid_cell() -> &'static Mutex<Option<u32>> {
    DEPLOY_PID.get_or_init(|| Mutex::new(None))
}

fn track_deploy_pid(pid: u32) {
    *deploy_pid_cell().lock().unwrap() = Some(pid);
}

fn install_shutdown_handler() {
    let _ = ctrlc::set_handler(|| {
        let pid = *deploy_pid_cell().lock().unwrap();
        if let Some(pid) = pid {
            #[cfg(unix)]
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGTERM);
            }
        }
        eprintln!("warp-taper: interrupted, exiting.");
        std::process::exit(130);
    });
}

fn render_scenario_template(slug: &str, title: &str, ticket: Option<&str>) -> String {
    let mut ident = slug.replace('-', "_");
    if ident.starts_with(|c: char| c.is_ascii_digit()) {
        ident.insert(0, '_');
    }
    let ticket_line = match ticket {
        Some(t) => format!("        .ticket(\"{t}\")"),
        None => "        // .ticket(\"owner/repo#NNN\")".to_string(),
    };
    let body = [
        format!("//! Built-in scenario: {title}."),
        String::new(),
        "use crate::assertion::{Assertion, McpLogSnapshotCaptured};".to_string(),
        "use crate::error::Result;".to_string(),
        "use crate::scenario::Scenario;".to_string(),
        "use crate::scenarios::Builtin;".to_string(),
        String::new(),
        format!("pub fn {ident}() -> Result<Builtin> {{"),
        format!("    let scenario = Scenario::builder(\"{slug}\")"),
        format!("        .title(\"{title}\")"),
        ticket_line,
        "        .expected(\"TODO: describe the expected behavior\")".to_string(),
        "        .build()?;".to_string(),
        String::new(),
        "    let assertions: Vec<Box<dyn Assertion>> = vec![".to_string(),
        "        Box::new(McpLogSnapshotCaptured),".to_string(),
        "    ];".to_string(),
        String::new(),
        "    Ok((scenario, assertions))".to_string(),
        "}".to_string(),
        String::new(),
        "#[cfg(test)]".to_string(),
        "mod tests {".to_string(),
        "    use super::*;".to_string(),
        String::new(),
        "    #[test]".to_string(),
        "    fn builds() {".to_string(),
        format!("        {ident}().unwrap();"),
        "    }".to_string(),
        "}".to_string(),
    ];
    let mut out = body.join("\n");
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_scenario_template_includes_slug_title_and_ticket() {
        let out = render_scenario_template("42-something", "A thing", Some("owner/repo#42"));
        assert!(out.contains("pub fn _42_something() -> Result<Builtin>"));
        assert!(out.contains("\"42-something\""));
        assert!(out.contains("\"A thing\""));
        assert!(out.contains(".ticket(\"owner/repo#42\")"));
    }

    #[test]
    fn render_scenario_template_without_ticket_emits_placeholder_comment() {
        let out = render_scenario_template("x", "X", None);
        assert!(out.contains("// .ticket"));
    }
}
