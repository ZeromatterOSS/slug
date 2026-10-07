use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use slug_bzlmod_v2::RegistryFileUrl;
use slug_bzlmod_v2::RegistryIo;
use slug_bzlmod_v2::RegistryIoOutcome;
use slug_bzlmod_v2::RegistryRefusalCause;
use slug_bzlmod_v2::RegistryRefusalDetail;
use slug_bzlmod_v2::RegistryRefusalSite;
use slug_bzlmod_v2::RegistryTransportError;

use super::SourceBufferSite;
use super::SourceBufferUnit;
use super::SourceCounter;
use super::SourceHttpAdmission;
use super::SourceServiceError;
use super::SourceServiceScope;
use super::SourceTransportFailure;
use super::regular_file::Capture;
use super::regular_file::ReadPolicy;
use super::regular_file::RegularFileError;
use super::regular_file::RegularFileStage;
use super::regular_file::read_regular;
use super::request_scoped_typed;

pub(in crate::runtime) struct ScopedRegistryIo {
    scope: Arc<SourceServiceScope>,
}
impl ScopedRegistryIo {
    pub(in crate::runtime) fn new(scope: Arc<SourceServiceScope>) -> Self {
        Self { scope }
    }
}
fn error(error: SourceServiceError) -> RegistryTransportError {
    SourceTransportFailure::scope(error).into_registry()
}
fn opaque(site: RegistryRefusalSite, message: &'static str) -> RegistryTransportError {
    SourceTransportFailure::opaque(site, message).into_registry()
}

#[async_trait]
impl RegistryIo for ScopedRegistryIo {
    async fn read_exact(
        &self,
        url: &RegistryFileUrl,
    ) -> Result<RegistryIoOutcome, RegistryTransportError> {
        #[cfg(not(all(test, target_os = "linux")))]
        self.scope
            .charge(SourceCounter::RegistryAttempts, 1)
            .map_err(error)?;
        #[cfg(all(test, target_os = "linux"))]
        self.scope
            .charge_registry_request_for_test(super::registry_association::RegistryRequestRef::Url(
                url,
            ))
            .map_err(error)?;
        if url.as_str().len() > 8192 {
            return Err(error(self.scope.poison_buffer_limit_at(
                SourceBufferSite::RegistryUrlLength,
                url.as_str().len(),
                8192,
                SourceBufferUnit::Bytes,
            )));
        }
        let _address_credit = self
            .scope
            .reserve_buffer_at(64 * 1024, SourceBufferSite::RegistryAddress)
            .map_err(error)?;
        let parsed = url::Url::parse(url.as_str()).map_err(|_| {
            opaque(
                RegistryRefusalSite::AddressValidation,
                "invalid scoped registry URL",
            )
        })?;
        if parsed.scheme() == "file" {
            let path = parsed.to_file_path().map_err(|_| {
                opaque(
                    RegistryRefusalSite::AddressValidation,
                    "invalid scoped local registry URL",
                )
            })?;
            return read_local(&self.scope, &path);
        }
        let response = request_scoped_typed(&self.scope, &parsed, SourceHttpAdmission::Registry)
            .await
            .map_err(SourceTransportFailure::into_registry)?;
        read_response(&self.scope, response).await
    }
    async fn read_local_exact(
        &self,
        _url: &RegistryFileUrl,
        path: &Path,
    ) -> Result<RegistryIoOutcome, RegistryTransportError> {
        #[cfg(not(all(test, target_os = "linux")))]
        self.scope
            .charge(SourceCounter::RegistryAttempts, 1)
            .map_err(error)?;
        #[cfg(all(test, target_os = "linux"))]
        self.scope
            .charge_registry_request_for_test(
                super::registry_association::RegistryRequestRef::Local { url: _url, path },
            )
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
            return Err(SourceTransportFailure::new(
                format!("scoped registry returned HTTP {status}"),
                RegistryRefusalCause {
                    site: RegistryRefusalSite::HttpStatus,
                    detail: RegistryRefusalDetail::Status(status.as_u16()),
                },
            )
            .into_registry());
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
        let mut buffer = Capture::new(scope, ReadPolicy::Registry).map_err(error)?;
        while let Some(chunk) = response
            .next_data_typed()
            .await
            .map_err(SourceTransportFailure::into_registry)?
        {
            buffer.append(scope, &chunk).map_err(error)?;
        }
        buffer
            .finish(scope)
            .map(RegistryIoOutcome::Found)
            .map_err(error)
    }
    .await;
    let disposed = response.dispose_typed().await;
    match result {
        Err(primary) => Err(primary),
        Ok(value) => {
            disposed.map_err(SourceTransportFailure::into_registry)?;
            scope.check().map_err(error)?;
            Ok(value)
        }
    }
}

fn read_local(
    scope: &Arc<SourceServiceScope>,
    path: &Path,
) -> Result<RegistryIoOutcome, RegistryTransportError> {
    match read_regular(scope, path, ReadPolicy::Registry) {
        Ok(bytes) => Ok(RegistryIoOutcome::Found(bytes)),
        Err(RegularFileError::Scope(value)) => Err(error(value)),
        Err(RegularFileError::Io {
            stage: RegularFileStage::Pin,
            error: io,
            ..
        }) if io.kind() == std::io::ErrorKind::NotFound => {
            scope.check().map_err(error)?;
            Ok(RegistryIoOutcome::NotFound)
        }
        Err(RegularFileError::Io {
            stage,
            error: io,
            message,
        }) => {
            let site = match stage {
                RegularFileStage::Pin => RegistryRefusalSite::LocalPin,
                RegularFileStage::Stat => RegistryRefusalSite::LocalStat,
                RegularFileStage::Reopen => RegistryRefusalSite::LocalReopen,
                RegularFileStage::Read => RegistryRefusalSite::LocalRead,
                RegularFileStage::Identity => RegistryRefusalSite::LocalIdentity,
            };
            Err(SourceTransportFailure::io(site, &io, message).into_registry())
        }
        Err(RegularFileError::NonRegular { .. }) => Err(error(
            scope.poison(SourceServiceError::UnsupportedSourceOwner),
        )),
        Err(RegularFileError::Changed { before_read }) => Err(opaque(
            RegistryRefusalSite::LocalIdentity,
            if before_read {
                "scoped registry pinned identity changed"
            } else {
                "scoped registry inode changed while reading"
            },
        )),
    }
}
