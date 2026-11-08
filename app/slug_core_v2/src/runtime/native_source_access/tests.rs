//! Ordinary owned-reader/kernel tests. Actual physical A/B registry integration
//! is a separate native gate; these tests do not mint a fake physical lease.
#![cfg(target_os = "linux")]
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use sha2::Digest;
use sha2::Sha256;
use slug_workspace_v2::FileContentDigest;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::*;
use crate::runtime::dice::source_service::SourceServiceLimits;

fn digest(bytes: &[u8]) -> FileContentDigest {
    FileContentDigest::new(Sha256::digest(bytes).into(), bytes.len() as u64).unwrap()
}
fn scope(duration: Duration) -> Arc<SourceServiceScope> {
    let deadline = Instant::now() + duration;
    SourceServiceScope::new_for_test(
        NormalizedAbsolutePath::new("/native-reader-test").unwrap(),
        deadline,
        deadline + Duration::from_secs(1),
        SourceServiceLimits::default(),
    )
    .unwrap()
}
#[test]
fn shared_reader_fd_admission_and_seek_release_exact_credits() {
    let bank = NativeReaderResources::new(
        crate::runtime::repository_physical_root::RepositoryRootResourceLimits::new(
            1024 * 1024,
            1,
            1,
        )
        .unwrap(),
    )
    .unwrap();
    let baseline = bank.snapshot().retained_bytes;
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(b"abc\0\xff").unwrap();
    let mut reader =
        reader::host_for_test(file.path(), digest(b"abc\0\xff"), None, bank.clone(), None).unwrap();
    assert_eq!(bank.snapshot().native_fds, 1);
    assert!(
        reader::host_for_test(file.path(), digest(b"abc\0\xff"), None, bank.clone(), None).is_err()
    );
    assert_eq!(bank.snapshot().native_fds, 1);
    reader.seek(SeekFrom::Start(3)).unwrap();
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"\0\xff");
    drop(reader);
    assert_eq!(bank.snapshot().native_fds, 0);
    assert_eq!(bank.snapshot().retained_bytes, baseline);
}
#[tokio::test]
async fn async_reader_streams_beyond_filebytes_cap_without_refreshing_budgets() {
    let bank = new_reader_resources().unwrap();
    let baseline = bank.snapshot().retained_bytes;
    let body = vec![0xff; 2 * 1024 * 1024 + 3];
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(&body).unwrap();
    let reader =
        reader::host_for_test(file.path(), digest(&body), None, bank.clone(), None).unwrap();
    let mut reader = reader.into_async();
    let mut copied = Vec::new();
    tokio::io::AsyncReadExt::read_to_end(&mut reader, &mut copied)
        .await
        .unwrap();
    assert_eq!(copied, body);
    drop(reader);
    assert_eq!(bank.snapshot().native_fds, 0);
    assert_eq!(bank.snapshot().retained_bytes, baseline);
}
#[tokio::test]
async fn dropping_async_adapter_keeps_entire_blocking_job_and_credits_alive() {
    let bank = new_reader_resources().unwrap();
    let baseline = bank.snapshot().retained_bytes;
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(b"actual bytes").unwrap();
    let gate = reader::ReadGate::new();
    let reader = reader::host_for_test(
        file.path(),
        digest(b"actual bytes"),
        None,
        bank.clone(),
        Some(gate.clone()),
    )
    .unwrap();
    let task = tokio::spawn(async move {
        let mut reader = reader.into_async();
        let mut out = [0u8; 4];
        tokio::io::AsyncReadExt::read_exact(&mut reader, &mut out).await
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while !gate.entered.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(bank.snapshot().native_fds, 1);
    assert!(bank.snapshot().retained_bytes >= baseline + 64 * 1024);
    gate.release();
    tokio::time::timeout(Duration::from_secs(2), async {
        while {
            let now = bank.snapshot();
            now.native_fds != 0 || now.retained_bytes != baseline
        } {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(bank.snapshot().retained_bytes, baseline);
}
#[tokio::test]
async fn original_scope_expires_even_when_effect_never_opens_a_reader() {
    struct Pending(Arc<AtomicBool>);
    impl std::future::Future for Pending {
        type Output = ();
        fn poll(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<()> {
            std::task::Poll::Pending
        }
    }
    impl Drop for Pending {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }
    let bank = new_reader_resources().unwrap();
    let active = scope(Duration::from_millis(60));
    let original = active.work_deadline();
    let access = NativeSourceAccess::new(
        &[],
        SelectedRepositoryRoots::default(),
        Some(active.clone()),
        bank,
    )
    .unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    let result = tokio::time::timeout(
        Duration::from_secs(1),
        access.run_while_active(Pending(dropped.clone())),
    )
    .await
    .unwrap();
    assert!(result.is_err());
    assert!(dropped.load(Ordering::Acquire));
    assert_eq!(active.work_deadline(), original);
}
#[test]
fn expired_scope_is_nonretryable_and_refuses_open_without_fd_charge() {
    let bank = new_reader_resources().unwrap();
    let active = scope(Duration::from_millis(5));
    std::thread::sleep(Duration::from_millis(10));
    let file = tempfile::NamedTempFile::new().unwrap();
    let result = reader::host_for_test(file.path(), digest(b""), Some(active), bank.clone(), None);
    assert_ne!(result.unwrap_err().kind(), io::ErrorKind::Interrupted);
    assert_eq!(bank.snapshot().native_fds, 0);
}
