//! Immutable resolution boundary, never native filesystem authority.

use super::*;

/// A selected materialization incarnation and its absolute display root.
///
/// This carries no IO or lifetime capability. The outside-DICE observer must
/// still authenticate the selected, live incarnation for every demand.
#[derive(Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct MaterializationPathAnchor {
    namespace: PathObservationNamespace,
    root: NormalizedAbsolutePath,
    sources: Option<Arc<[MaterializationTerminalAnchor]>>,
}

/// A terminal, internal-only resolution boundary. It cannot contain another
/// source context and carries no native acquisition or lifetime authority.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct MaterializationTerminalAnchor {
    namespace: PathObservationNamespace,
    root: NormalizedAbsolutePath,
}
impl MaterializationTerminalAnchor {
    pub fn new(
        namespace: PathObservationNamespace,
        root: NormalizedAbsolutePath,
    ) -> Result<Self, MaterializationPathAnchorError> {
        let anchor = MaterializationPathAnchor::new(namespace, root)?;
        Ok(Self {
            namespace: anchor.namespace,
            root: anchor.root,
        })
    }
    pub const fn namespace(&self) -> PathObservationNamespace {
        self.namespace
    }
    pub fn root(&self) -> &NormalizedAbsolutePath {
        &self.root
    }
    pub fn as_anchor(&self) -> MaterializationPathAnchor {
        MaterializationPathAnchor {
            namespace: self.namespace,
            root: self.root.dupe(),
            sources: None,
        }
    }
}
impl fmt::Debug for MaterializationPathAnchor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MaterializationPathAnchor")
            .field("namespace", &self.namespace)
            .field("root", &self.root)
            .field("source_count", &self.source_anchors().len())
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum MaterializationPathAnchorError {
    NotNonzeroMaterialization,
    RelativePath,
    EscapesRoot,
    InvalidSourceContext,
}

impl fmt::Display for MaterializationPathAnchorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid materialization path anchor or demand: {self:?}")
    }
}

impl std::error::Error for MaterializationPathAnchorError {}

impl MaterializationPathAnchor {
    pub fn new(
        namespace: PathObservationNamespace,
        root: NormalizedAbsolutePath,
    ) -> Result<Self, MaterializationPathAnchorError> {
        if !matches!(namespace, PathObservationNamespace::Materialization(instance) if instance.value() != 0)
        {
            return Err(MaterializationPathAnchorError::NotNonzeroMaterialization);
        }
        Ok(Self {
            namespace,
            root,
            sources: None,
        })
    }

    /// Consume a separately prepaid terminal array. Sorting canonicalizes
    /// order; duplicate or overlapping identities are refused, never deduped.
    pub fn with_source_anchors(
        mut self,
        mut sources: Vec<MaterializationTerminalAnchor>,
    ) -> Result<Self, MaterializationPathAnchorError> {
        if self.sources.is_some() || sources.len() > 128 {
            return Err(MaterializationPathAnchorError::InvalidSourceContext);
        }
        sources.sort_by(|a, b| a.root.cmp(&b.root));
        for (index, source) in sources.iter().enumerate() {
            if source.namespace == self.namespace
                || source.root.as_path().starts_with(self.root.as_path())
                || self.root.as_path().starts_with(source.root.as_path())
                || sources[..index].iter().any(|previous| {
                    previous.namespace == source.namespace
                        || source.root.as_path().starts_with(previous.root.as_path())
                        || previous.root.as_path().starts_with(source.root.as_path())
                })
            {
                return Err(MaterializationPathAnchorError::InvalidSourceContext);
            }
        }
        if !sources.is_empty() {
            self.sources = Some(sources.into());
        }
        Ok(self)
    }

    pub fn source_anchors(&self) -> &[MaterializationTerminalAnchor] {
        self.sources.as_deref().unwrap_or(&[])
    }

    pub const fn namespace(&self) -> PathObservationNamespace {
        self.namespace
    }

    pub fn root(&self) -> &NormalizedAbsolutePath {
        &self.root
    }

    // Check the raw traversal before normalization can erase an escape and
    // reentry. Component-aware prefix matching also rejects sibling spellings.
    fn path(&self, path: &Path) -> Result<NormalizedAbsolutePath, MaterializationPathAnchorError> {
        if !path.is_absolute() {
            return Err(MaterializationPathAnchorError::RelativePath);
        }
        let suffix = path
            .strip_prefix(self.root.as_path())
            .map_err(|_| MaterializationPathAnchorError::EscapesRoot)?;
        let mut depth = 0usize;
        for component in suffix.components() {
            match component {
                Component::Normal(_) => depth += 1,
                Component::CurDir => {}
                Component::ParentDir => {
                    depth = depth
                        .checked_sub(1)
                        .ok_or(MaterializationPathAnchorError::EscapesRoot)?;
                }
                Component::Prefix(_) | Component::RootDir => {
                    return Err(MaterializationPathAnchorError::EscapesRoot);
                }
            }
        }
        Ok(
            NormalizedAbsolutePath::new(path.to_path_buf())
                .expect("checked absolute anchored path"),
        )
    }

    pub(super) fn link_target(
        &self,
        physical_link: &NormalizedAbsolutePath,
        target: &Path,
    ) -> Result<(NormalizedAbsolutePath, Self), MaterializationPathAnchorError> {
        if target.is_absolute() {
            if target.starts_with(self.root.as_path()) {
                return self.path(target).map(|path| (path, self.dupe()));
            }
            // External targets are admitted only in their canonical spelling.
            // Never erase a missing intermediate component or an escape.
            let normalized = NormalizedAbsolutePath::new(target.to_path_buf())
                .map_err(|_| MaterializationPathAnchorError::EscapesRoot)?;
            if normalized.as_path().as_os_str() != target.as_os_str() {
                return Err(MaterializationPathAnchorError::EscapesRoot);
            }
            self.source_anchors()
                .iter()
                .find(|source| target.starts_with(source.root.as_path()))
                .ok_or(MaterializationPathAnchorError::EscapesRoot)
                .and_then(|source| {
                    let anchor = source.as_anchor();
                    anchor.path(target).map(|path| (path, anchor))
                })
        } else {
            let parent = physical_link
                .as_path()
                .parent()
                .ok_or(MaterializationPathAnchorError::EscapesRoot)?;
            self.path(&parent.join(target))
                .map(|path| (path, self.dupe()))
        }
    }
}

/// Boundary refusal stays separate from the existing unanchored error schema.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum AnchoredPathResolutionError {
    /// Synchronous guarded adapter could not allocate its prepaid work array.
    WorkingStorageRefused,
    Resolution(PathResolutionError),
    EscapesAnchor {
        anchor: MaterializationPathAnchor,
        requested_path: NormalizedAbsolutePath,
    },
}

pub(super) fn anchored_error(
    anchor: &MaterializationPathAnchor,
    requested_path: &NormalizedAbsolutePath,
    cause: ResolutionCause,
) -> AnchoredPathResolutionError {
    match cause {
        ResolutionCause::AnchorEscape => AnchoredPathResolutionError::EscapesAnchor {
            anchor: anchor.dupe(),
            requested_path: requested_path.dupe(),
        },
        cause => AnchoredPathResolutionError::Resolution(
            cause.with_outer_context(anchor.namespace(), requested_path.dupe()),
        ),
    }
}

/// Operational resolution whose ancestor walk stops at the actual root demand.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct AnchoredResolvedPathKey {
    anchor: MaterializationPathAnchor,
    logical_path: NormalizedAbsolutePath,
}

impl AnchoredResolvedPathKey {
    pub fn new(
        anchor: MaterializationPathAnchor,
        logical_path: impl AsRef<Path>,
    ) -> Result<Self, MaterializationPathAnchorError> {
        let logical_path = anchor.path(logical_path.as_ref())?;
        Ok(Self {
            anchor,
            logical_path,
        })
    }

    pub fn anchor(&self) -> &MaterializationPathAnchor {
        &self.anchor
    }

    pub fn logical_path(&self) -> &NormalizedAbsolutePath {
        &self.logical_path
    }

    /// Drive the same resolution machine without DICE. Modeled callers retain
    /// only the computed result; native callers must supply actual FD facts.
    /// This adapter publishes no observation epoch or filesystem authority.
    /// Guard allocation/work checkpoints independently of filesystem demands.
    /// The caller supplies a prepaid provider and retains peak credit through
    /// the returned value. This callback is allocation-free numeric metadata.
    pub fn resolve_sync_with_work<E>(
        &self,
        observe: impl FnMut(&PathObservationDemand) -> Result<PathObservationResult, E>,
        before_work: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Result<ResolvedPath, AnchoredPathResolutionError>, E> {
        work::resolve(self, observe, before_work)
    }

    pub fn resolve_sync<E>(
        &self,
        mut observe: impl FnMut(&PathObservationDemand) -> Result<PathObservationResult, E>,
    ) -> Result<Result<ResolvedPath, AnchoredPathResolutionError>, E> {
        let mut machine = ResolutionMachine::new(self.anchor.namespace(), self.logical_path.dupe());
        machine.set_anchor(Some(self.anchor.dupe()));
        let mut next = None;
        loop {
            let step = next.take().unwrap_or_else(|| machine.transition());
            match step {
                MachineStep::PushParent(parent) => machine.push_parent(parent),
                MachineStep::Observe(demand) => {
                    let result = observe(&demand)?;
                    assert_eq!(
                        demand.operation(),
                        result.operation(),
                        "resolver provider operation"
                    );
                    next = machine.observe(demand, Arc::new(result));
                }
                MachineStep::Complete(result) => {
                    if let Some(step) = machine.finish_frame(result) {
                        if machine.frames.is_empty() {
                            let MachineStep::Complete(result) = step else {
                                unreachable!()
                            };
                            return Ok(result
                                .map(|mut value| {
                                    value.anchor = Some(self.anchor.dupe());
                                    value
                                })
                                .map_err(|cause| {
                                    anchored_error(&self.anchor, &self.logical_path, cause)
                                }));
                        }
                        next = Some(step);
                    }
                }
            }
        }
    }
}

impl fmt::Display for AnchoredResolvedPathKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "anchored-resolved-path:{:?}:{:?}",
            self.anchor, self.logical_path
        )
    }
}

#[async_trait]
impl Key for AnchoredResolvedPathKey {
    type Value = PathResult<ResolvedPath, AnchoredPathResolutionError>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_resolution(
            ctx,
            self.anchor.namespace(),
            self.logical_path.dupe(),
            Some(self.anchor.dupe()),
            None,
        )
        .await
        .expect("PathObservationKey result operation matches its demand")
        .map(|result| {
            result.map_err(|cause| anchored_error(&self.anchor, &self.logical_path, cause))
        })
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedAnchoredResolvedPath {
    result: Result<ResolvedPath, AnchoredPathResolutionError>,
    observations: PathObservationEpoch,
}

impl ObservedAnchoredResolvedPath {
    pub fn result(&self) -> &Result<ResolvedPath, AnchoredPathResolutionError> {
        &self.result
    }
    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

/// The observed sibling retains all real observations, including error routes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct AnchoredResolvedPathObservationKey(AnchoredResolvedPathKey);

impl AnchoredResolvedPathObservationKey {
    pub fn new(
        anchor: MaterializationPathAnchor,
        logical_path: impl AsRef<Path>,
    ) -> Result<Self, MaterializationPathAnchorError> {
        AnchoredResolvedPathKey::new(anchor, logical_path).map(Self)
    }
}

impl fmt::Display for AnchoredResolvedPathObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[async_trait]
impl Key for AnchoredResolvedPathObservationKey {
    type Value = PathOutcome<Result<ObservedAnchoredResolvedPath, ObservedPathFrontierError>>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let mut captured = Vec::new();
        match drive_resolution(
            ctx,
            self.0.anchor.namespace(),
            self.0.logical_path.dupe(),
            Some(self.0.anchor.dupe()),
            Some(&mut captured),
        )
        .await
        {
            Err(error) => PathOutcome::Complete(Err(error.into())),
            Ok(PathOutcome::Need(need)) => PathOutcome::Need(need),
            Ok(PathOutcome::Complete(result)) => PathOutcome::Complete(
                PathObservationEpoch::from_shared(captured)
                    .map(|observations| ObservedAnchoredResolvedPath {
                        result: result.map_err(|cause| {
                            anchored_error(&self.0.anchor, &self.0.logical_path, cause)
                        }),
                        observations,
                    })
                    .map_err(Into::into),
            ),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}
