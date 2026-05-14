//! On-disk cargo project fixtures.
//!
//! [`tiny_warp`] writes a minimal cargo workspace under a caller-provided
//! directory. The workspace has one member, `warp`, which exposes a `warp-oss`
//! binary that prints a short line and exits. It's enough to exercise
//! [`warp_taper_core::stages::build`] and [`warp_taper_core::stages::deploy`]
//! end-to-end without depending on the real warp checkout.

use std::path::{Path, PathBuf};

const WORKSPACE_CARGO_TOML: &str = "\
[workspace]
members = [\"warp\"]
resolver = \"2\"
";

const WARP_CARGO_TOML: &str = "\
[package]
name = \"warp\"
version = \"0.0.0\"
edition = \"2021\"

[[bin]]
name = \"warp-oss\"
path = \"src/main.rs\"
";

const WARP_MAIN_RS: &str = "\
fn main() {
    println!(\"hello from fake warp-oss\");
}
";

/// Handle to a fixture cargo workspace.
#[derive(Debug, Clone)]
pub struct TinyWarp {
    root: PathBuf,
}

impl TinyWarp {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn binary_name(&self) -> &'static str {
        "warp-oss"
    }

    pub fn package_name(&self) -> &'static str {
        "warp"
    }
}

/// Materialize a minimal cargo workspace at `root`. The directory must exist
/// and be empty (or callers must accept files being added underneath).
pub fn tiny_warp(root: impl Into<PathBuf>) -> std::io::Result<TinyWarp> {
    let root = root.into();
    std::fs::create_dir_all(root.join("warp/src"))?;
    std::fs::write(root.join("Cargo.toml"), WORKSPACE_CARGO_TOML)?;
    std::fs::write(root.join("warp/Cargo.toml"), WARP_CARGO_TOML)?;
    std::fs::write(root.join("warp/src/main.rs"), WARP_MAIN_RS)?;
    Ok(TinyWarp { root })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materializes_expected_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let fixture = tiny_warp(tmp.path()).unwrap();
        assert_eq!(fixture.root(), tmp.path());
        assert!(tmp.path().join("Cargo.toml").is_file());
        assert!(tmp.path().join("warp/Cargo.toml").is_file());
        assert!(tmp.path().join("warp/src/main.rs").is_file());
        let main_rs = std::fs::read_to_string(tmp.path().join("warp/src/main.rs")).unwrap();
        assert!(main_rs.contains("warp-oss"));
    }
}
