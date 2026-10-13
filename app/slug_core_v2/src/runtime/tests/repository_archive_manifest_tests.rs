use std::io::Cursor;
use std::time::Duration;
use std::time::Instant;

use super::*;
use crate::runtime::dice::source_service::SourceServiceLimits;
use crate::runtime::dice::source_service::SourceServiceScope;

fn scope() -> Arc<SourceServiceScope> {
    let deadline = Instant::now() + Duration::from_secs(5);
    SourceServiceScope::new_for_test(
        slug_workspace_v2::NormalizedAbsolutePath::new("/workspace").unwrap(),
        deadline,
        deadline + Duration::from_secs(1),
        SourceServiceLimits::default(),
    )
    .unwrap()
}

#[test]
fn terminal_manifest_bounds_breadth_paths_and_content_before_publication() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("b"), b"b").unwrap();
    fs::write(root.path().join("a"), b"a").unwrap();
    for (limits, error) in [
        (
            Limits {
                entries: 2,
                ..LIMITS
            },
            "entry budget",
        ),
        (
            Limits {
                paths: PATH_SCRATCH + 1,
                ..LIMITS
            },
            "path storage budget",
        ),
        (Limits { bytes: 1, ..LIMITS }, "byte budget"),
    ] {
        let scope = scope();
        assert!(
            capture(root.path(), &|| true, Some(&scope), limits)
                .unwrap_err()
                .message
                .contains(error)
        );
        assert_eq!(scope.snapshot().live_buffer_bytes, 0);
        assert_eq!(fs::read(root.path().join("a")).unwrap(), b"a");
        assert_eq!(fs::read(root.path().join("b")).unwrap(), b"b");
    }
    let scope = scope();
    let limits = Limits {
        entries: 3,
        bytes: 2,
        ..LIMITS
    };
    let first = capture(root.path(), &|| true, Some(&scope), limits).unwrap();
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);
    assert_eq!(scope.snapshot().peak_buffer_bytes, LIMITS.paths + 64 * 1024);
    assert_eq!(first, capture(root.path(), &|| true, None, limits).unwrap());
    fs::write(root.path().join("b"), b"c").unwrap();
    assert_ne!(first, capture(root.path(), &|| true, None, limits).unwrap());
    scope.cancel();
    assert!(capture(root.path(), &|| true, Some(&scope), limits).is_err());
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);
}

#[test]
fn terminal_manifest_detects_short_and_growing_files_and_bounds_read_requests() {
    struct Reader {
        bytes: Cursor<Vec<u8>>,
        requests: Vec<usize>,
    }
    impl Read for Reader {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            self.requests.push(out.len());
            self.bytes.read(out)
        }
    }
    for (bytes, size, expected) in [
        (b"abc".to_vec(), 4, "truncated"),
        (b"abcde".to_vec(), 4, "grew"),
        (b"abcd".to_vec(), 4, ""),
    ] {
        let mut reader = Reader {
            bytes: Cursor::new(bytes),
            requests: Vec::new(),
        };
        let result = read_file(
            &mut reader,
            size,
            &mut [0; 3],
            &mut Sha256::new(),
            &|| true,
            None,
        );
        if expected.is_empty() {
            assert!(result.is_ok());
        } else {
            assert!(result.unwrap_err().message.contains(expected));
        }
        assert!(reader.requests.iter().all(|n| *n <= 3));
        if expected != "truncated" {
            assert_eq!(reader.requests, [3, 1, 1]);
        }
    }
}
