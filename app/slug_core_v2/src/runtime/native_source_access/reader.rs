//! One native readable object and every owner/credit needed by its actual IO.
use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;

use slug_workspace_v2::FileContentDigest;

use super::*;
use crate::runtime::repository_io::physical_roots::SelectedRootLease;
#[cfg(target_os = "linux")]
use crate::runtime::repository_physical_root::RepositoryRootReader;
#[cfg(target_os = "linux")]
use crate::runtime::repository_physical_root::source::RepositorySourceReader;

enum Input {
    #[cfg(target_os = "linux")]
    Host(File),
    #[cfg(target_os = "linux")]
    Physical(RepositoryRootReader),
    #[cfg(target_os = "linux")]
    SourceSnapshot(RepositorySourceReader),
    #[cfg(not(target_os = "linux"))]
    Unavailable,
}
/// No raw descriptor or std File conversion is exposed. A detached reader keeps
/// only its selected minimal lease, expected digest, scope and own bank guard;
/// it does not retain the whole prepared action or workspace transcript.
pub struct NativeSourceReader {
    input: Input,
    _lease: Option<SelectedRootLease>,
    pub(super) scope: Option<Arc<SourceServiceScope>>,
    pub(super) resources: Arc<NativeReaderResources>,
    expected: FileContentDigest,
    #[cfg(all(test, target_os = "linux"))]
    gate: Option<Arc<ReadGate>>,
    _credit: NativeReaderCredit,
}
impl std::fmt::Debug for NativeSourceReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeSourceReader")
            .field("bytes", &self.expected.size_bytes())
            .finish_non_exhaustive()
    }
}
fn refusal(value: impl std::fmt::Debug) -> io::Error {
    io::Error::other(format!("native source refused: {value:?}"))
}
impl NativeSourceReader {
    /// Tests enter the production open kernel with an actual checked selected
    /// lease set. This creates neither a root nor a selected-root capability.
    #[cfg(test)]
    pub(in crate::runtime) fn open_selected_for_test(
        source: &SourceArtifactInput,
        roots: &SelectedRepositoryRoots,
        scope: Option<Arc<SourceServiceScope>>,
        resources: Arc<NativeReaderResources>,
    ) -> io::Result<Self> {
        open(source, roots, scope, resources)
    }
    pub fn digest(&self) -> FileContentDigest {
        self.expected
    }
    pub fn into_async(self) -> super::NativeAsyncSourceReader {
        super::async_reader::NativeAsyncSourceReader::new(self)
    }
    pub(super) fn check(&self) -> io::Result<()> {
        if let Some(scope) = &self.scope {
            scope.check().map_err(refusal)?;
        }
        Ok(())
    }
}
pub(super) fn open(
    source: &SourceArtifactInput,
    roots: &SelectedRepositoryRoots,
    scope: Option<Arc<SourceServiceScope>>,
    resources: Arc<NativeReaderResources>,
) -> io::Result<NativeSourceReader> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (source, roots, scope, resources);
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "native source reading requires Linux",
        ));
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(scope) = &scope {
            scope.check().map_err(refusal)?;
        }
        let lease = roots.lease_for(source).map_err(refusal)?;
        // Positive shared finite reader admission applies to ALL reader variants.
        let credit = resources
            .reserve(std::mem::size_of::<NativeSourceReader>() + 128, 1, 0)
            .map_err(refusal)?;
        let input = if let Some(root) = lease.as_ref().and_then(|l| l.physical()) {
            let scope = scope
                .as_ref()
                .ok_or_else(|| refusal("physical source requires current source scope"))?;
            let anchor = source
                .anchor()
                .ok_or_else(|| refusal("physical source lost anchor"))?;
            if anchor != root.anchor() || scope.workspace() != root.reservation().workspace() {
                return Err(refusal("foreign physical source scope"));
            }
            let relative = source
                .real_path()
                .as_path()
                .strip_prefix(anchor.root().as_path())
                .ok()
                .and_then(|p| p.to_str())
                .ok_or_else(|| refusal("physical source escapes selected anchor"))?;
            Input::Physical(
                root.open_member(relative, scope, &|| true)
                    .map_err(refusal)?,
            )
        } else if let Some(root) = lease.as_ref().and_then(|l| l.source_snapshot()) {
            let scope = scope
                .as_ref()
                .ok_or_else(|| refusal("source snapshot requires current source scope"))?;
            let anchor = source
                .anchor()
                .ok_or_else(|| refusal("source snapshot lost anchor"))?;
            if anchor != root.anchor() || scope.workspace() != root.reservation().workspace() {
                return Err(refusal("foreign source snapshot scope"));
            }
            let relative = source
                .real_path()
                .as_path()
                .strip_prefix(anchor.root().as_path())
                .ok()
                .and_then(|p| p.to_str())
                .ok_or_else(|| refusal("source snapshot escapes selected anchor"))?;
            Input::SourceSnapshot(
                root.open_member(relative, scope, &|| true)
                    .map_err(refusal)?,
            )
        } else {
            // Protected Host/legacy acquisition policy: ordinary observed real path,
            // nonblocking open, then regular-file validation. No new proc policy.
            let mut options = File::options();
            options.read(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.custom_flags(nix::libc::O_NONBLOCK);
            }
            let file = options.open(source.real_path().as_path())?;
            if !file.metadata()?.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "selected source is no longer a regular file",
                ));
            }
            Input::Host(file)
        };
        let reader = NativeSourceReader {
            input,
            _lease: lease,
            scope,
            resources,
            expected: source.digest(),
            #[cfg(all(test, target_os = "linux"))]
            gate: None,
            _credit: credit,
        };
        reader.check()?;
        Ok(reader)
    }
}
impl Read for NativeSourceReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        self.check()?;
        #[cfg(all(test, target_os = "linux"))]
        if let Some(gate) = &self.gate {
            gate.wait();
        }
        self.check()?;
        let count = match &mut self.input {
            #[cfg(target_os = "linux")]
            Input::Host(file) => file.read(out),
            #[cfg(target_os = "linux")]
            Input::Physical(reader) => reader.read(out),
            #[cfg(target_os = "linux")]
            Input::SourceSnapshot(reader) => reader.read(out),
            #[cfg(not(target_os = "linux"))]
            Input::Unavailable => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "native source reading requires Linux",
            )),
        }?;
        self.check()?;
        Ok(count)
    }
}
impl Seek for NativeSourceReader {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.check()?;
        let position = match &mut self.input {
            #[cfg(target_os = "linux")]
            Input::Host(file) => file.seek(from),
            #[cfg(target_os = "linux")]
            Input::Physical(reader) => reader.seek(from),
            #[cfg(target_os = "linux")]
            Input::SourceSnapshot(reader) => reader.seek(from),
            #[cfg(not(target_os = "linux"))]
            Input::Unavailable => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "native source reading requires Linux",
            )),
        }?;
        self.check()?;
        Ok(position)
    }
}

#[cfg(all(test, target_os = "linux"))]
pub(super) struct ReadGate {
    pub(super) entered: std::sync::atomic::AtomicBool,
    released: std::sync::Mutex<bool>,
    wake: std::sync::Condvar,
}
#[cfg(all(test, target_os = "linux"))]
impl ReadGate {
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self {
            entered: std::sync::atomic::AtomicBool::new(false),
            released: std::sync::Mutex::new(false),
            wake: std::sync::Condvar::new(),
        })
    }
    fn wait(&self) {
        self.entered
            .store(true, std::sync::atomic::Ordering::Release);
        let mut released = self.released.lock().unwrap();
        while !*released {
            released = self.wake.wait(released).unwrap();
        }
    }
    pub(super) fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.wake.notify_all();
    }
}
// Private cfg(test) Host-kernel fixture only. No selected-root/native-wrapper
// constructor is exposed and no physical source authority can be fabricated.
#[cfg(all(test, target_os = "linux"))]
pub(super) fn host_for_test(
    path: &std::path::Path,
    expected: FileContentDigest,
    scope: Option<Arc<SourceServiceScope>>,
    resources: Arc<NativeReaderResources>,
    gate: Option<Arc<ReadGate>>,
) -> io::Result<NativeSourceReader> {
    if let Some(scope) = &scope {
        scope.check().map_err(refusal)?;
    }
    let credit = resources
        .reserve(std::mem::size_of::<NativeSourceReader>() + 128, 1, 0)
        .map_err(refusal)?;
    let mut options = File::options();
    options.read(true);
    use std::os::unix::fs::OpenOptionsExt;
    options.custom_flags(nix::libc::O_NONBLOCK);
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "test source is not regular",
        ));
    }
    Ok(NativeSourceReader {
        input: Input::Host(file),
        _lease: None,
        scope,
        resources,
        expected,
        gate,
        _credit: credit,
    })
}
