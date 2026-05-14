//! Byte-offset-based slicing of a growing log file.
//!
//! [`LogTail::open`] records the current end-of-file offset; a later call to
//! [`LogTail::slice_since_start`] copies everything appended since that
//! snapshot into a destination file. This is the Rust replacement for the
//! `dd if=warp.log skip=$pre count=$delta` block in the bash pipeline.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct LogTail {
    source: PathBuf,
    start_offset: u64,
}

impl LogTail {
    /// Snapshot the current size of `source`. If the file doesn't exist, it
    /// is created empty so subsequent appends have a target.
    pub fn open(source: impl Into<PathBuf>) -> Result<Self> {
        let source = source.into();
        let start_offset = match std::fs::metadata(&source) {
            Ok(m) => m.len(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&source)
                    .map_err(Error::Io)?;
                0
            }
            Err(e) => return Err(Error::Io(e)),
        };
        Ok(Self {
            source,
            start_offset,
        })
    }

    pub fn start_offset(&self) -> u64 {
        self.start_offset
    }

    pub fn source(&self) -> &Path {
        &self.source
    }

    /// Copy bytes appended to `source` since [`open`](Self::open) into `dest`.
    /// Returns the number of bytes written. If the source shrank below
    /// `start_offset` (rotation, truncation) the function writes an empty
    /// `dest` and returns `0`.
    pub fn slice_since_start(&self, dest: impl AsRef<Path>) -> Result<u64> {
        let dest = dest.as_ref();
        let current_len = std::fs::metadata(&self.source).map_err(Error::Io)?.len();

        let mut dst = File::create(dest).map_err(Error::Io)?;
        if current_len <= self.start_offset {
            return Ok(0);
        }

        let mut src = File::open(&self.source).map_err(Error::Io)?;
        src.seek(SeekFrom::Start(self.start_offset))
            .map_err(Error::Io)?;

        let to_copy = current_len - self.start_offset;
        let mut remaining = to_copy;
        let mut buf = [0u8; 8192];
        let mut total: u64 = 0;
        while remaining > 0 {
            let chunk = std::cmp::min(remaining as usize, buf.len());
            let n = src.read(&mut buf[..chunk]).map_err(Error::Io)?;
            if n == 0 {
                break;
            }
            dst.write_all(&buf[..n]).map_err(Error::Io)?;
            total += n as u64;
            remaining = remaining.saturating_sub(n as u64);
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::OpenOptions;
    use std::io::Write;

    fn append(path: &Path, bytes: &[u8]) {
        let mut f = OpenOptions::new().append(true).open(path).unwrap();
        f.write_all(bytes).unwrap();
    }

    #[test]
    fn open_creates_missing_file_with_zero_offset() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("does-not-exist.log");
        assert!(!target.exists());

        let tail = LogTail::open(&target).unwrap();
        assert!(target.exists());
        assert_eq!(tail.start_offset(), 0);
    }

    #[test]
    fn open_existing_file_records_current_length() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("seeded.log");
        std::fs::write(&target, b"hello").unwrap();

        let tail = LogTail::open(&target).unwrap();
        assert_eq!(tail.start_offset(), 5);
    }

    #[test]
    fn slice_with_no_appends_writes_empty_dest() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("src.log");
        let dest = tmp.path().join("dest.log");
        std::fs::write(&source, b"abc").unwrap();

        let tail = LogTail::open(&source).unwrap();
        let written = tail.slice_since_start(&dest).unwrap();
        assert_eq!(written, 0);
        assert!(dest.exists());
        assert_eq!(std::fs::read(&dest).unwrap(), b"");
    }

    #[test]
    fn slice_captures_only_appended_bytes() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("src.log");
        let dest = tmp.path().join("dest.log");
        std::fs::write(&source, b"BEFORE").unwrap();

        let tail = LogTail::open(&source).unwrap();
        append(&source, b"AFTER");

        let written = tail.slice_since_start(&dest).unwrap();
        assert_eq!(written, 5);
        assert_eq!(std::fs::read(&dest).unwrap(), b"AFTER");
    }

    #[test]
    fn slice_handles_truncation_as_zero_bytes() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("src.log");
        let dest = tmp.path().join("dest.log");
        std::fs::write(&source, b"original-content").unwrap();

        let tail = LogTail::open(&source).unwrap();
        // Simulate rotation: file is now smaller than start_offset.
        std::fs::write(&source, b"").unwrap();

        let written = tail.slice_since_start(&dest).unwrap();
        assert_eq!(written, 0);
        assert_eq!(std::fs::read(&dest).unwrap(), b"");
    }

    #[test]
    fn slice_handles_large_append_across_buffer_boundary() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("src.log");
        let dest = tmp.path().join("dest.log");
        File::create(&source).unwrap();

        let tail = LogTail::open(&source).unwrap();
        // 20 KiB — exercises multiple 8 KiB buffer reads.
        let payload = vec![b'x'; 20 * 1024];
        append(&source, &payload);

        let written = tail.slice_since_start(&dest).unwrap();
        assert_eq!(written, payload.len() as u64);
        assert_eq!(std::fs::read(&dest).unwrap(), payload);
    }

    #[test]
    fn open_missing_parent_dir_errors() {
        let tail = LogTail::open("/no/such/parent/dir/file.log");
        assert!(matches!(tail, Err(Error::Io(_))));
    }

    #[test]
    fn source_returns_provided_path() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("x.log");
        let tail = LogTail::open(&target).unwrap();
        assert_eq!(tail.source(), target.as_path());
    }
}
