//! Closed, data-only Source targets admitted by the current prefix owner.
//! This is not a registry or a path capability. The actor authenticates the
//! complete current associations before minting and rechecks after native IO.
use compact_str::CompactString;
use slug_bzlmod_v2::RepositoryWorkspaceReadView;

use super::*;

#[derive(Debug)]
pub(in crate::runtime) struct SourceLinkSet {
    dependencies: Arc<NativeReadViewDependencies>,
    resources: Arc<RepositoryRootResources>,
    _credit: ResourceCredit,
}
pub(super) struct ClassifiedLink<'a> {
    pub(super) source: Option<&'a Arc<NativeReadViewData>>,
    pub(super) relative: CompactString,
    _credit: ResourceCredit,
}
pub(in crate::runtime) struct ValidatedAbsoluteLink<'a> {
    literal: &'a std::path::Path,
    _classification: ClassifiedLink<'a>,
}
impl ValidatedAbsoluteLink<'_> {
    pub(in crate::runtime) fn literal(&self) -> &std::path::Path {
        self.literal
    }
}
impl SourceLinkSet {
    /// Caller has authenticated every owner against the complete current
    /// declared Source set, outside DICE and before any effect. Scope and bank
    /// checks here cannot replace that association admission.
    pub(in crate::runtime) fn new(
        owners: &[Arc<NativeReadViewData>],
        resources: &Arc<RepositoryRootResources>,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Arc<Self>, PhysicalRootError> {
        check(scope, active)?;
        check_bank(scope, resources)?;
        if owners.len() > 128 {
            return Err(PhysicalRootError::Resource);
        }
        let credit = resources.reserve(
            add(
                arc_bytes::<Self>()?,
                owners.len() * 2 * std::mem::size_of::<Arc<NativeReadViewData>>(),
            )?,
            0,
            0,
        )?;
        for (index, owner) in owners.iter().enumerate() {
            let root = owner.source_root().ok_or(PhysicalRootError::Reservation)?;
            root.check_origin(scope, active)?;
            if !Arc::ptr_eq(&root.resources, resources)
                || owners[..index].iter().any(|old| {
                    old.projection().instance() == owner.projection().instance()
                        || old
                            .projection()
                            .display_root()
                            .as_path()
                            .starts_with(owner.projection().display_root().as_path())
                        || owner
                            .projection()
                            .display_root()
                            .as_path()
                            .starts_with(old.projection().display_root().as_path())
                })
            {
                return Err(PhysicalRootError::Reservation);
            }
        }
        let dependencies =
            NativeReadViewDependencies::from_owners(owners.to_vec(), 128, resources)?;
        Ok(Arc::new(Self {
            dependencies,
            resources: resources.clone(),
            _credit: credit,
        }))
    }
    pub(super) fn empty(
        resources: &Arc<RepositoryRootResources>,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Arc<Self>, PhysicalRootError> {
        Self::new(&[], resources, scope, active)
    }
    pub(super) fn from_views(
        dependencies: &NativeReadViewDependencies,
        views: &[RepositoryWorkspaceReadView],
        resources: &Arc<RepositoryRootResources>,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<Arc<Self>, PhysicalRootError> {
        let _peak = resources.reserve(
            add(
                256,
                views
                    .len()
                    .checked_mul(std::mem::size_of::<Arc<NativeReadViewData>>())
                    .ok_or(PhysicalRootError::Resource)?,
            )?,
            0,
            0,
        )?;
        if views.len() > 128 {
            return Err(PhysicalRootError::Resource);
        }
        let mut owners = Vec::with_capacity(views.len());
        for view in views.iter().filter(|view| view.source_state().is_some()) {
            let owner = dependencies
                .owners()
                .iter()
                .find(|owner| owner.covers(view))
                .ok_or(PhysicalRootError::Reservation)?;
            if !owners.iter().any(|old| Arc::ptr_eq(old, owner)) {
                owners.push(owner.clone());
            }
        }
        Self::new(&owners, resources, scope, active)
    }
    pub(super) fn dependencies(&self) -> &Arc<NativeReadViewDependencies> {
        &self.dependencies
    }
    pub(super) fn canonicalize_request(
        &self,
        request: slug_bzlmod_v2::RepositoryWorkspaceRequest,
        scope: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<slug_bzlmod_v2::RepositoryWorkspaceRequest, PhysicalRootError> {
        let slug_bzlmod_v2::RepositoryWorkspaceOperation::Symlink {
            path,
            target,
            source,
        } = request.operation()
        else {
            return Ok(request);
        };
        let classified = self.classify(
            request.reservation(),
            path,
            target
                .as_path()
                .to_str()
                .ok_or(PhysicalRootError::Snapshot)?,
        )?;
        let canonical = match (source, classified.source) {
            (None, None) => return Ok(request),
            (Some(raw), Some(owner)) => {
                let raw = raw.source_state().ok_or(PhysicalRootError::Reservation)?;
                let canonical = owner
                    .projection()
                    .source_state()
                    .ok_or(PhysicalRootError::Reservation)?;
                if !ViewComparison::new(&self.resources, scope, active)?
                    .source_state_equal(raw, canonical)?
                {
                    return Err(PhysicalRootError::Reservation);
                }
                owner.projection().clone()
            }
            _ => return Err(PhysicalRootError::Reservation),
        };
        request
            .with_symlink_source(canonical)
            .map_err(PhysicalRootError::Protocol)
    }
    pub(super) fn validate_absolute<'a>(
        &'a self,
        reservation: &RepositoryWorkspaceReservation,
        path: &str,
        target: &'a slug_workspace_v2::NormalizedAbsolutePath,
    ) -> Result<ValidatedAbsoluteLink<'a>, PhysicalRootError> {
        let literal = target.as_path();
        let text = literal.to_str().ok_or(PhysicalRootError::Snapshot)?;
        Ok(ValidatedAbsoluteLink {
            literal,
            _classification: self.classify(reservation, path, text)?,
        })
    }
    pub(super) fn source(
        &self,
        namespace: PathObservationNamespace,
    ) -> Result<&Arc<source::RepositorySourceRoot>, PhysicalRootError> {
        self.dependencies
            .owners()
            .iter()
            .filter_map(|owner| owner.source_root())
            .find(|root| root.anchor().namespace() == namespace)
            .ok_or(PhysicalRootError::Reservation)
    }
    pub(super) fn classify<'a>(
        &'a self,
        reservation: &RepositoryWorkspaceReservation,
        link_path: &str,
        literal: &str,
    ) -> Result<ClassifiedLink<'a>, PhysicalRootError> {
        let credit = self.resources.reserve(2 * 4096 + 256, 0, 0)?;
        if literal.is_empty() || literal.len() > 4096 || literal.contains(['\0', '\\']) {
            return Err(PhysicalRootError::Snapshot);
        }
        let own = reservation.display_root().as_path();
        let (source, prefix, tail) = if literal.starts_with('/') {
            let path = std::path::Path::new(literal);
            if let Ok(tail) = path.strip_prefix(own) {
                (None, "", tail.to_str().ok_or(PhysicalRootError::Snapshot)?)
            } else {
                if literal[1..]
                    .split('/')
                    .any(|part| part.is_empty() || part == "." || part == "..")
                {
                    return Err(PhysicalRootError::Snapshot);
                }
                let owner = self
                    .dependencies
                    .owners()
                    .iter()
                    .find(|owner| path.starts_with(owner.projection().display_root().as_path()))
                    .ok_or(PhysicalRootError::Snapshot)?;
                let tail = path
                    .strip_prefix(owner.projection().display_root().as_path())
                    .map_err(|_| PhysicalRootError::Snapshot)?
                    .to_str()
                    .ok_or(PhysicalRootError::Snapshot)?;
                (Some(owner), "", tail)
            }
        } else {
            (
                None,
                link_path.rsplit_once('/').map_or("", |(parent, _)| parent),
                literal,
            )
        };
        let mut buffer = [0u8; 4096];
        let mut length = 0usize;
        for part in prefix.split('/').chain(tail.split('/')) {
            match part {
                "" | "." => (),
                ".." => {
                    if length == 0 {
                        return Err(PhysicalRootError::Snapshot);
                    }
                    length = buffer[..length]
                        .iter()
                        .rposition(|b| *b == b'/')
                        .unwrap_or(0);
                }
                name if name.len() <= 255 => {
                    let end = add(add(length, usize::from(length != 0))?, name.len())?;
                    if end > buffer.len() {
                        return Err(PhysicalRootError::Snapshot);
                    }
                    if length != 0 {
                        buffer[length] = b'/';
                        length += 1;
                    }
                    buffer[length..end].copy_from_slice(name.as_bytes());
                    length = end;
                }
                _ => return Err(PhysicalRootError::Snapshot),
            }
        }
        let relative = CompactString::from(
            std::str::from_utf8(&buffer[..length]).map_err(|_| PhysicalRootError::Snapshot)?,
        );
        Ok(ClassifiedLink {
            source,
            relative,
            _credit: credit,
        })
    }
}
