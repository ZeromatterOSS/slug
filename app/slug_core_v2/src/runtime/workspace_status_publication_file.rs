//! Attempt-local status file staging; never retained by a DICE value.
use std::io;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StablePublication {
    Preserved,
    Replaced,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PublicationOutcome {
    pub(super) stable: StablePublication,
}
#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod linux;

#[derive(Debug)]
pub(super) struct Staging {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    inner: Option<linux::Staging>,
}
impl Staging {
    pub(super) fn new(workspace: &Path, stable: &[u8], volatile: &[u8]) -> io::Result<Self> {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            Ok(Self {
                inner: Some(linux::Staging::new(workspace, stable, volatile)?),
            })
        }
        #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
        {
            let _ = (workspace, stable, volatile);
            Err(unsupported())
        }
    }
    pub(super) fn publish(
        &mut self,
        workspace: &Path,
        stable: &[u8],
        volatile: &[u8],
    ) -> io::Result<PublicationOutcome> {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            let outcome = self
                .inner
                .as_mut()
                .ok_or_else(|| io::Error::other("status stage retired"))?
                .publish(workspace, stable, volatile)?;
            // Even an outstanding scratch clone cannot retain descriptors after success.
            self.inner = None;
            Ok(outcome)
        }
        #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
        {
            let _ = (workspace, stable, volatile);
            Err(unsupported())
        }
    }
    #[cfg(all(test, target_os = "linux", target_env = "gnu"))]
    pub(super) fn names(&self) -> [&str; 2] {
        self.inner.as_ref().unwrap().names()
    }
    #[cfg(all(test, target_os = "linux", target_env = "gnu"))]
    pub(super) fn fail_second(&mut self) {
        self.inner.as_mut().unwrap().fail_second = true;
    }
}
#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn unsupported() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "native status publication requires Linux GNU",
    )
}
