//! Private command-owned archive realization. Historical replay records never
//! grant source authority to this mutable tree; terminal adoption seals it.
use std::path::Path;
use std::sync::Arc;

use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan;
use slug_bzlmod_v2::GeneratedRepositoryOrderedEffectRef;
use slug_bzlmod_v2::RepositoryFilesystemCallId;
use slug_bzlmod_v2::RepositoryFilesystemOperation;
use slug_bzlmod_v2::RepositoryFilesystemRealization;
use slug_bzlmod_v2::RepositoryFilesystemRecord;
use slug_bzlmod_v2::RepositoryFilesystemRequest;
use slug_bzlmod_v2::RepositoryFilesystemResult;
use slug_bzlmod_v2::RepositoryFilesystemTranscript;

use super::repository_io::ArchiveMaterializationError;
use super::repository_io::Materialized;
use super::repository_io::source_service::SourceServiceScope;
use super::repository_io::source_service::{self};

struct Prefix {
    owner: Arc<slug_bzlmod_v2::HostSelectedExtensionOwner>,
    ordinal: usize,
    generated_repository: String,
    root: tempfile::TempDir,
    realized: GeneratedRepositoryFileEffectPlan,
    decoded: u64,
    deletion_work: deletion::DeleteWork,
    records: RepositoryFilesystemTranscript,
    scope: Option<Arc<SourceServiceScope>>,
}

/// Dropping a command drops every unfinished prefix. A failed operation removes
/// its prefix before doing I/O, so it cannot authorize a retry with partial bytes.
#[derive(Default)]
pub(super) struct RepositoryFilesystemProgress {
    prefixes: Vec<Prefix>,
}

impl RepositoryFilesystemProgress {
    pub(super) fn complete_call(
        &mut self,
        request: &Arc<RepositoryFilesystemRequest>,
        runtime: &tokio::runtime::Runtime,
        active: &dyn Fn() -> bool,
        scope: Option<&Arc<SourceServiceScope>>,
        distdirs: &super::ArchiveDistdirPolicy,
    ) -> Result<RepositoryFilesystemResult, ArchiveMaterializationError> {
        check(active, scope)?;
        let id = request.id();
        let mut prefix = match self.take(id) {
            Some(prefix) => prefix,
            None if id.call_index() == 0 => Prefix {
                owner: id.owner().clone(),
                ordinal: id.ordinal(),
                generated_repository: request.generated_repository().into(),
                root: tempfile::Builder::new()
                    .prefix("slug-repository-archive-")
                    .tempdir_in("/tmp")
                    .map_err(io_error)?,
                realized: GeneratedRepositoryFileEffectPlan::builder().finish(),
                decoded: 0,
                deletion_work: Default::default(),
                records: RepositoryFilesystemTranscript::empty(),
                scope: scope.cloned(),
            },
            None => return Err(spec("filesystem prefix is absent or was discarded")),
        };
        source_service::check_identity(prefix.scope.as_ref(), scope).map_err(io_error)?;
        if prefix.generated_repository != request.generated_repository()
            || prefix.records.records().len() != id.call_index()
            || !prefix.realized.is_prefix_of(request.prefix())
        {
            return Err(spec("filesystem request differs from completed prefix"));
        }
        apply_file_suffix(
            &prefix.realized,
            request.prefix(),
            prefix.root.path(),
            active,
            scope,
        )?;
        let result = match request.operation() {
            RepositoryFilesystemOperation::Archive(archive) => {
                let urls = archive
                    .urls()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>();
                let capture =
                super::repository_archive_http::capture_selected_generated_archive_with_distdirs(
                        &urls,
                        archive.sha256(),
                        runtime,
                        active,
                        scope,
                        distdirs,
                )?;
                super::repository_generated_archive::extract_generated_archive(
                    capture.as_file(),
                    prefix.root.path(),
                    archive.strip_prefix(),
                    archive.format(),
                    active,
                    &mut prefix.decoded,
                    scope,
                )
                .map_err(|error| {
                    ArchiveMaterializationError::materialization(format!(
                        "archive repository {} owner {} ordinal {} SHA256 {}: {}",
                        request.generated_repository(),
                        request.id().owner().unique_name(),
                        request.id().ordinal(),
                        hex::encode(archive.sha256()),
                        error.message,
                    ))
                })?;
                capture.close().map_err(io_error)?;
                RepositoryFilesystemResult::Archive(archive.sha256())
            }
            RepositoryFilesystemOperation::Delete(path) => {
                let removed = deletion::delete(
                    prefix.root.path(),
                    path,
                    &mut prefix.deletion_work,
                    active,
                    scope,
                )?;
                RepositoryFilesystemResult::Delete(removed)
            }
        };
        check(active, scope)?;
        prefix.realized = request
            .prefix()
            .with_operation(request.operation())
            .map_err(|_| spec("invalid realized archive prefix"))?;
        prefix.records = prefix
            .records
            .append(RepositoryFilesystemRecord::new(
                request.clone(),
                result.clone(),
            ))
            .map_err(|_| spec("invalid completed archive transcript"))?;
        check(active, scope)?;
        self.prefixes.push(prefix);
        Ok(result)
    }

    pub(super) fn finish(
        &mut self,
        realization: &RepositoryFilesystemRealization,
        expected_lease: u64,
        active: &dyn Fn() -> bool,
        scope: Option<&Arc<SourceServiceScope>>,
    ) -> Result<Materialized, ArchiveMaterializationError> {
        check(active, scope)?;
        if realization.lease_nonce() != expected_lease {
            return Err(spec("filesystem realization belongs to a stale command"));
        }
        let id =
            RepositoryFilesystemCallId::new(realization.owner().clone(), realization.ordinal(), 0);
        let prefix = self
            .take(&id)
            .ok_or_else(|| spec("completed filesystem root is absent"))?;
        source_service::check_identity(prefix.scope.as_ref(), scope).map_err(io_error)?;
        if prefix.records != *realization.transcript()
            || !prefix.realized.is_prefix_of(realization.terminal_plan())
        {
            return Err(spec(
                "terminal filesystem plan differs from verified realization",
            ));
        }
        apply_file_suffix(
            &prefix.realized,
            realization.terminal_plan(),
            prefix.root.path(),
            active,
            scope,
        )?;
        // Capture actual output facts once, after all mutations. No old prefix
        // record is used as an observation certificate for this final tree.
        let identity = manifest::final_manifest(prefix.root.path(), active, scope)?;
        check(active, scope)?;
        Ok(Materialized::AssociatedImmutable {
            source_identity: identity,
            root: prefix.root,
        })
    }

    #[cfg(test)]
    pub(super) fn provisional_paths(&self) -> Vec<std::path::PathBuf> {
        self.prefixes
            .iter()
            .map(|prefix| prefix.root.path().to_path_buf())
            .collect()
    }

    fn take(&mut self, id: &RepositoryFilesystemCallId) -> Option<Prefix> {
        self.prefixes
            .iter()
            .position(|prefix| &prefix.owner == id.owner() && prefix.ordinal == id.ordinal())
            .map(|index| self.prefixes.swap_remove(index))
    }
}

fn check(
    active: &dyn Fn() -> bool,
    scope: Option<&Arc<SourceServiceScope>>,
) -> Result<(), ArchiveMaterializationError> {
    if !active() {
        return Err(spec("archive command/session is no longer active"));
    }
    source_service::check(scope).map_err(ArchiveMaterializationError::materialization)
}
fn spec(message: &str) -> ArchiveMaterializationError {
    ArchiveMaterializationError::spec(message)
}
fn io_error(error: impl std::fmt::Display) -> ArchiveMaterializationError {
    ArchiveMaterializationError::materialization(error.to_string())
}

fn apply_file_suffix(
    realized: &GeneratedRepositoryFileEffectPlan,
    target: &GeneratedRepositoryFileEffectPlan,
    root: &Path,
    active: &dyn Fn() -> bool,
    scope: Option<&Arc<SourceServiceScope>>,
) -> Result<(), ArchiveMaterializationError> {
    if !realized.is_prefix_of(target) {
        return Err(spec(
            "archive suffix does not extend the exact realized prefix",
        ));
    }
    for effect in target.ordered_effects().skip(realized.effect_count()) {
        check(active, scope)?;
        let GeneratedRepositoryOrderedEffectRef::File(file) = effect else {
            return Err(spec(
                "unverified archive or unsupported symlink in realization suffix",
            ));
        };
        super::repository_io::apply_generated_repository_file(root, file, active, scope)?;
    }
    check(active, scope)
}

#[path = "repository_delete.rs"]
mod deletion;
#[path = "repository_filesystem_manifest.rs"]
mod manifest;
