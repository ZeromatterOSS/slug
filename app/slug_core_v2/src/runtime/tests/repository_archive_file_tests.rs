use std::cell::Cell;
use std::fs;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::os::unix::net::UnixListener;
use std::path::Path;

use sha2::Digest;
use sha2::Sha256;

use super::*;

fn uri(path: &Path) -> String {
    url::Url::from_file_path(path).unwrap().to_string()
}

fn capture(
    path: &Path,
    bytes: &[u8],
    limit: u64,
    active: &dyn Fn() -> bool,
) -> Result<tempfile::NamedTempFile, String> {
    capture_file(
        &uri(path),
        tempfile::NamedTempFile::new().unwrap(),
        Sha256::digest(bytes).into(),
        limit,
        "archive",
        active,
    )
}

fn bytes(mut capture: tempfile::NamedTempFile) -> Vec<u8> {
    capture.rewind().unwrap();
    let mut result = Vec::new();
    capture.read_to_end(&mut result).unwrap();
    result
}

#[test]
fn file_grammar_rejects_normalization_and_decoding_traps() {
    for value in [
        "file:///",
        "file:relative",
        "file:/tmp/x",
        "file://localhost/tmp/x",
        "file://host/tmp/x",
        "file://user@host/tmp/x",
        "file://host:4/tmp/x",
        "file:////tmp/x",
        "file:///tmp/x/",
        "file:///tmp/./x",
        "file:///tmp/../x",
        "file:///tmp/%2e/x",
        "file:///tmp/%2E%2e/x",
        "file:///tmp/.%2e/x",
        "file:///tmp/%2E./x",
        "file:///tmp/a%2Fb",
        "file:///tmp/a%5cb",
        "file:///tmp/%",
        "file:///tmp/%1",
        "file:///tmp/%xx",
        "file:///tmp/%ff",
        "file:///tmp/%00",
        "file:///tmp/%0a",
        "file:///tmp/%7f",
        "file:///tmp/a\\b",
        "file:///tmp/a b",
        "file:///tmp/a\tb",
        "file:///tmp/a\nb",
        " file:///tmp/a",
        "file:///tmp/a?query",
        "file:///tmp/a#fragment",
        "file:///tmp/a?",
        "file:///tmp/a#",
        "file:///C:/x",
        "file:///C|/x",
        "file:///c%3a/x",
        "file:///c%7C/x",
        "https://host/x",
        "http://host/x",
    ] {
        assert!(file_path(value).is_err(), "admitted {value:?}");
    }
    assert!(file_path(&format!("file:///{}", "a".repeat(4095))).is_err());
    assert!(file_path(&format!("file:///{}", "%61".repeat(5500))).is_err());
    for (value, path) in [
        ("FILE:///tmp/λ", "/tmp/λ"),
        ("file:///tmp/a%20b", "/tmp/a b"),
        ("file:///tmp/%25%23%3F", "/tmp/%#?"),
        ("file:///tmp/%252e", "/tmp/%2e"),
        ("file:///tmp/%C3%A9", "/tmp/é"),
        ("file:///mirror//tmp/a", "/mirror//tmp/a"),
    ] {
        assert_eq!(file_path(value).unwrap(), Path::new(path));
    }
    assert!(file_path(&format!("file:///{}", "a".repeat(4094))).is_ok());
}

#[test]
fn empty_unicode_escaped_and_symlink_bytes_are_content_addressed() {
    let root = tempfile::tempdir().unwrap();
    for (name, body) in [("empty", b"".as_slice()), ("λ %#?", b"body".as_slice())] {
        let path = root.path().join(name);
        fs::write(&path, body).unwrap();
        let link = root.path().join(format!("link-{name}"));
        symlink(&path, &link).unwrap();
        assert_eq!(bytes(capture(&link, body, 4, &|| true).unwrap()), body);
        let captured = capture(&path, body, 4, &|| true).unwrap();
        fs::write(&path, b"changed source").unwrap();
        assert_eq!(bytes(captured), body);
    }
    let nested = root.path().join("mirror");
    fs::create_dir(&nested).unwrap();
    fs::write(nested.join("payload"), b"mirror").unwrap();
    let doubled = root.path().join("mirror//payload");
    assert_eq!(
        bytes(capture(&doubled, b"mirror", 6, &|| true).unwrap()),
        b"mirror"
    );
}

#[test]
fn nonregular_sources_are_rejected_before_data_open() {
    let root = tempfile::tempdir().unwrap();
    let fifo = root.path().join("fifo");
    nix::unistd::mkfifo(&fifo, nix::sys::stat::Mode::S_IRUSR).unwrap();
    let socket = root.path().join("socket");
    let _listener = UnixListener::bind(&socket).unwrap();
    for (index, path) in [
        root.path(),
        fifo.as_path(),
        socket.as_path(),
        Path::new("/dev/null"),
    ]
    .into_iter()
    .enumerate()
    {
        let link = root.path().join(format!("link{index}"));
        symlink(path, &link).unwrap();
        for source in [path, link.as_path()] {
            let calls = Cell::new(0);
            let error = capture(source, b"", 1, &|| {
                calls.set(calls.get() + 1);
                true
            })
            .unwrap_err();
            assert!(error.contains("not a regular file"), "{error}");
            assert_eq!(calls.get(), 1, "must reject before post-pin/data-open step");
        }
    }
}

#[test]
fn pinned_file_survives_unlink_and_replacement_with_a_fifo() {
    let root = tempfile::tempdir().unwrap();
    for replace in [false, true] {
        let path = root.path().join("source");
        fs::write(&path, b"pinned").unwrap();
        let calls = Cell::new(0);
        let captured = capture(&path, b"pinned", 6, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == 2 {
                fs::remove_file(&path).unwrap();
                if replace {
                    nix::unistd::mkfifo(&path, nix::sys::stat::Mode::S_IRUSR).unwrap();
                }
            }
            true
        })
        .unwrap();
        assert_eq!(bytes(captured), b"pinned");
    }
}

#[test]
fn unread_mutation_truncation_and_growth_cannot_bypass_sri_or_cap() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("source");
    let original = vec![b'a'; 65540];
    for change in ["mutate", "truncate", "grow"] {
        fs::write(&path, &original).unwrap();
        let calls = Cell::new(0);
        let output = tempfile::NamedTempFile::new().unwrap();
        let output_path = output.path().to_owned();
        let result = capture_file(
            &uri(&path),
            output,
            Sha256::digest(&original).into(),
            original.len() as u64,
            "archive",
            &|| {
                calls.set(calls.get() + 1);
                if calls.get() == 5 {
                    let mut writer = fs::OpenOptions::new().write(true).open(&path).unwrap();
                    match change {
                        "mutate" => {
                            writer.seek(SeekFrom::Start(65536)).unwrap();
                            writer.write_all(b"bbbb").unwrap();
                        }
                        "truncate" => writer.set_len(65536).unwrap(),
                        "grow" => writer.set_len(65541).unwrap(),
                        _ => unreachable!(),
                    }
                }
                true
            },
        );
        let error = result.unwrap_err();
        assert!(
            error.contains(if change == "grow" {
                "capture limit"
            } else {
                "SRI mismatch"
            }),
            "{error}"
        );
        assert!(!output_path.exists());
    }
}

fn handles_for(path: &Path) -> usize {
    fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| fs::read_link(entry.path()).is_ok_and(|target| target == path))
        .count()
}

#[test]
fn cancellation_at_every_boundary_releases_capture_and_source_descriptors() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("source");
    fs::write(&path, b"bytes").unwrap();
    let steps = Cell::new(0);
    drop(
        capture(&path, b"bytes", 5, &|| {
            steps.set(steps.get() + 1);
            true
        })
        .unwrap(),
    );
    assert_eq!(steps.get(), 7);
    for stop in 1..=steps.get() {
        let calls = Cell::new(0);
        let output = tempfile::NamedTempFile::new().unwrap();
        let output_path = output.path().to_owned();
        let result = capture_file(
            &uri(&path),
            output,
            Sha256::digest(b"bytes").into(),
            5,
            "MODULE",
            &|| {
                calls.set(calls.get() + 1);
                if calls.get() == 2 {
                    assert_eq!(handles_for(&path), 1);
                }
                if calls.get() == 3 {
                    assert_eq!(handles_for(&path), 2);
                }
                calls.get() != stop
            },
        );
        assert!(result.unwrap_err().contains("no longer active"));
        assert!(!output_path.exists());
        assert_eq!(handles_for(&path), 0);
    }
}

#[test]
fn pinned_reopen_failure_has_no_pathname_fallback() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("source");
    fs::write(&path, b"pinned").unwrap();
    let calls = Cell::new(0);
    let result = capture(&path, b"pinned", 6, &|| {
        calls.set(calls.get() + 1);
        if calls.get() == 2 {
            fs::set_permissions(&path, fs::Permissions::from_mode(0)).unwrap();
        }
        true
    });
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(result.unwrap_err().contains("opening pinned"));
    assert_eq!(handles_for(&path), 0);
    // ENOENT (missing procfs) and EACCES above propagate through the same open
    // error edge. Do not mutate mounts or bypass a held descriptor for this test.
    let owner = include_str!("../repository_archive_file.rs");
    assert_eq!(owner.matches(".open(").count(), 2);
    assert!(owner.contains("/proc/self/fd/{}"));
    for forbidden in ["unsafe", "spawn(", "spawn_blocking", "mmap", "read_to_end"] {
        assert!(!owner.contains(forbidden), "{forbidden}");
    }
}
