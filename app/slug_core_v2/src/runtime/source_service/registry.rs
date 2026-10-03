use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use slug_bzlmod_v2::RegistryFileUrl;
use slug_bzlmod_v2::RegistryIo;
use slug_bzlmod_v2::RegistryIoOutcome;
use slug_bzlmod_v2::RegistryTransportError;

use super::SourceBufferReservation;
use super::SourceCounter;
use super::SourceHttpAdmission;
use super::SourceServiceError;
use super::SourceServiceScope;
use super::request_scoped;

pub(in crate::runtime) struct ScopedRegistryIo {
    scope: Arc<SourceServiceScope>,
}
impl ScopedRegistryIo {
    pub(in crate::runtime) fn new(scope: Arc<SourceServiceScope>) -> Self {
        Self { scope }
    }
}
fn error(message: impl ToString) -> RegistryTransportError {
    RegistryTransportError {
        message: message.to_string().into(),
    }
}

// Value before its reservation, including on every early return. Registry raw
// results cannot carry a bank guard: cumulative admitted bytes instead remain
// charged for the entire consumed scope/engine lifetime.
struct RegistryBuffer {
    bytes: Vec<u8>,
    _credit: SourceBufferReservation,
}
impl RegistryBuffer {
    fn new(scope: &Arc<SourceServiceScope>) -> Result<Self, RegistryTransportError> {
        let maximum = usize::try_from(scope.limits().registry_object_bytes).map_err(error)?;
        let credit = scope
            .reserve_buffer(maximum + std::mem::size_of::<Vec<u8>>())
            .map_err(error)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(maximum)
            .map_err(|_| error(scope.poison(SourceServiceError::Allocation)))?;
        // Exact reserve requests are explicit; reject an unexpectedly larger
        // reported capacity rather than using uncredited spare capacity.
        if bytes.capacity() > maximum {
            return Err(error(scope.poison(SourceServiceError::Allocation)));
        }
        Ok(Self {
            bytes,
            _credit: credit,
        })
    }
    fn append(
        &mut self,
        scope: &SourceServiceScope,
        bytes: &[u8],
    ) -> Result<(), RegistryTransportError> {
        let len = self.bytes.len().checked_add(bytes.len()).ok_or_else(|| {
            error(scope.poison(SourceServiceError::Overflow(SourceCounter::RegistryBytes)))
        })?;
        if len as u64 > scope.limits().registry_object_bytes {
            return Err(error(
                scope.poison(SourceServiceError::Limit(SourceCounter::RegistryBytes)),
            ));
        }
        scope
            .charge(SourceCounter::RegistryBytes, bytes.len() as u64)
            .map_err(error)?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn finish(
        self,
        scope: &Arc<SourceServiceScope>,
    ) -> Result<RegistryIoOutcome, RegistryTransportError> {
        scope.check().map_err(error)?;
        // Arc conversion may allocate: reserve its entire block while Vec
        // bytes and credit remain live. Scope cumulative bytes pins history.
        let _conversion = scope.reserve_buffer(self.bytes.len() + 64).map_err(error)?;
        let result = Arc::<[u8]>::from(self.bytes.as_slice());
        scope.check().map_err(error)?;
        drop(self);
        Ok(RegistryIoOutcome::Found(result))
    }
}

#[async_trait]
impl RegistryIo for ScopedRegistryIo {
    async fn read_exact(
        &self,
        url: &RegistryFileUrl,
    ) -> Result<RegistryIoOutcome, RegistryTransportError> {
        self.scope
            .charge(SourceCounter::RegistryAttempts, 1)
            .map_err(error)?;
        if url.as_str().len() > 8192 {
            return Err(error(self.scope.poison(SourceServiceError::BufferLimit)));
        }
        let _address_credit = self.scope.reserve_buffer(64 * 1024).map_err(error)?;
        let parsed =
            url::Url::parse(url.as_str()).map_err(|_| error("invalid scoped registry URL"))?;
        if parsed.scheme() == "file" {
            let path = parsed
                .to_file_path()
                .map_err(|_| error("invalid scoped local registry URL"))?;
            return read_local(&self.scope, &path);
        }
        let response = request_scoped(&self.scope, &parsed, SourceHttpAdmission::Registry)
            .await
            .map_err(error)?;
        read_response(&self.scope, response).await
    }
    async fn read_local_exact(
        &self,
        _url: &RegistryFileUrl,
        path: &Path,
    ) -> Result<RegistryIoOutcome, RegistryTransportError> {
        self.scope
            .charge(SourceCounter::RegistryAttempts, 1)
            .map_err(error)?;
        // Preserve the supplied typed path, including non-UTF8 and symlinks.
        read_local(&self.scope, path)
    }
}

pub(super) async fn read_response(
    scope: &Arc<SourceServiceScope>,
    mut response: super::http::ScopedHttpResponse,
) -> Result<RegistryIoOutcome, RegistryTransportError> {
    let status = response.status();
    let result = async {
        if status == hyper::StatusCode::NOT_FOUND {
            return Ok(RegistryIoOutcome::NotFound);
        }
        if !status.is_success() {
            return Err(error(format!("scoped registry returned HTTP {status}")));
        }
        if let Some(length) = response.headers().get(hyper::header::CONTENT_LENGTH) {
            if let Ok(length) = length.to_str().unwrap_or("").parse::<u64>() {
                if length > scope.limits().registry_object_bytes {
                    return Err(error(
                        scope.poison(SourceServiceError::Limit(SourceCounter::RegistryBytes)),
                    ));
                }
            }
        }
        let mut buffer = RegistryBuffer::new(scope)?;
        while let Some(chunk) = response.next_data().await.map_err(error)? {
            buffer.append(scope, &chunk)?;
        }
        buffer.finish(scope)
    }
    .await;
    let disposed = response.dispose().await;
    match result {
        Err(primary) => Err(primary),
        Ok(value) => {
            disposed.map_err(error)?;
            scope.check().map_err(error)?;
            Ok(value)
        }
    }
}

#[cfg(target_os = "linux")]
fn read_local(
    scope: &Arc<SourceServiceScope>,
    path: &Path,
) -> Result<RegistryIoOutcome, RegistryTransportError> {
    use std::fs::OpenOptions;
    use std::io::Read;
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::fs::OpenOptionsExt;
    scope.check().map_err(error)?;
    let pin = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_PATH | libc::O_CLOEXEC)
        .open(path)
    {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            scope.check().map_err(error)?;
            return Ok(RegistryIoOutcome::NotFound);
        }
        Err(_) => return Err(error("scoped registry inode pin failed")),
    };
    let before = pin
        .metadata()
        .map_err(|_| error("scoped registry pinned stat failed"))?;
    if !before.is_file() {
        return Err(error(
            scope.poison(SourceServiceError::UnsupportedSourceOwner),
        ));
    }
    if before.len() > scope.limits().registry_object_bytes {
        return Err(error(
            scope.poison(SourceServiceError::Limit(SourceCounter::RegistryBytes)),
        ));
    }
    scope.check().map_err(error)?;
    let mut input = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(format!("/proc/self/fd/{}", pin.as_raw_fd()))
        .map_err(|_| error("scoped registry data reopen failed"))?;
    let identity = |m: &std::fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.mode(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    };
    if identity(
        &input
            .metadata()
            .map_err(|_| error("scoped registry data stat failed"))?,
    ) != identity(&before)
    {
        return Err(error("scoped registry pinned identity changed"));
    }
    let mut buffer = RegistryBuffer::new(scope)?;
    let _read_credit = scope.reserve_buffer(64 * 1024).map_err(error)?;
    let mut chunk = [0u8; 64 * 1024];
    loop {
        scope.check().map_err(error)?;
        let read = input
            .read(&mut chunk)
            .map_err(|_| error("scoped registry local read failed"))?;
        scope.check().map_err(error)?;
        if read == 0 {
            break;
        }
        buffer.append(scope, &chunk[..read])?;
    }
    let after = input
        .metadata()
        .map_err(|_| error("scoped registry final stat failed"))?;
    if identity(&after) != identity(&before) || buffer.bytes.len() as u64 != before.len() {
        return Err(error("scoped registry inode changed while reading"));
    }
    buffer.finish(scope)
}
#[cfg(not(target_os = "linux"))]
fn read_local(
    scope: &Arc<SourceServiceScope>,
    _path: &Path,
) -> Result<RegistryIoOutcome, RegistryTransportError> {
    Err(error(
        scope.poison(SourceServiceError::UnsupportedSourceOwner),
    ))
}
