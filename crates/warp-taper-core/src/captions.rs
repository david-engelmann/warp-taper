//! Burn timed captions onto a recorded `.mov` via ffmpeg `drawtext`.
//!
//! Used by the post-record pipeline stage to produce a captioned `.mp4` and
//! `.gif` alongside the raw `master.mov`. The raw file is preserved
//! unchanged so reviewers can always replay the source recording.
//!
//! Skip behavior:
//! - Empty caption list → the function returns `Ok(false)` without invoking
//!   ffmpeg. Caller treats this as "no captioned artifacts produced".
//! - ffmpeg not installed → returns `Ok(false)` after logging a warning.
//!   The pipeline keeps moving so a missing toolchain doesn't block tape
//!   bundling.
//! - ffmpeg invocation fails → returns the error so the caller can fail the
//!   tape or surface it in the bundle stage log.
//!
//! Caption styling: 70px translucent black bar across the top of the video,
//! centered white text at fontsize=30 using the system Arial. This matches
//! the inline shell script used to produce the first captioned tape for
//! warpdotdev/warp#10874.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Error, Result};
use crate::scenario::Caption;

/// Default macOS path to a TTF font that ffmpeg can load via libfreetype.
/// Captioned output renders white text at this font. Overridable via
/// [`CaptionConfig::font_path`] for hosts where this path is missing.
pub const DEFAULT_FONT_PATH: &str = "/System/Library/Fonts/Supplemental/Arial.ttf";

/// Inputs to [`apply_captions`]. Most callers should construct via
/// [`CaptionConfig::default`] and override only what they need.
#[derive(Debug, Clone)]
pub struct CaptionConfig {
    /// TTF font ffmpeg's `drawtext` filter loads. Must exist on the host.
    pub font_path: PathBuf,
    /// Font size in pixels.
    pub fontsize: u32,
    /// Hex/RGB color string ffmpeg accepts, e.g. `"white"` or `"#ffffff"`.
    pub fontcolor: String,
    /// Height of the top bar in pixels (also positions y for the caption).
    pub bar_height: u32,
    /// Alpha of the translucent bar, 0.0..=1.0.
    pub bar_alpha: f32,
    /// y-offset of caption text from the top.
    pub y_offset: u32,
}

impl Default for CaptionConfig {
    fn default() -> Self {
        Self {
            font_path: PathBuf::from(DEFAULT_FONT_PATH),
            fontsize: 30,
            fontcolor: "white".to_string(),
            bar_height: 70,
            bar_alpha: 0.85,
            y_offset: 20,
        }
    }
}

/// Outputs from [`apply_captions`]. Both paths are present iff captioning ran.
#[derive(Debug, Clone)]
pub struct CaptionedArtifacts {
    pub mp4_path: PathBuf,
    pub gif_path: PathBuf,
}

/// Apply `captions` to `input_mov` and produce `output_mp4` + `output_gif`.
///
/// Returns `Ok(Some(_))` if captions were rendered. Returns `Ok(None)` if:
/// - `captions` is empty (caller probably wants no captioning), or
/// - `ffmpeg` isn't on `PATH`.
///
/// Returns an error only when ffmpeg is available but the actual invocation
/// fails (e.g. font missing, input unreadable, encoder error).
pub fn apply_captions(
    input_mov: &Path,
    captions: &[Caption],
    output_mp4: &Path,
    output_gif: &Path,
    cfg: &CaptionConfig,
) -> Result<Option<CaptionedArtifacts>> {
    if captions.is_empty() {
        return Ok(None);
    }
    if !ffmpeg_available() {
        eprintln!(
            "warp-taper: ffmpeg not found on PATH; skipping caption burn for {}",
            input_mov.display()
        );
        return Ok(None);
    }
    if !input_mov.is_file() {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("input recording missing: {}", input_mov.display()),
        )));
    }
    if !cfg.font_path.is_file() {
        eprintln!(
            "warp-taper: caption font {} not found; skipping caption burn",
            cfg.font_path.display()
        );
        return Ok(None);
    }

    let filter = build_drawtext_filter(captions, cfg);

    // 1. Burn captions onto an mp4. libx264 + yuv420p produces a portable
    //    output that GitHub and most browsers can preview inline.
    let mp4_status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(input_mov)
        .arg("-vf")
        .arg(&filter)
        .arg("-c:v")
        .arg("libx264")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-crf")
        .arg("20")
        .arg(output_mp4)
        .status()
        .map_err(Error::Io)?;
    if !mp4_status.success() {
        return Err(Error::Io(std::io::Error::other(format!(
            "ffmpeg failed to produce captioned mp4 (exit={:?})",
            mp4_status.code()
        ))));
    }

    // 2. Build a gif from the captioned mp4 using the palettegen / paletteuse
    //    pattern (smaller, better-quality output than naive `-i in.mp4 out.gif`).
    let gif_filter =
        "fps=10,scale=1000:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse";
    let gif_status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(output_mp4)
        .arg("-vf")
        .arg(gif_filter)
        .arg(output_gif)
        .status()
        .map_err(Error::Io)?;
    if !gif_status.success() {
        return Err(Error::Io(std::io::Error::other(format!(
            "ffmpeg failed to produce captioned gif (exit={:?})",
            gif_status.code()
        ))));
    }

    Ok(Some(CaptionedArtifacts {
        mp4_path: output_mp4.to_path_buf(),
        gif_path: output_gif.to_path_buf(),
    }))
}

fn ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Builds the chained `drawtext` filter string applied to the input video.
///
/// Shape: `drawbox=...,drawtext=...,drawtext=...,...` — one `drawbox` for
/// the translucent header bar plus one `drawtext` per caption, each gated
/// by an `enable='between(t,start,end)'` clause so only the active caption
/// is visible at any moment.
///
/// Escaping: ffmpeg's filter parser uses `:` to separate key/value pairs
/// and `,` to chain filters, and treats `\` as an escape inside parameter
/// values. We escape each of those plus `'` in caption text so the filter
/// string is unambiguous.
pub fn build_drawtext_filter(captions: &[Caption], cfg: &CaptionConfig) -> String {
    let mut out = format!(
        "drawbox=y=0:h={bh}:c=black@{alpha:.2}:t=fill",
        bh = cfg.bar_height,
        alpha = cfg.bar_alpha,
    );
    for caption in captions {
        let escaped = escape_drawtext_text(&caption.text);
        let start = caption.start.as_secs_f64();
        let end = caption.end.as_secs_f64();
        out.push_str(&format!(
            ",drawtext=fontfile={font}:text='{text}':fontcolor={color}:fontsize={size}:x=(w-text_w)/2:y={y}:enable='between(t\\,{start}\\,{end})'",
            font = cfg.font_path.display(),
            text = escaped,
            color = cfg.fontcolor,
            size = cfg.fontsize,
            y = cfg.y_offset,
        ));
    }
    out
}

/// Escapes a caption string for embedding inside a `drawtext` `text='...'`
/// parameter. Order matters: `\` must be escaped first so we don't
/// double-escape the escapes we just emit.
fn escape_drawtext_text(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace(':', "\\:")
        .replace(',', "\\,")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn cap(start_s: f64, end_s: f64, text: &str) -> Caption {
        Caption::new(
            Duration::from_secs_f64(start_s),
            Duration::from_secs_f64(end_s),
            text,
        )
    }

    #[test]
    fn empty_captions_skips_ffmpeg() {
        // Passing an empty caption list must return `Ok(None)` without
        // touching ffmpeg or filesystem. We point at a path that doesn't
        // exist to prove neither side-effect runs.
        let result = apply_captions(
            Path::new("/nonexistent/in.mov"),
            &[],
            Path::new("/nonexistent/out.mp4"),
            Path::new("/nonexistent/out.gif"),
            &CaptionConfig::default(),
        )
        .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn missing_input_video_returns_io_error() {
        // The host has ffmpeg (CI runs `ffmpeg-available` paths since the
        // CI step itself runs `cargo test`). We assert: with a non-empty
        // caption list and a missing input file, the call returns an I/O
        // NotFound error instead of silently no-op'ing.
        if !ffmpeg_available() {
            // On hosts without ffmpeg the call short-circuits to Ok(None)
            // before reaching the input-check branch; skip this assertion.
            return;
        }
        let captions = vec![cap(0.0, 1.0, "hi")];
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("does-not-exist.mov");
        let result = apply_captions(
            &missing,
            &captions,
            &tmp.path().join("out.mp4"),
            &tmp.path().join("out.gif"),
            &CaptionConfig::default(),
        );
        let err = result.expect_err("missing input must produce an error");
        let crate::error::Error::Io(io_err) = err else {
            panic!("expected Error::Io, got: {err:?}");
        };
        assert_eq!(io_err.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn missing_font_returns_ok_none() {
        // If the configured font file isn't present, the call must skip
        // rather than fail — captioning is a non-fatal post-process, and
        // surfacing the skip as `Ok(None)` lets the pipeline keep going.
        if !ffmpeg_available() {
            return;
        }
        let captions = vec![cap(0.0, 1.0, "hi")];
        let tmp = tempfile::tempdir().unwrap();
        // We need a real input file to get past the input-check, but the
        // font check fires before ffmpeg runs so the file contents don't
        // matter.
        let input = tmp.path().join("dummy.mov");
        std::fs::write(&input, b"not really a video").unwrap();
        let cfg = CaptionConfig {
            font_path: tmp.path().join("nonexistent.ttf"),
            ..CaptionConfig::default()
        };
        let result = apply_captions(
            &input,
            &captions,
            &tmp.path().join("out.mp4"),
            &tmp.path().join("out.gif"),
            &cfg,
        )
        .unwrap();
        assert!(result.is_none(), "missing font must produce Ok(None)");
    }

    #[test]
    fn caption_config_default_values_are_stable() {
        // Pin the default values so a future refactor that bumps them
        // surfaces in code review rather than silently changing the
        // appearance of every captioned tape.
        let cfg = CaptionConfig::default();
        assert_eq!(cfg.font_path.to_str(), Some(DEFAULT_FONT_PATH));
        assert_eq!(cfg.fontsize, 30);
        assert_eq!(cfg.fontcolor, "white");
        assert_eq!(cfg.bar_height, 70);
        assert!((cfg.bar_alpha - 0.85).abs() < f32::EPSILON);
        assert_eq!(cfg.y_offset, 20);
    }

    #[test]
    fn empty_caption_text_after_escape_is_preserved() {
        // Edge case the escape function must handle: an all-whitespace
        // caption (we don't reject these at the captions module level —
        // the scenario layer does — but the renderer should produce
        // a deterministic output).
        let captions = vec![cap(0.0, 1.0, "   ")];
        let filter = build_drawtext_filter(&captions, &CaptionConfig::default());
        assert!(filter.contains("text='   '"));
    }

    #[test]
    fn filter_starts_with_translucent_bar() {
        let captions = vec![cap(0.0, 3.0, "hello")];
        let filter = build_drawtext_filter(&captions, &CaptionConfig::default());
        assert!(
            filter.starts_with("drawbox=y=0:h=70:c=black@0.85:t=fill"),
            "got: {filter}"
        );
    }

    #[test]
    fn each_caption_gates_on_time_window() {
        let captions = vec![cap(1.5, 4.0, "one"), cap(4.0, 7.5, "two")];
        let filter = build_drawtext_filter(&captions, &CaptionConfig::default());
        // First caption window.
        assert!(
            filter.contains("enable='between(t\\,1.5\\,4)'"),
            "missing first caption time window: {filter}"
        );
        // Second caption window.
        assert!(
            filter.contains("enable='between(t\\,4\\,7.5)'"),
            "missing second caption time window: {filter}"
        );
    }

    #[test]
    fn caption_text_with_commas_is_escaped() {
        // A bare comma in the caption text would be interpreted as a
        // filter-chain separator by ffmpeg. Must be backslash-escaped.
        let captions = vec![cap(0.0, 1.0, "10 MiB per file, 5 rotations")];
        let filter = build_drawtext_filter(&captions, &CaptionConfig::default());
        assert!(
            filter.contains(r"10 MiB per file\, 5 rotations"),
            "comma not escaped: {filter}"
        );
    }

    #[test]
    fn caption_text_with_colons_is_escaped() {
        let captions = vec![cap(0.0, 1.0, "policy: 60 MiB cap")];
        let filter = build_drawtext_filter(&captions, &CaptionConfig::default());
        assert!(
            filter.contains(r"policy\: 60 MiB cap"),
            "colon not escaped: {filter}"
        );
    }

    #[test]
    fn caption_text_with_single_quote_is_escaped() {
        // An unescaped `'` would close the `text='...'` parameter early.
        let captions = vec![cap(0.0, 1.0, "don't break")];
        let filter = build_drawtext_filter(&captions, &CaptionConfig::default());
        assert!(
            filter.contains(r"don\'t break"),
            "single quote not escaped: {filter}"
        );
    }

    #[test]
    fn caption_text_with_backslash_is_escaped_first() {
        // `\` must be escaped BEFORE other characters so we don't
        // double-escape the escapes we just emit.
        let captions = vec![cap(0.0, 1.0, r"path\to\file")];
        let filter = build_drawtext_filter(&captions, &CaptionConfig::default());
        assert!(
            filter.contains(r"path\\to\\file"),
            "backslash not escaped: {filter}"
        );
    }
}
