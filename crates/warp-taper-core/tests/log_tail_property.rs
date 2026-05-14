//! Property tests for [`LogTail`]: for any sequence of appends to a source
//! file, the slice copied after `open` must equal the bytes appended between
//! `open` and `slice_since_start`.

use std::fs::OpenOptions;
use std::io::Write;

use proptest::collection::vec;
use proptest::prelude::*;
use warp_taper_core::LogTail;

fn write_chunks(path: &std::path::Path, chunks: &[Vec<u8>]) {
    let mut f = OpenOptions::new().append(true).open(path).unwrap();
    for c in chunks {
        f.write_all(c).unwrap();
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn slice_equals_bytes_appended_after_open(
        prefix in vec(any::<u8>(), 0..256),
        suffix_chunks in vec(vec(any::<u8>(), 0..64), 0..16),
    ) {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("log");
        let dest = tmp.path().join("slice");

        // Seed the file with `prefix` before opening the tail.
        std::fs::write(&source, &prefix).unwrap();

        let tail = LogTail::open(&source).unwrap();

        // Append all suffix chunks AFTER opening the tail.
        write_chunks(&source, &suffix_chunks);

        let expected: Vec<u8> = suffix_chunks.iter().flatten().copied().collect();
        let written = tail.slice_since_start(&dest).unwrap();

        let captured = std::fs::read(&dest).unwrap();
        prop_assert_eq!(written, expected.len() as u64);
        prop_assert_eq!(captured, expected);
    }

    #[test]
    fn start_offset_equals_initial_file_length(initial in vec(any::<u8>(), 0..1024)) {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("log");
        std::fs::write(&source, &initial).unwrap();

        let tail = LogTail::open(&source).unwrap();
        prop_assert_eq!(tail.start_offset(), initial.len() as u64);
    }
}
