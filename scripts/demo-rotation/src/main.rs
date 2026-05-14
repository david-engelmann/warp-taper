//! Synthetic MCP-log-rotation demo.
//!
//! Mimics the behavior of warpdotdev/warp's `simple_logger` crate on the
//! `david/advanced-log-rotation-poc` branch (PR #10882): writes log lines
//! to an "active" file, rotates when the size threshold is crossed, and
//! emits both a `.rotations.jsonl` event and a `.summaries.jsonl` record.
//! The summary is fetched from LM Studio (`http://localhost:1234/v1/chat/completions`)
//! when reachable, otherwise falls back to a canned line so the demo runs
//! anywhere.
//!
//! Reads three env vars (with sensible defaults):
//!   WARP_OSS_LOG_PATH   — file warp-taper's LogTail watches; written to.
//!   DEMO_MCP_LOG_DIR    — directory the rotated files + sidecars land in.
//!   DEMO_DURATION_MS    — total runtime budget; defaults to 8000.

use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const ROTATION_THRESHOLD_BYTES: usize = 800;
const ROTATION_MAX_COPIES: u32 = 3;
const LM_STUDIO_URL: &str = "http://localhost:1234/v1/chat/completions";

fn main() {
    let log_path = env_or("WARP_OSS_LOG_PATH", "/tmp/warp-taper-demo.log");
    let mcp_dir = PathBuf::from(env_or("DEMO_MCP_LOG_DIR", "/tmp/warp-taper-demo-mcp"));
    let duration_ms: u64 = env_or("DEMO_DURATION_MS", "8000").parse().unwrap_or(8000);
    std::fs::create_dir_all(&mcp_dir).expect("create mcp dir");

    let server_id = "demo-mcp-server-abc123";
    let active = mcp_dir.join(format!("{server_id}.log"));
    let rotations_sidecar = mcp_dir.join(format!("{server_id}.log.rotations.jsonl"));
    let summaries_sidecar = mcp_dir.join(format!("{server_id}.log.summaries.jsonl"));

    println!("==============================================================");
    println!("  warp-taper sample: LLM-driven MCP log rotation");
    println!("  modeled on warpdotdev/warp PR #10882 (simple_logger)");
    println!("==============================================================");
    println!("  threshold:       {ROTATION_THRESHOLD_BYTES} bytes");
    println!("  max rotated:     {ROTATION_MAX_COPIES} copies");
    println!("  warp log path:   {log_path}");
    println!("  active mcp log:  {}", active.display());
    println!("  rotations:       {}", rotations_sidecar.display());
    println!("  summaries:       {}", summaries_sidecar.display());
    println!();

    let lines: &[&str] = &[
        "[INFO] mcp-server started, listening on stdio",
        "[INFO] received initialize request, protocolVersion=2024-11-05",
        "[INFO] tools/list returned 4 tools",
        "[WARN] tool call \"search\": rate-limit backoff 250ms",
        "[INFO] tool call \"search\": 1342 ms, 18 results",
        "[INFO] tool call \"read_resource\": 89 ms",
        "[ERROR] upstream cache miss for sha256:abc1f4..., falling back to source",
        "[INFO] tool call \"search\": 1099 ms, 12 results",
        "[INFO] heartbeat ok",
        "[INFO] tool call \"write_memo\": 12 ms",
        "[INFO] heartbeat ok",
        "[WARN] tool call \"search\": rate-limit backoff 500ms",
    ];

    let started = Instant::now();
    let deadline = started + Duration::from_millis(duration_ms);

    let mut active_file = open_truncate(&active);
    let mut warp_log_file = open_append(&log_path);

    let mut bytes_in_active: usize = 0;
    let mut rotation_count: u32 = 0;
    let mut idx: usize = 0;

    while Instant::now() < deadline {
        let raw = lines[idx % lines.len()];
        let formatted = format!("[2026-05-14T17:00:{:02}Z] {raw}\n", 30 + (idx % 30));
        active_file.write_all(formatted.as_bytes()).ok();
        warp_log_file.write_all(formatted.as_bytes()).ok();
        bytes_in_active += formatted.len();
        println!(
            "wrote entry ({:>3} bytes; active total {:>4} bytes)",
            formatted.len(),
            bytes_in_active
        );

        if bytes_in_active >= ROTATION_THRESHOLD_BYTES && rotation_count < ROTATION_MAX_COPIES {
            rotation_count += 1;
            println!();
            println!(
                "ROTATION TRIGGERED — active log hit {bytes_in_active} bytes (threshold {ROTATION_THRESHOLD_BYTES})"
            );

            drop(active_file);
            let rotated_path = mcp_dir.join(format!("{server_id}.log.{rotation_count}"));
            std::fs::rename(&active, &rotated_path).expect("rotate");
            println!(
                "rotated  {} -> {}",
                active.display(),
                rotated_path.display()
            );

            // Layer A: rotations.jsonl event (always-on)
            let event = serde_json::json!({
                "timestamp": "2026-05-14T17:00:30Z",
                "event": "rotated",
                "active_path": active.display().to_string(),
                "rotated_path": rotated_path.display().to_string(),
                "bytes_rotated": bytes_in_active,
                "rotation_index": rotation_count,
            });
            append_jsonl(&rotations_sidecar, &event);
            println!("wrote rotation event -> {}", rotations_sidecar.display());

            // Layer B: optional LLM summary
            print!("asking LM Studio at {LM_STUDIO_URL} for summary... ");
            let _ = std::io::stdout().flush();
            let rotated_content = std::fs::read_to_string(&rotated_path).unwrap_or_default();
            let (summary, source) = summarize(&rotated_content);
            println!("({source})");
            for line in summary.lines() {
                println!("  > {line}");
            }
            let summary_record = serde_json::json!({
                "rotation_index": rotation_count,
                "source": source,
                "summary": summary,
                "input_bytes": rotated_content.len(),
            });
            append_jsonl(&summaries_sidecar, &summary_record);
            println!("wrote summary record -> {}", summaries_sidecar.display());
            println!();

            active_file = open_truncate(&active);
            bytes_in_active = 0;
        }

        idx += 1;
        std::thread::sleep(Duration::from_millis(180));
    }

    println!("==============================================================");
    println!("  demo finished: {rotation_count} rotation(s), {} byte(s) since last rotate", bytes_in_active);
    println!("==============================================================");
}

fn env_or(key: &str, fallback: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| fallback.to_string())
}

fn open_truncate(p: &std::path::Path) -> std::fs::File {
    std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(p)
        .expect("open log")
}

fn open_append(p: &str) -> std::fs::File {
    if let Some(parent) = std::path::Path::new(p).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(p)
        .expect("open warp log")
}

fn append_jsonl(p: &std::path::Path, v: &serde_json::Value) {
    let line = serde_json::to_string(v).expect("serialize");
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(p)
        .expect("open sidecar");
    writeln!(f, "{line}").ok();
}

/// Returns `(summary_text, source_label)`. `source_label` is "lm-studio" when
/// the local LM Studio HTTP endpoint responded successfully, otherwise
/// "canned-fallback".
fn summarize(rotated_content: &str) -> (String, &'static str) {
    if let Some(s) = summarize_via_lm_studio(rotated_content) {
        return (s, "lm-studio");
    }
    let canned = "12 routine search/read tool calls; 2 rate-limit backoffs; 1 \
                  upstream cache miss recovered via source fallback. No \
                  user-visible errors. Suggested follow-up: monitor cache hit \
                  rate.";
    (canned.to_string(), "canned-fallback")
}

fn summarize_via_lm_studio(content: &str) -> Option<String> {
    let payload = serde_json::json!({
        "model": "local",
        "messages": [
            {"role": "system", "content": "You are summarizing one rotation cycle of an MCP server log. Reply in 1-2 sentences."},
            {"role": "user",   "content": content},
        ],
        "max_tokens": 80,
        "temperature": 0.2,
    });
    let out = std::process::Command::new("curl")
        .args([
            "-sS",
            "-X",
            "POST",
            LM_STUDIO_URL,
            "-H",
            "Content-Type: application/json",
            "-m",
            "8",
            "-d",
            &payload.to_string(),
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let body: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    body["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
