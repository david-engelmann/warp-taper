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

    /// Burn captions onto a pre-recorded video, producing a captioned `.mp4`
    /// and a `.gif` alongside the input. Useful for adding evidence captions
    /// to a recording that wasn't produced by the full pipeline (e.g. a
    /// `screencapture -v` capture of a terminal session).
    Caption {
        /// Input video (typically a `.mov`).
        #[arg(long)]
        input: PathBuf,

        /// JSON file with `[{start, end, text}]` entries (seconds).
        #[arg(long)]
        captions: PathBuf,

        /// Output `.mp4` path. Defaults to `<input-without-extension>-captioned.mp4`.
        #[arg(long)]
        output_mp4: Option<PathBuf>,

        /// Output `.gif` path. Defaults to `<input-without-extension>-captioned.gif`.
        #[arg(long)]
        output_gif: Option<PathBuf>,
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

    /// Pre-specify the screencapture rect as `X,Y,W,H` (in display points).
    /// Skips screencapture's interactive region picker. macOS only; ignored
    /// in --no-screencapture mode.
    ///
    /// **Privacy note:** region capture records pixels at the chosen
    /// screen coordinates regardless of which app owns them. Anything
    /// else visible in that rectangle ends up in the .mov. Prefer
    /// --screencapture-window-id, which records only the target window's
    /// content.
    #[arg(
        long,
        value_name = "X,Y,W,H",
        conflicts_with = "screencapture_window_id"
    )]
    screencapture_region: Option<String>,

    /// CGWindowID of the window to record. Discover it for a running app via:
    ///
    ///   osascript -e 'tell application "System Events" to id of front window of (first process whose name is "Warp")'
    #[arg(long, value_name = "ID")]
    screencapture_window_id: Option<u32>,

    /// After deploy spawns, look up the deployed binary's front window and
    /// scope the recorder to it. Recommended for real Warp builds — pair
    /// with --record-warmup-ms so Warp has time to render its window
    /// before discovery runs.
    #[arg(long)]
    auto_window_id: bool,

    /// Sleep this long between deploy and recording start. Gives a GUI
    /// app a moment to bring up its window. Required in practice for
    /// real Warp (its window takes ~1–3s to appear).
    #[arg(long, value_name = "MS")]
    record_warmup_ms: Option<u64>,
}

fn parse_region(s: &str) -> warp_taper_core::Result<(u32, u32, u32, u32)> {
    let parts: Vec<&str> = s.split(',').map(str::trim).collect();
    if parts.len() != 4 {
        return Err(warp_taper_core::Error::ScenarioInvalid(format!(
            "--screencapture-region must be X,Y,W,H; got {s:?}"
        )));
    }
    let parse = |label: &str, raw: &str| -> warp_taper_core::Result<u32> {
        raw.parse::<u32>().map_err(|e| {
            warp_taper_core::Error::ScenarioInvalid(format!(
                "--screencapture-region {label} component {raw:?}: {e}"
            ))
        })
    };
    Ok((
        parse("x", parts[0])?,
        parse("y", parts[1])?,
        parse("w", parts[2])?,
        parse("h", parts[3])?,
    ))
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
        Command::Caption {
            input,
            captions,
            output_mp4,
            output_gif,
        } => dispatch_int(run_caption(input, captions, output_mp4, output_gif)),
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

fn run_caption(
    input: PathBuf,
    captions_path: PathBuf,
    output_mp4: Option<PathBuf>,
    output_gif: Option<PathBuf>,
) -> warp_taper_core::Result<()> {
    use std::time::Duration;
    use warp_taper_core::{apply_captions, Caption, CaptionConfig};

    #[derive(serde::Deserialize)]
    struct RawCaption {
        start: f64,
        end: f64,
        text: String,
    }

    let captions_text =
        std::fs::read_to_string(&captions_path).map_err(warp_taper_core::Error::Io)?;
    let raw: Vec<RawCaption> = serde_json::from_str(&captions_text).map_err(|e| {
        warp_taper_core::Error::ScenarioInvalid(format!(
            "failed to parse captions JSON {}: {e}",
            captions_path.display()
        ))
    })?;
    let captions: Vec<Caption> = raw
        .into_iter()
        .map(|c| Caption {
            start: Duration::from_secs_f64(c.start),
            end: Duration::from_secs_f64(c.end),
            text: c.text,
        })
        .collect();
    if captions.is_empty() {
        eprintln!("warp-taper: captions list is empty; nothing to do");
        return Ok(());
    }

    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "video".to_string());
    let parent = input.parent().unwrap_or_else(|| Path::new("."));
    let out_mp4 = output_mp4.unwrap_or_else(|| parent.join(format!("{stem}-captioned.mp4")));
    let out_gif = output_gif.unwrap_or_else(|| parent.join(format!("{stem}-captioned.gif")));

    match apply_captions(
        &input,
        &captions,
        &out_mp4,
        &out_gif,
        &CaptionConfig::default(),
    )? {
        Some(artifacts) => {
            println!("captioned mp4: {}", artifacts.mp4_path.display());
            println!("captioned gif: {}", artifacts.gif_path.display());
            Ok(())
        }
        None => Err(warp_taper_core::Error::ScenarioInvalid(
            "captions step produced no output (ffmpeg missing or skipped)".to_string(),
        )),
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
    if let Some(ms) = args.record_warmup_ms {
        pipeline = pipeline.with_record_warmup(Duration::from_millis(ms));
    }
    if args.auto_window_id {
        pipeline = pipeline.with_auto_window_id(true);
    }

    let trigger = match args.duration_ms {
        Some(ms) => RecordTrigger::Duration(Duration::from_millis(ms)),
        None => RecordTrigger::Interactive,
    };

    let region = match args.screencapture_region.as_deref() {
        Some(s) => Some(parse_region(s)?),
        None => None,
    };
    let window_id = args.screencapture_window_id;
    // Configure screencapture's own -V to match our wait window when we
    // know it. Otherwise screencapture runs to its 600s default and our
    // stop() has to signal it (which can lose the .mov).
    let recorder_max_duration_secs = args.duration_ms.map(|ms| ms.div_ceil(1000).max(1));
    let tape = if args.no_screencapture {
        pipeline.run(NoOpRecorder::new(), trigger)?
    } else {
        run_with_real_recorder(
            &pipeline,
            trigger,
            region,
            window_id,
            recorder_max_duration_secs,
        )?
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
    region: Option<(u32, u32, u32, u32)>,
    window_id: Option<u32>,
    max_duration_secs: Option<u64>,
) -> warp_taper_core::Result<warp_taper_core::Tape> {
    let mut recorder = warp_taper_core::MacOsScreencapture::new();
    if let Some(secs) = max_duration_secs {
        recorder = recorder.with_max_duration_seconds(secs);
    }
    if let Some(id) = window_id {
        recorder = recorder.with_window_id(id);
    } else if let Some((x, y, w, h)) = region {
        recorder = recorder.with_region(x, y, w, h);
    }
    pipeline.run(recorder, trigger)
}

#[cfg(not(target_os = "macos"))]
fn run_with_real_recorder(
    pipeline: &Pipeline,
    trigger: RecordTrigger,
    _region: Option<(u32, u32, u32, u32)>,
    _window_id: Option<u32>,
    _max_duration_secs: Option<u64>,
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
