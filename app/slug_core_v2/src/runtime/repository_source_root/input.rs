//! Closed source acquisition. Only actual admitted producers create these
//! inputs; captured files, source descriptors and storage credits remain private.
use std::io::Write;
use std::os::unix::fs::FileExt;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::OpenOptionsExt;

use sha2::Digest;
use sha2::Sha256;
use slug_bzlmod_v2::RepositorySourceSnapshotKind as Kind;

use super::*;
use crate::runtime::ArchiveDistdirPolicy;
use crate::runtime::repository_archive::SelectedBcrArchive;

pub(in crate::runtime) struct SourceRootInput {
    request: Arc<Request>,
    input: Input,
    resources: Arc<RepositoryRootResources>,
    scope: Arc<SourceServiceScope>,
    #[cfg(test)]
    after_local_capture: Option<Box<dyn FnOnce() + Send>>,
    _credit: ResourceCredit,
    // LAST: acquisition inputs may outlive the outer command while its async
    // initializer is being cancelled or a worker result remains queued.
    retirement: Option<progress::command::RepositoryWorkspaceRetirementGuard>,
}
enum Input {
    Local(LocalInput),
    Builtin(slug_bzlmod_v2::BuiltinBazelToolsRouteIdentity),
    Archive(Archive),
}
struct LocalInput {
    file: File,
    anchor: Option<File>,
    anchor_name: Option<slug_workspace_v2::NormalizedAbsolutePath>,
    relative: std::path::PathBuf,
    named: std::path::PathBuf,
    logical: std::path::PathBuf,
}
impl LocalInput {
    fn validate_name(&self) -> Result<(), PhysicalRootError> {
        if self.logical.canonicalize()? != self.named {
            return Err(PhysicalRootError::Snapshot);
        }
        let current = if let Some(anchor) = &self.anchor {
            let named_anchor = crate::runtime::input_image::open_image_anchor(
                self.anchor_name
                    .as_ref()
                    .ok_or(PhysicalRootError::Reservation)?,
            )
            .map_err(|_| PhysicalRootError::Snapshot)?;
            if !same_metadata(&anchor.metadata()?, &named_anchor.metadata()?) {
                return Err(PhysicalRootError::Snapshot);
            }
            let base = RootIo::new(anchor, host_limits())?;
            base.open_directory(&self.relative)?
        } else {
            crate::runtime::input_image::open_image_anchor(
                &slug_workspace_v2::NormalizedAbsolutePath::new(&self.named)
                    .map_err(|_| PhysicalRootError::Reservation)?,
            )
            .map_err(|_| PhysicalRootError::Snapshot)?
        };
        if !same_metadata(&self.file.metadata()?, &current.metadata()?) {
            return Err(PhysicalRootError::Snapshot);
        }
        Ok(())
    }
}
fn host_limits() -> RootIoLimits {
    RootIoLimits {
        max_path_bytes: 4096,
        max_name_bytes: 255,
        max_depth: 2048,
        max_entries: 8192,
        max_directory_name_bytes: 1024 * 1024,
        max_file_bytes: 64 * 1024 * 1024,
        max_link_bytes: 256,
    }
}
struct Archive {
    plan: SelectedBcrArchive,
    archive: Capture,
    overlays: Vec<Capture>,
    patches: Vec<Capture>,
    module: Capture,
}
struct Capture {
    file: Option<tempfile::NamedTempFile>,
    frame: Option<std::fs::Metadata>,
    digest: [u8; 32],
    _credit: ResourceCredit,
    storage: Option<ResourceCredit>,
}
impl Capture {
    fn acquire(
        expected: [u8; 32],
        limit: u64,
        resources: &Arc<RepositoryRootResources>,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
        producer: impl FnOnce() -> Result<
            tempfile::NamedTempFile,
            crate::runtime::repository_io::ArchiveMaterializationError,
        >,
    ) -> Result<Self, PhysicalRootError> {
        check(scope, active)?;
        let credit = resources.reserve(std::mem::size_of::<Self>() + 64 * 1024 + 8192, 1, 0)?;
        let storage = resources.reserve(0, 0, limit)?;
        let file = producer().map_err(PhysicalRootError::Mutation)?;
        // File is armed before metadata/SHA checks; every failure closes first.
        let mut owner = Self {
            file: Some(file),
            frame: None,
            digest: expected,
            _credit: credit,
            storage: Some(storage),
        };
        owner.frame = Some(owner.file().metadata()?);
        owner.verify(limit, scope, active)?;
        Ok(owner)
    }
    fn frame(&self) -> &std::fs::Metadata {
        self.frame.as_ref().expect("verified capture frame")
    }
    fn close(&mut self) -> Result<(), PhysicalRootError> {
        if let Some(file) = self.file.take() {
            if let Err(error) = file.close() {
                if let Some(storage) = self.storage.take() {
                    std::mem::forget(storage)
                }
                return Err(PhysicalRootError::Io(error));
            }
        }
        Ok(())
    }
    fn file(&self) -> &File {
        self.file.as_ref().expect("live source capture").as_file()
    }
    fn verify(
        &self,
        limit: u64,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), PhysicalRootError> {
        let before = self.file().metadata()?;
        if !before.is_file()
            || before.nlink() != 1
            || before.len() > limit
            || !same_metadata(&before, self.frame())
        {
            return Err(PhysicalRootError::Snapshot);
        }
        let mut hash = Sha256::new();
        self.read_chunks(scope, active, |chunk| {
            hash.update(chunk);
            Ok(())
        })?;
        if <[u8; 32]>::from(hash.finalize()) != self.digest {
            return Err(PhysicalRootError::Snapshot);
        }
        Ok(())
    }
    fn read_chunks(
        &self,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
        mut visit: impl FnMut(&[u8]) -> Result<(), PhysicalRootError>,
    ) -> Result<(), PhysicalRootError> {
        let mut buffer = [0u8; 64 * 1024];
        let mut offset = 0;
        while offset < self.frame().len() {
            check(scope, active)?;
            let count = self.file().read_at(
                &mut buffer[..((self.frame().len() - offset).min(64 * 1024)) as usize],
                offset,
            )?;
            if count == 0 {
                return Err(PhysicalRootError::Snapshot);
            }
            visit(&buffer[..count])?;
            offset += count as u64;
        }
        if self.file().read_at(&mut [0u8; 1], offset)? != 0
            || !same_metadata(self.frame(), &self.file().metadata()?)
        {
            return Err(PhysicalRootError::Snapshot);
        }
        check(scope, active)
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
fn same_metadata(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.mode() == b.mode()
        && a.nlink() == b.nlink()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}
impl SourceRootInput {
    pub(in crate::runtime) fn capture(
        request: Arc<Request>,
        resources: &Arc<RepositoryRootResources>,
        runtime: &tokio::runtime::Runtime,
        distdirs: &ArchiveDistdirPolicy,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        check(scope, active)?;
        check_bank(scope, resources)?;
        if request.workspace() != scope.workspace() {
            return Err(PhysicalRootError::Reservation);
        }
        let retained = request
            .retained_storage_bytes()
            .map_err(PhysicalRootError::Protocol)?;
        let credit = resources.reserve(
            add(
                retained.checked_mul(4).ok_or(PhysicalRootError::Resource)?,
                8 * 1024 * 1024,
            )?,
            8,
            0,
        )?;
        let input = match request.kind() {
            Kind::Workspace | Kind::Local => {
                let path = if request.kind() == Kind::Workspace {
                    request.workspace()
                } else {
                    request.local_root().ok_or(PhysicalRootError::Reservation)?
                };
                if request.kind() == Kind::Local
                    && request.local_path_policy()
                        == Some(slug_bzlmod_v2::HostRepositoryLocalPathPolicy::LocalUnsupported)
                {
                    return Err(PhysicalRootError::UnsupportedOperation);
                }
                let actual = path.as_path().canonicalize()?;
                if request.kind() == Kind::Local
                    && request.local_path_policy()
                        == Some(slug_bzlmod_v2::HostRepositoryLocalPathPolicy::WorkspaceRelative)
                    && !actual.starts_with(request.workspace().as_path())
                {
                    return Err(PhysicalRootError::Reservation);
                }
                let relative_policy = request.kind() == Kind::Workspace
                    || request.local_path_policy()
                        == Some(slug_bzlmod_v2::HostRepositoryLocalPathPolicy::WorkspaceRelative);
                let (file, anchor, relative) = if relative_policy {
                    let anchor =
                        crate::runtime::input_image::open_image_anchor(request.workspace())
                            .map_err(|_| PhysicalRootError::Snapshot)?;
                    let relative = actual
                        .strip_prefix(request.workspace().as_path())
                        .map_err(|_| PhysicalRootError::Reservation)?
                        .to_path_buf();
                    let io = RootIo::new(&anchor, host_limits())?;
                    let file = io.open_directory(&relative)?;
                    (file, Some(anchor), relative)
                } else {
                    let file = crate::runtime::input_image::open_image_anchor(
                        &slug_workspace_v2::NormalizedAbsolutePath::new(&actual)
                            .map_err(|_| PhysicalRootError::Reservation)?,
                    )
                    .map_err(|_| PhysicalRootError::Snapshot)?;
                    (file, None, std::path::PathBuf::new())
                };
                let owner = LocalInput {
                    file,
                    anchor,
                    anchor_name: if relative_policy {
                        Some(request.workspace().clone())
                    } else {
                        None
                    },
                    relative,
                    named: actual,
                    logical: path.as_path().to_path_buf(),
                };
                owner.validate_name()?;
                Input::Local(owner)
            }
            Kind::Builtin => Input::Builtin(
                request
                    .builtin_identity()
                    .ok_or(PhysicalRootError::Reservation)?,
            ),
            Kind::Immutable => {
                let crate::runtime::repository_archive::ArchivePlan::SelectedBcrTarGz(plan) =
                    crate::runtime::repository_archive::parse_archive_plan(
                        request.repo_spec().ok_or(PhysicalRootError::Reservation)?,
                    )
                    .map_err(|_| PhysicalRootError::UnsupportedOperation)?
                else {
                    return Err(PhysicalRootError::UnsupportedOperation);
                };
                let count = plan
                    .overlays
                    .len()
                    .checked_add(plan.patches.len())
                    .and_then(|n| n.checked_add(2))
                    .ok_or(PhysicalRootError::Resource)?;
                let storage = (plan.overlays.len() as u64)
                    .checked_mul(64 * 1024 * 1024)
                    .and_then(|n| {
                        n.checked_add((plan.patches.len() as u64).checked_mul(8 * 1024 * 1024)?)
                    })
                    .and_then(|n| n.checked_add(129 * 1024 * 1024))
                    .ok_or(PhysicalRootError::Resource)?;
                // Forecast the WHOLE capture set before the first acquisition;
                // individual owners take over their paid storage/FD lifetime.
                let _admission = resources.reserve(
                    count
                        .checked_mul(std::mem::size_of::<Capture>() + 64)
                        .ok_or(PhysicalRootError::Resource)?,
                    count,
                    storage,
                )?;
                let archive = Capture::acquire(
                    plan.integrity,
                    128 * 1024 * 1024,
                    resources,
                    scope,
                    active,
                    || {
                        crate::runtime::repository_archive_http::capture_selected_bcr_with_distdirs(
                            &plan,
                            runtime,
                            active,
                            Some(scope),
                            distdirs,
                        )
                    },
                )?;
                let mut overlays = Vec::new();
                let mut patches = Vec::new();
                overlays
                    .try_reserve_exact(plan.overlays.len())
                    .map_err(|_| PhysicalRootError::Resource)?;
                patches
                    .try_reserve_exact(plan.patches.len())
                    .map_err(|_| PhysicalRootError::Resource)?;
                for overlay in &plan.overlays {
                    overlays.push(Capture::acquire(
                        overlay.integrity,
                        64 * 1024 * 1024,
                        resources,
                        scope,
                        active,
                        || {
                            crate::runtime::repository_archive_http::capture_selected_bcr_overlay(
                                overlay,
                                runtime,
                                active,
                                Some(scope),
                            )
                        },
                    )?);
                }
                for patch in &plan.patches {
                    patches.push(Capture::acquire(
                        patch.integrity,
                        8 * 1024 * 1024,
                        resources,
                        scope,
                        active,
                        || {
                            crate::runtime::repository_archive_http::capture_selected_bcr_patch(
                                patch,
                                runtime,
                                active,
                                Some(scope),
                            )
                        },
                    )?);
                }
                let module = Capture::acquire(
                    plan.module_integrity,
                    1024 * 1024,
                    resources,
                    scope,
                    active,
                    || {
                        crate::runtime::repository_archive_http::capture_selected_bcr_module(
                            &plan,
                            runtime,
                            active,
                            Some(scope),
                        )
                    },
                )?;
                Input::Archive(Archive {
                    plan,
                    archive,
                    overlays,
                    patches,
                    module,
                })
            }
        };
        check(scope, active)?;
        Ok(Self {
            request,
            input,
            resources: resources.clone(),
            scope: scope.clone(),
            #[cfg(test)]
            after_local_capture: None,
            _credit: credit,
            retirement: None,
        })
    }
    pub(in crate::runtime::repository_physical_root) fn check_binding(
        &self,
        reservation: &Reservation,
        resources: &Arc<RepositoryRootResources>,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), PhysicalRootError> {
        super::super::check_origin(&self.scope, scope, active)?;
        if !Arc::ptr_eq(&self.resources, resources)
            || self.request.as_ref() != reservation.request().as_ref()
        {
            return Err(PhysicalRootError::Reservation);
        }
        check(scope, active)
    }
    pub(in crate::runtime::repository_physical_root) fn with_retirement(
        mut self,
        observer: progress::command::RepositoryWorkspaceRetirementObserver,
    ) -> Self {
        assert!(self.retirement.is_none());
        self.retirement = Some(observer.worker_guard());
        self
    }
    #[cfg(test)]
    pub(super) fn after_local_capture_for_test(
        mut self,
        hook: impl FnOnce() + Send + 'static,
    ) -> Self {
        self.after_local_capture = Some(Box::new(hook));
        self
    }
    pub(in crate::runtime::repository_physical_root) fn populate(
        mut self,
        builder: &SourceRootBuilder,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<tree::CapturedTree, PhysicalRootError> {
        builder.check(scope, active)?;
        if !Arc::ptr_eq(&self.scope, scope)
            || !Arc::ptr_eq(&self.resources, &builder.resources)
            || self.request.as_ref() != builder.reservation.request().as_ref()
        {
            return Err(PhysicalRootError::Reservation);
        }
        let io = RootIo::new(&builder.file, limits(&builder.reservation)?)?;
        let _scratch = builder.resources.reserve(2 * IO_SCRATCH, 8, 0)?;
        let running = || active() && scope.check().is_ok();
        let captured = match &self.input {
            Input::Local(input) => {
                input.validate_name()?;
                let source = RootIo::new(&input.file, limits(&builder.reservation)?)?;
                let first = tree::capture(
                    &source,
                    &builder.reservation,
                    Some(&io),
                    &self.resources,
                    scope,
                    active,
                )?;
                #[cfg(test)]
                if let Some(hook) = self.after_local_capture.take() {
                    hook();
                }
                let second = tree::capture(
                    &source,
                    &builder.reservation,
                    None,
                    &self.resources,
                    scope,
                    active,
                )?;
                if !first.same_host_capture(&second) {
                    return Err(PhysicalRootError::Snapshot);
                }
                input.validate_name()?;
                let output = builder.capture(scope, active)?;
                if first.entries() != output.entries() {
                    return Err(PhysicalRootError::Snapshot);
                }
                Ok(output)
            }
            Input::Builtin(identity) => {
                identity.snapshot().visit_validated_catalog(
                    *identity,
                    |path, body, executable, _digest| -> Result<(), PhysicalRootError> {
                        check(scope, active)?;
                        if body.len() > 64 * 1024 * 1024 {
                            return Err(PhysicalRootError::Resource);
                        }
                        if let Some(parent) = Path::new(path).parent() {
                            io.ensure_directory(parent, 0o755)?;
                        }
                        scope.charge(
                            crate::runtime::dice::source_service::SourceCounter::WrittenBytes,
                            body.len() as u64,
                        )?;
                        io.write_file(
                            Path::new(path),
                            body,
                            if executable { 0o755 } else { 0o644 },
                            crate::runtime::repository_root_io::WriteMode::CreateNew,
                            &running,
                        )?;
                        Ok(())
                    },
                )?;
                builder.capture(scope, active)
            }
            Input::Archive(archive) => {
                archive.archive.verify(128 * 1024 * 1024, scope, active)?;
                crate::runtime::repository_archive_realize::extract_selected_bcr_at(
                    archive.archive.file(),
                    &io,
                    archive.plan.strip_prefix.as_deref(),
                    &running,
                    Some(scope),
                )
                .map_err(PhysicalRootError::Mutation)?;
                archive.archive.verify(128 * 1024 * 1024, scope, active)?;
                for (entry, capture) in archive.plan.overlays.iter().zip(&archive.overlays) {
                    place_capture(capture, &io, &entry.destination, 0o755, scope, active)?;
                }
                for capture in &archive.patches {
                    let _buffer = scope.reserve_buffer(capture.frame().len() as usize)?;
                    let mut bytes = Vec::new();
                    bytes
                        .try_reserve_exact(capture.frame().len() as usize)
                        .map_err(|_| PhysicalRootError::Resource)?;
                    capture.read_chunks(scope, active, |chunk| {
                        bytes.extend_from_slice(chunk);
                        Ok(())
                    })?;
                    crate::runtime::repository_archive_patch::apply_selected_bcr_patch_at(
                        &io,
                        &bytes,
                        archive.plan.patch_strip,
                        &running,
                        Some(scope),
                    )
                    .map_err(PhysicalRootError::Mutation)?;
                }
                place_capture(&archive.module, &io, "MODULE.bazel", 0o644, scope, active)?;
                builder.capture(scope, active)
            }
        }?;
        if let Input::Archive(archive) = &mut self.input {
            let mut failure = None;
            for capture in std::iter::once(&mut archive.archive)
                .chain(archive.overlays.iter_mut())
                .chain(archive.patches.iter_mut())
                .chain(std::iter::once(&mut archive.module))
            {
                if let Err(error) = capture.close() {
                    if failure.is_none() {
                        failure = Some(error)
                    }
                }
            }
            if let Some(error) = failure {
                return Err(error);
            }
        }
        check(scope, active)?;
        Ok(captured)
    }
}
fn place_capture(
    capture: &Capture,
    io: &RootIo<'_>,
    path: &str,
    mode: u32,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<(), PhysicalRootError> {
    if let Some(parent) = Path::new(path).parent() {
        io.ensure_directory(parent, 0o755)?;
    }
    let mut output = io.open_write_regular(
        Path::new(path),
        crate::runtime::repository_root_io::WriteMode::ReplaceRegular,
    )?;
    let mut hash = Sha256::new();
    capture.read_chunks(scope, active, |chunk| {
        crate::runtime::repository_io::source_service::write_counted(
            &mut output,
            chunk,
            active,
            Some(scope),
        )?;
        hash.update(chunk);
        Ok(())
    })?;
    if <[u8; 32]>::from(hash.finalize()) != capture.digest {
        return Err(PhysicalRootError::Snapshot);
    }
    io.chmod_regular(&output, mode)?;
    output.flush()?;
    Ok(())
}
impl From<slug_bzlmod_v2::BuiltinBazelToolsSourceFileError> for PhysicalRootError {
    fn from(_: slug_bzlmod_v2::BuiltinBazelToolsSourceFileError) -> Self {
        Self::Snapshot
    }
}
