//! Immutable physical acquisition hints. Original URL/SRI owns repository identity.
use std::path::Path;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::dice::source_service::SourceBufferSite;
use super::dice::source_service::SourceServiceScope;

/// Ordered readonly archive directories for one command, never a semantic cache.
#[derive(Debug, Clone, Default, Eq, PartialEq, Dupe, Allocative)]
pub struct ArchiveDistdirPolicy {
    paths: Option<Arc<[NormalizedAbsolutePath]>>,
}

impl ArchiveDistdirPolicy {
    /// Resolve relative entries once against the requesting workspace, without IO.
    pub fn from_request(workspace: &Path, raw: &[String]) -> Result<Self, String> {
        if raw.is_empty() {
            return Ok(Self::default());
        }
        if !cfg!(target_os = "linux") {
            return Err("--distdir archive capture is supported only on Linux".into());
        }
        if !workspace.is_absolute() {
            return Err("distdir workspace must be absolute".into());
        }
        let paths = raw
            .iter()
            .map(|value| {
                if value.is_empty() || value.contains('\0') {
                    return Err("--distdir requires a nonempty path without NUL".into());
                }
                let input = Path::new(value);
                let path = if input.is_absolute() {
                    input.to_owned()
                } else {
                    workspace.join(input)
                };
                NormalizedAbsolutePath::new(path)
                    .map_err(|error| format!("invalid --distdir: {error}"))
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self {
            paths: Some(paths.into()),
        })
    }

    pub fn paths(&self) -> &[NormalizedAbsolutePath] {
        self.paths.as_deref().unwrap_or(&[])
    }

    pub(super) fn try_capture(
        &self,
        original: &str,
        integrity: [u8; 32],
        limit: u64,
        subject: &str,
        active: &dyn Fn() -> bool,
        scope: Option<&Arc<SourceServiceScope>>,
    ) -> Result<Option<tempfile::NamedTempFile>, String> {
        if self.paths().is_empty() {
            return Ok(None);
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (original, integrity, limit, subject, active, scope);
            Err("--distdir archive capture is supported only on Linux".into())
        }
        #[cfg(target_os = "linux")]
        {
            use super::repository_archive_file::FileCaptureError;
            use super::repository_archive_file::capture_path_typed;
            if !original
                .get(..8)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"))
            {
                return Ok(None);
            }
            let live = || {
                super::repository_io::source_service::check(scope)?;
                if active() {
                    Ok(())
                } else {
                    Err("repository session is no longer active".to_owned())
                }
            };
            live()?;
            // Address storage/accounting matches scoped archive capture. No new
            // diagnostic scope is installed by an ordinary production command.
            if let Some(scope) = scope {
                if original.len() > 8192 {
                    return Err(scope
                        .poison_buffer_limit_at(
                            SourceBufferSite::ArchiveUrlLength,
                            original.len(),
                            8192,
                            super::dice::source_service::SourceBufferUnit::Bytes,
                        )
                        .to_string());
                }
            }
            let _address_credit = scope
                .map(|scope| scope.reserve_buffer_at(64 * 1024, SourceBufferSite::ArchiveAddress))
                .transpose()
                .map_err(|error| error.to_string())?;
            let Some(name) = basename(original)? else {
                return Ok(None);
            };
            for dir in self.paths() {
                live()?;
                if let Some(scope) = scope {
                    super::repository_archive_http::charge_distdir_capture(scope)?;
                }
                // A miss drops this owned temp, including wrong-hash bytes and offset.
                let capture = tempfile::NamedTempFile::new()
                    .map_err(|error| format!("creating source capture failed: {error}"))?;
                match capture_path_typed(
                    &dir.as_path().join(&name),
                    capture,
                    integrity,
                    limit,
                    subject,
                    active,
                    scope,
                ) {
                    Ok(capture) => {
                        live()?;
                        return Ok(Some(capture));
                    }
                    Err(FileCaptureError::Miss(_)) => {
                        live()?;
                    }
                    Err(FileCaptureError::Fatal(message)) => return Err(message),
                }
            }
            Ok(None)
        }
    }
}

fn basename(original: &str) -> Result<Option<&str>, String> {
    let url = url::Url::parse(original).map_err(|_| "invalid source URL".to_owned())?;
    if url.scheme() != "https" {
        return Ok(None);
    }
    if url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() {
        return Err("BCR archive URL must be unauthenticated HTTPS".into());
    }
    // Reject raw normalization/encoding traps instead of interpreting them as
    // a different basename. This admitted subset has no output-name aliases.
    let raw_path = original
        .split_once("://")
        .and_then(|(_, rest)| rest.split_once('/'))
        .map(|(_, path)| path.split(['?', '#']).next().unwrap_or(""))
        .unwrap_or("");
    if raw_path
        .split('/')
        .any(|part| matches!(part, "." | "..") || part.contains('%'))
    {
        return Err("archive URL is outside the admitted distdir basename grammar".into());
    }
    // The admitted raw ASCII path has no percent/dot normalization; borrow its
    // final component instead of retaining another String from the parsed URL.
    let name = raw_path.rsplit('/').next().unwrap_or("");
    if name.is_empty()
        || matches!(name, "." | "..")
        || name.len() > 255
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-+".contains(&b))
    {
        return Err("archive URL is outside the admitted distdir basename grammar".into());
    }
    Ok(Some(name))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use std::cell::Cell;
    use std::fs;
    use std::io::Read;
    use std::io::Seek;
    use std::time::Duration;
    use std::time::Instant;

    use sha2::Digest;
    use sha2::Sha256;

    use super::super::dice::source_service::SourceServiceError;
    use super::super::dice::source_service::SourceServiceLimits;
    use super::*;

    fn scope(limits: SourceServiceLimits) -> Arc<SourceServiceScope> {
        let work = Instant::now() + Duration::from_secs(2);
        SourceServiceScope::new_for_test(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            work,
            work + Duration::from_secs(1),
            limits,
        )
        .unwrap()
    }

    #[test]
    fn verified_distdir_hit_and_ordered_miss() {
        let root = tempfile::tempdir().unwrap();
        let policy = ArchiveDistdirPolicy::from_request(
            root.path(),
            &["missing".into(), "wrong".into(), "good".into()],
        )
        .unwrap();
        fs::create_dir(root.path().join("wrong")).unwrap();
        fs::create_dir(root.path().join("good")).unwrap();
        fs::write(root.path().join("wrong/archive.tar.gz"), b"incorrect").unwrap();
        fs::write(root.path().join("good/archive.tar.gz"), b"verified").unwrap();
        assert_eq!(policy.paths()[0].as_path(), root.path().join("missing"));
        assert_eq!(policy.paths()[2].as_path(), root.path().join("good"));
        let copy = policy.clone();
        assert!(Arc::ptr_eq(
            policy.paths.as_ref().unwrap(),
            copy.paths.as_ref().unwrap()
        ));
        let own = scope(SourceServiceLimits::default());
        let mut captured = policy
            .try_capture(
                "https://original.invalid/archive.tar.gz",
                Sha256::digest(b"verified").into(),
                20,
                "archive",
                &|| true,
                Some(&own),
            )
            .unwrap()
            .unwrap();
        captured.rewind().unwrap();
        let mut bytes = Vec::new();
        captured.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"verified");
        let path = captured.path().to_owned();
        drop(captured);
        assert!(!path.exists());
        assert_eq!(own.snapshot().source_attempts, 3);
        assert_eq!(own.snapshot().transferred_bytes, 17);
        assert_eq!(own.snapshot().written_bytes, 17);
        assert_eq!(own.snapshot().live_buffer_bytes, 0);
        let empty = ArchiveDistdirPolicy::default();
        assert!(empty.paths.is_none());
        assert!(
            empty
                .try_capture(
                    "not a URL",
                    [0; 32],
                    0,
                    "archive",
                    &|| panic!("default calls no hook"),
                    None
                )
                .unwrap()
                .is_none()
        );
        let normalized = ArchiveDistdirPolicy::from_request(
            root.path(),
            &[
                "./good/../wrong".into(),
                root.path().join("good").to_str().unwrap().into(),
            ],
        )
        .unwrap();
        assert_eq!(normalized.paths()[0].as_path(), root.path().join("wrong"));
        for raw in [vec![String::new()], vec!["a\0b".into()]] {
            assert!(ArchiveDistdirPolicy::from_request(root.path(), &raw).is_err());
        }
        for url in [
            "https://original.invalid/",
            "https://original.invalid/a/../archive.tar.gz",
            "https://original.invalid/%2e%2e/archive.tar.gz",
            "https://original.invalid/a%2Fb",
            "https://user@original.invalid/archive.tar.gz",
        ] {
            assert!(
                policy
                    .try_capture(url, [0; 32], 20, "archive", &|| true, None)
                    .is_err(),
                "{url}"
            );
        }
        assert!(
            policy
                .try_capture(
                    "file:///not-read",
                    [0; 32],
                    20,
                    "archive",
                    &|| panic!("unselected path"),
                    None
                )
                .unwrap()
                .is_none()
        );
        fs::remove_file(root.path().join("good/archive.tar.gz")).unwrap();
        assert!(
            policy
                .try_capture(
                    "https://original.invalid/archive.tar.gz",
                    Sha256::digest(b"verified").into(),
                    20,
                    "archive",
                    &|| true,
                    None
                )
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn distdir_fatal_cancel_and_limit_never_fall_through() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("first")).unwrap();
        fs::create_dir(root.path().join("second")).unwrap();
        fs::write(root.path().join("first/archive.tar.gz"), b"too large").unwrap();
        fs::write(root.path().join("second/archive.tar.gz"), b"ok").unwrap();
        let policy =
            ArchiveDistdirPolicy::from_request(root.path(), &["first".into(), "second".into()])
                .unwrap();
        let digest = Sha256::digest(b"ok").into();
        let own = scope(SourceServiceLimits::default());
        assert!(
            policy
                .try_capture(
                    "https://original.invalid/archive.tar.gz",
                    digest,
                    2,
                    "archive",
                    &|| true,
                    Some(&own)
                )
                .unwrap_err()
                .contains("capture limit")
        );
        assert_eq!(own.snapshot().source_attempts, 1);
        assert_eq!(own.snapshot().transferred_bytes, 0);
        assert_eq!(own.snapshot().live_buffer_bytes, 0);
        let stopped = scope(SourceServiceLimits::default());
        assert!(
            policy
                .try_capture(
                    "https://original.invalid/archive.tar.gz",
                    digest,
                    20,
                    "archive",
                    &|| false,
                    Some(&stopped)
                )
                .is_err()
        );
        assert_eq!(stopped.snapshot().source_attempts, 0);
        let denied = scope(SourceServiceLimits::default());
        denied.poison(SourceServiceError::Cancelled);
        assert!(
            policy
                .try_capture(
                    "https://original.invalid/archive.tar.gz",
                    digest,
                    20,
                    "archive",
                    &|| true,
                    Some(&denied)
                )
                .is_err()
        );
        assert_eq!(denied.snapshot().source_attempts, 0);
        let calls = Cell::new(0);
        assert!(
            policy
                .try_capture(
                    "https://original.invalid/archive.tar.gz",
                    digest,
                    20,
                    "archive",
                    &|| {
                        calls.set(calls.get() + 1);
                        calls.get() != 5
                    },
                    None
                )
                .is_err()
        );
        assert_eq!(
            calls.get(),
            5,
            "fatal candidate cannot try the next directory"
        );
        let budget = scope(SourceServiceLimits {
            transferred_bytes: 1,
            ..SourceServiceLimits::default()
        });
        assert!(
            policy
                .try_capture(
                    "https://original.invalid/archive.tar.gz",
                    digest,
                    20,
                    "archive",
                    &|| true,
                    Some(&budget)
                )
                .is_err()
        );
        assert_eq!(budget.snapshot().source_attempts, 1);
        assert_eq!(budget.snapshot().transferred_bytes, 1);
        assert_eq!(budget.snapshot().written_bytes, 1);
        assert_eq!(budget.snapshot().live_buffer_bytes, 0);
        let address = scope(SourceServiceLimits::default());
        assert!(
            policy
                .try_capture(
                    &format!("https://original.invalid/{}", "x".repeat(8192)),
                    digest,
                    20,
                    "archive",
                    &|| true,
                    Some(&address)
                )
                .is_err()
        );
        assert_eq!(address.snapshot().source_attempts, 0);
        assert_eq!(address.snapshot().live_buffer_bytes, 0);
    }
}
