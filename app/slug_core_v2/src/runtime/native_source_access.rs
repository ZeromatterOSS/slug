//! Attempt-owned source lifetime/IO, separate from resource-free DICE metadata.
//! Factories receive ONLY the materializer's fully checked selected-root set.
//! No path-list constructor, global lookup, raw File/FD escape or scope refresh.
use std::io;
use std::ops::Deref;
use std::sync::Arc;

use super::ObservedSourceArtifactInput;
use super::PreparedActionChainInputs;
use super::PreparedSourceActionInputs;
use super::SourceArtifactInput;
use super::dice::NativeDemandSessionError;
use super::dice::source_service::SourceServiceScope;
use super::repository_io::physical_roots::SelectedRepositoryRoots;
#[cfg(target_os = "linux")]
pub(in crate::runtime) type NativeReaderResources =
    super::repository_physical_root::RepositoryRootResources;
#[cfg(target_os = "linux")]
pub(in crate::runtime) type NativeReaderCredit = super::repository_physical_root::ResourceCredit;
#[cfg(not(target_os = "linux"))]
#[derive(Debug)]
pub(in crate::runtime) struct NativeReaderResources {
    bytes: slug_workspace_v2::ImageDescriptorBudget,
    _control: slug_workspace_v2::ImageDescriptorReservation,
}
#[cfg(not(target_os = "linux"))]
#[derive(Debug)]
pub(in crate::runtime) struct NativeReaderCredit {
    _bytes: slug_workspace_v2::ImageDescriptorReservation,
}
#[cfg(not(target_os = "linux"))]
impl NativeReaderResources {
    pub(in crate::runtime) fn reserve(
        self: &Arc<Self>,
        bytes: usize,
        fds: usize,
        backing: u64,
    ) -> io::Result<NativeReaderCredit> {
        if fds != 0 || backing != 0 {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "native source reading requires Linux",
            ));
        }
        Ok(NativeReaderCredit {
            _bytes: self
                .bytes
                .try_reserve(bytes)
                .map_err(|_| io::Error::other("native reader retention exhausted"))?,
        })
    }
}
pub(in crate::runtime) fn new_reader_resources() -> io::Result<Arc<NativeReaderResources>> {
    #[cfg(target_os = "linux")]
    {
        NativeReaderResources::new(
            super::repository_physical_root::RepositoryRootResourceLimits::new(
                16 * 1024 * 1024,
                128,
                1,
            )
            .map_err(|_| io::Error::other("invalid fixed native reader policy"))?,
        )
        .map_err(|_| io::Error::other("native reader bank admission"))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let bytes = slug_workspace_v2::ImageDescriptorBudget::new(16 * 1024 * 1024);
        let control = bytes
            .try_reserve(256)
            .map_err(|_| io::Error::other("native reader bank admission"))?;
        Ok(Arc::new(NativeReaderResources {
            bytes,
            _control: control,
        }))
    }
}

#[path = "native_source_access/async_reader.rs"]
mod async_reader;
#[path = "native_source_access/reader.rs"]
mod reader;
pub use async_reader::NativeAsyncSourceReader;
pub use reader::NativeSourceReader;

struct Access {
    roots: SelectedRepositoryRoots,
    scope: Option<Arc<SourceServiceScope>>,
    readers: Arc<NativeReaderResources>,
    // Native owners and metadata precede the shared bank guard on destruction.
    _credit: NativeReaderCredit,
}
#[derive(Clone)]
struct NativeSourceAccess(Arc<Access>);
impl NativeSourceAccess {
    fn new(
        sources: &[Arc<ObservedSourceArtifactInput>],
        roots: SelectedRepositoryRoots,
        scope: Option<Arc<SourceServiceScope>>,
        readers: Arc<NativeReaderResources>,
    ) -> Result<Self, NativeDemandSessionError> {
        let credit = readers
            .reserve(std::mem::size_of::<Access>() + 128, 0, 0)
            .map_err(error)?;
        if let Some(scope) = &scope {
            scope.check().map_err(error)?;
        }
        // The COMPLETE source set is authenticated before any source open or
        // external transport effect, even when CAS will report every blob hit.
        for source in sources {
            let source = source.result().as_ref().map_err(error)?;
            let lease = roots.lease_for(source).map_err(error)?;
            #[cfg(target_os = "linux")]
            if let Some(root) = lease.as_ref().and_then(|l| l.physical()) {
                let current = scope
                    .as_ref()
                    .ok_or_else(|| error("physical source requires current source scope"))?;
                if current.workspace() != root.reservation().workspace() {
                    return Err(error("foreign physical source scope"));
                }
            }
        }
        if let Some(scope) = &scope {
            scope.check().map_err(error)?;
        }
        Ok(Self(Arc::new(Access {
            roots,
            scope,
            readers,
            _credit: credit,
        })))
    }
    fn check_active(&self) -> io::Result<()> {
        if let Some(scope) = &self.0.scope {
            scope
                .check()
                .map_err(|e| io::Error::other(format!("native source scope: {e:?}")))?;
        }
        Ok(())
    }
    async fn run_while_active<F: std::future::Future>(&self, future: F) -> io::Result<F::Output> {
        self.check_active()?;
        if self.0.scope.is_none() {
            return Ok(future.await);
        }
        tokio::pin!(future);
        loop {
            tokio::select! {
                result=&mut future=>{self.check_active()?;return Ok(result);}
                _=tokio::time::sleep(std::time::Duration::from_millis(10))=>self.check_active()?,
            }
        }
    }
    fn open(&self, source: &SourceArtifactInput) -> io::Result<NativeSourceReader> {
        reader::open(
            source,
            &self.0.roots,
            self.0.scope.clone(),
            self.0.readers.clone(),
        )
    }
}
fn error(value: impl std::fmt::Debug) -> NativeDemandSessionError {
    NativeDemandSessionError::Computation(anyhow::anyhow!("native source access: {value:?}"))
}

/// Pure prepared metadata plus exact selected native leases. Clone duplicates
/// no descriptor. This is a native transport value, NEVER a DICE value.
#[derive(Clone)]
pub struct NativeSourceActionInputs {
    prepared: Arc<PreparedSourceActionInputs>,
    access: NativeSourceAccess,
}
impl NativeSourceActionInputs {
    pub(in crate::runtime) fn new(
        prepared: Arc<PreparedSourceActionInputs>,
        roots: SelectedRepositoryRoots,
        scope: Option<Arc<SourceServiceScope>>,
        readers: Arc<NativeReaderResources>,
    ) -> Result<Self, NativeDemandSessionError> {
        let access = NativeSourceAccess::new(prepared.observed_sources(), roots, scope, readers)?;
        Ok(Self { prepared, access })
    }
    pub fn check_active(&self) -> io::Result<()> {
        self.access.check_active()
    }
    /// Retain this exact selected lease set and ORIGINAL scope while an owned
    /// transport effect runs. Cancellation drops the effect future; any already
    /// running blocking reader still owns its own complete lifetime guards.
    pub async fn run_while_active<F: std::future::Future>(
        &self,
        future: F,
    ) -> io::Result<F::Output> {
        self.access.run_while_active(future).await
    }
    pub fn metadata(&self) -> &PreparedSourceActionInputs {
        &self.prepared
    }
    pub fn open_source(&self, index: usize) -> io::Result<NativeSourceReader> {
        let source = self.prepared.sources().nth(index).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "unknown selected source")
        })?;
        self.access.open(source)
    }
}
impl Deref for NativeSourceActionInputs {
    type Target = PreparedSourceActionInputs;
    fn deref(&self) -> &Self::Target {
        self.metadata()
    }
}
impl std::fmt::Debug for NativeSourceActionInputs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeSourceActionInputs")
            .field("sources", &self.sources().len())
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct NativeActionChainInputs {
    prepared: Arc<PreparedActionChainInputs>,
    access: NativeSourceAccess,
}
impl NativeActionChainInputs {
    pub(in crate::runtime) fn new(
        prepared: Arc<PreparedActionChainInputs>,
        roots: SelectedRepositoryRoots,
        scope: Option<Arc<SourceServiceScope>>,
        readers: Arc<NativeReaderResources>,
    ) -> Result<Self, NativeDemandSessionError> {
        let access = NativeSourceAccess::new(prepared.observed_sources(), roots, scope, readers)?;
        Ok(Self { prepared, access })
    }
    pub fn check_active(&self) -> io::Result<()> {
        self.access.check_active()
    }
    /// Retain this exact selected lease set and ORIGINAL scope while an owned
    /// transport effect runs. Cancellation drops the effect future; any already
    /// running blocking reader still owns its own complete lifetime guards.
    pub async fn run_while_active<F: std::future::Future>(
        &self,
        future: F,
    ) -> io::Result<F::Output> {
        self.access.run_while_active(future).await
    }
    pub fn metadata(&self) -> &PreparedActionChainInputs {
        &self.prepared
    }
    pub fn open_source(&self, index: usize) -> io::Result<NativeSourceReader> {
        let source = self.prepared.sources().nth(index).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "unknown selected source")
        })?;
        self.access.open(source)
    }
}
impl Deref for NativeActionChainInputs {
    type Target = PreparedActionChainInputs;
    fn deref(&self) -> &Self::Target {
        self.metadata()
    }
}
impl std::fmt::Debug for NativeActionChainInputs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeActionChainInputs")
            .field("sources", &self.sources().len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "native_source_access/tests.rs"]
mod tests;

#[cfg(feature = "resource-set-test-support")]
#[path = "native_source_access/resource_set_counter.rs"]
mod resource_set_counter;
