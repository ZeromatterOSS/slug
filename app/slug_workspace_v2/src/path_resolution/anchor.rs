//! Immutable resolution boundary, never native filesystem authority.

use super::*;

/// A selected materialization incarnation and its absolute display root.
///
/// This carries no IO or lifetime capability. The outside-DICE observer must
/// still authenticate the selected, live incarnation for every demand.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct MaterializationPathAnchor {
    namespace: PathObservationNamespace,
    root: NormalizedAbsolutePath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum MaterializationPathAnchorError {
    NotNonzeroMaterialization,
    RelativePath,
    EscapesRoot,
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
        Ok(Self { namespace, root })
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
    ) -> Result<NormalizedAbsolutePath, MaterializationPathAnchorError> {
        if target.is_absolute() {
            self.path(target)
        } else {
            let parent = physical_link
                .as_path()
                .parent()
                .ok_or(MaterializationPathAnchorError::EscapesRoot)?;
            self.path(&parent.join(target))
        }
    }
}

/// Boundary refusal stays separate from the existing unanchored error schema.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum AnchoredPathResolutionError {
    Resolution(PathResolutionError),
    EscapesAnchor {
        anchor: MaterializationPathAnchor,
        requested_path: NormalizedAbsolutePath,
    },
}

fn anchored_error(
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
