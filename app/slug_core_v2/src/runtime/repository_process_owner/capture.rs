//! Actual bounded output capture. Source anchor: repository_process.rs
//! capture_bounded_with (EINTR/WouldBlock/EOF behavior), without its GCC policy.
use super::*;

pub(super) const STREAM_LIMIT: usize = 1024 * 1024;
pub(super) const CAPTURE_BYTES: usize = 4 * STREAM_LIMIT + 512;
pub(super) struct Captures {
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
    pub(super) out_eof: bool,
    pub(super) err_eof: bool,
    credit: resources::ResourceCredit,
}
impl Captures {
    pub(super) fn new(resources: &Arc<RepositoryRootResources>) -> Result<Self, PhysicalRootError> {
        let credit = resources.reserve(CAPTURE_BYTES, 0, 0)?;
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        stdout
            .try_reserve_exact(STREAM_LIMIT)
            .map_err(|_| PhysicalRootError::Resource)?;
        stderr
            .try_reserve_exact(STREAM_LIMIT)
            .map_err(|_| PhysicalRootError::Resource)?;
        if stdout.capacity() > STREAM_LIMIT || stderr.capacity() > STREAM_LIMIT {
            return Err(PhysicalRootError::Resource);
        }
        Ok(Self {
            stdout,
            stderr,
            out_eof: false,
            err_eof: false,
            credit,
        })
    }
    pub(super) fn drain(&mut self, fd: BorrowedFd<'_>, stderr: bool) -> io::Result<()> {
        self.drain_with(stderr, |scratch| {
            let n =
                unsafe { libc::read(fd.as_raw_fd(), scratch.as_mut_ptr().cast(), scratch.len()) };
            if n < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(n as usize)
            }
        })
    }
    pub(super) fn drain_with(
        &mut self,
        stderr: bool,
        mut read: impl FnMut(&mut [u8]) -> io::Result<usize>,
    ) -> io::Result<()> {
        let (bytes, eof) = if stderr {
            (&mut self.stderr, &mut self.err_eof)
        } else {
            (&mut self.stdout, &mut self.out_eof)
        };
        if *eof {
            return Ok(());
        }
        let mut scratch = [0u8; 8192];
        for _ in 0..16 {
            let n = match read(&mut scratch) {
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(e) => return Err(e),
            };
            if n == 0 {
                *eof = true;
                return Ok(());
            }
            if n > scratch.len() || n > STREAM_LIMIT - bytes.len() {
                return Err(io::Error::new(
                    io::ErrorKind::FileTooLarge,
                    "repository process output limit",
                ));
            }
            bytes.extend_from_slice(&scratch[..n]);
        }
        Ok(())
    }
    pub(super) fn finish(
        self,
        code: i32,
        mode: RunMode,
        nonce: [u8; 16],
        request: Option<Arc<RepositoryWorkspaceRequest>>,
        root: RootIdentity,
        _proof: super::lifecycle::NamespaceReaped,
    ) -> ReapedCapture {
        ReapedCapture {
            stdout: self.stdout,
            stderr: self.stderr,
            code,
            binding: ReapedBinding {
                request,
                root,
                mode,
                nonce,
            },
            credit: self.credit,
        }
    }
}

/// Constructed only by the owned native driver after actual all-child reap,
/// PID1 death, monitor reap and stream EOF. No immutable result injection seam.
pub(in crate::runtime::repository_physical_root) struct ReapedCapture {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    code: i32,
    binding: ReapedBinding,
    credit: resources::ResourceCredit,
}
impl ReapedCapture {
    pub(in crate::runtime::repository_physical_root) fn binding(&self) -> &ReapedBinding {
        &self.binding
    }
    pub(in crate::runtime::repository_physical_root) fn return_code(&self) -> i32 {
        self.code
    }
    pub(in crate::runtime::repository_physical_root) fn stdout(&self) -> &[u8] {
        &self.stdout
    }
    pub(in crate::runtime::repository_physical_root) fn stderr(&self) -> &[u8] {
        &self.stderr
    }
    pub(in crate::runtime::repository_physical_root) fn into_result(
        self,
    ) -> (
        slug_bzlmod_v2::RepositoryExecuteResult,
        ReapedBinding,
        resources::ResourceCredit,
    ) {
        let Self {
            stdout,
            stderr,
            code,
            binding,
            credit,
        } = self;
        let stdout: Arc<[u8]> = stdout.into();
        let stderr: Arc<[u8]> = stderr.into();
        (
            slug_bzlmod_v2::RepositoryExecuteResult::new(code, stdout, stderr),
            binding,
            credit,
        )
    }
}

/// A full structural request and exact physical generation, coupled to real
/// namespace quiescence. Neither a numeric index nor a digest can replace it.
pub(in crate::runtime::repository_physical_root) struct ReapedBinding {
    request: Option<Arc<RepositoryWorkspaceRequest>>,
    root: RootIdentity,
    mode: RunMode,
    nonce: [u8; 16],
}
impl ReapedBinding {
    pub(in crate::runtime::repository_physical_root) fn verify_attempt(
        &self,
        request: &RepositoryWorkspaceRequest,
        root: RootIdentity,
        nonce: &[u8; 16],
    ) -> Result<(), PhysicalRootError> {
        self.verify(request, root)?;
        if &self.nonce != nonce {
            return Err(PhysicalRootError::Reservation);
        }
        Ok(())
    }
    pub(in crate::runtime::repository_physical_root) fn verify(
        &self,
        request: &RepositoryWorkspaceRequest,
        root: RootIdentity,
    ) -> Result<(), PhysicalRootError> {
        if !self.mode.has_payload() || self.request.as_deref() != Some(request) || self.root != root
        {
            return Err(PhysicalRootError::Reservation);
        }
        Ok(())
    }
    pub(in crate::runtime::repository_physical_root) fn verify_bootstrap(
        &self,
        root: RootIdentity,
    ) -> Result<(), PhysicalRootError> {
        if self.mode != RunMode::InitializeArchive || self.request.is_some() || self.root != root {
            return Err(PhysicalRootError::Reservation);
        }
        Ok(())
    }
    pub(in crate::runtime::repository_physical_root) fn verify_finalize(
        &self,
        root: RootIdentity,
        mode: RunMode,
    ) -> Result<(), PhysicalRootError> {
        if !mode.is_finalize() || self.mode != mode || self.request.is_some() || self.root != root {
            return Err(PhysicalRootError::Reservation);
        }
        Ok(())
    }
}
