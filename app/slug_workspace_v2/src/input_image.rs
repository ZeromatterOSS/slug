//! Closed observed source-tree descriptors. No filesystem IO occurs in this key.

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;

use crate::FileContentDigest;
use crate::NeedPathObservations;
use crate::NormalizedAbsolutePath;
use crate::PathLstat;
use crate::PathNodeKind;
use crate::PathObservationDemand;
use crate::PathObservationEpoch;
use crate::PathObservationKey;
use crate::PathObservationNamespace;
use crate::PathObservationOperation;
use crate::PathObservationResult;
use crate::PathOperationResult;
use crate::PathOutcome;

pub mod budget;
mod observation;
pub use budget::BudgetedImageBlock;
pub use budget::ImageChargedPath;
pub use budget::ImageDescriptorBudget;
pub use budget::ImageDescriptorBudgetError;
pub use budget::ImageDescriptorReservation;
pub use observation::DirectoryObservationLimits;
pub use observation::ImageObservationData;
pub use observation::ImageObservationOperation;
pub use observation::ImageObservationRequest;
pub use observation::ImageObservationResult;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
pub enum InputImageAliasPolicy {
    Deny,
    RuntimeContained,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct InputImageLimits {
    pub max_entries: usize,
    pub max_depth: usize,
    pub max_total_file_bytes: u64,
    pub max_file_bytes: u64,
    pub max_descriptor_bytes: usize,
    pub max_path_bytes: usize,
    pub max_alias_hops: usize,
}

impl Default for InputImageLimits {
    fn default() -> Self {
        Self {
            max_entries: 32_768,
            max_depth: 32,
            max_total_file_bytes: 2 * 1024 * 1024 * 1024,
            max_file_bytes: 512 * 1024 * 1024,
            max_descriptor_bytes: 4 * 1024 * 1024,
            max_path_bytes: 4096,
            max_alias_hops: 32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct InputImagePolicy {
    aliases: InputImageAliasPolicy,
    limits: InputImageLimits,
}

impl InputImagePolicy {
    pub fn new(
        aliases: InputImageAliasPolicy,
        limits: InputImageLimits,
    ) -> Result<Self, InputImageError> {
        let ceiling = InputImageLimits::default();
        if limits.max_entries == 0
            || limits.max_entries > ceiling.max_entries
            || limits.max_depth > ceiling.max_depth
            || limits.max_total_file_bytes > ceiling.max_total_file_bytes
            || limits.max_file_bytes > ceiling.max_file_bytes
            || limits.max_descriptor_bytes > ceiling.max_descriptor_bytes
            || limits.max_path_bytes == 0
            || limits.max_path_bytes > ceiling.max_path_bytes
            || limits.max_alias_hops == 0
            || limits.max_alias_hops > ceiling.max_alias_hops
        {
            return Err(InputImageError::InvalidScope);
        }
        Ok(Self { aliases, limits })
    }
    pub fn plain() -> Self {
        Self {
            aliases: InputImageAliasPolicy::Deny,
            limits: InputImageLimits::default(),
        }
    }
    pub fn runtime_aliases() -> Self {
        Self {
            aliases: InputImageAliasPolicy::RuntimeContained,
            limits: InputImageLimits::default(),
        }
    }
    pub fn aliases(&self) -> InputImageAliasPolicy {
        self.aliases
    }
    pub fn limits(&self) -> &InputImageLimits {
        &self.limits
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum InputImageLimit {
    Entries,
    Depth,
    FileBytes,
    TotalFileBytes,
    DescriptorBytes,
    PathBytes,
    AliasHops,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum InputImageError {
    Budget(ImageDescriptorBudgetError),
    MissingBudget,
    Compute,
    InvalidScope,
    Missing,
    WrongKind,
    UnsupportedMode,
    LimitExceeded(InputImageLimit),
    ObservationMismatch,
    Observation(crate::PathObservationError),
    DigestSizeMismatch,
    InvalidAlias,
    AliasCycle,
}

impl InputImageError {
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::Budget(_)
                | Self::MissingBudget
                | Self::Compute
                | Self::Observation(crate::PathObservationError::InconsistentState { .. })
        )
    }
}
impl From<ImageDescriptorBudgetError> for InputImageError {
    fn from(value: ImageDescriptorBudgetError) -> Self {
        Self::Budget(value)
    }
}
impl fmt::Display for InputImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed input image: {self:?}")
    }
}
impl std::error::Error for InputImageError {}

pub type InputImageSpecError = InputImageError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct ObservedInputImageKey {
    namespace: PathObservationNamespace,
    anchor_root: ImageChargedPath,
    source_root: ImageChargedPath,
    policy: InputImagePolicy,
}

impl ObservedInputImageKey {
    pub fn new(
        namespace: PathObservationNamespace,
        anchor_root: NormalizedAbsolutePath,
        subtree_root: NormalizedAbsolutePath,
        policy: InputImagePolicy,
        budget: &ImageDescriptorBudget,
    ) -> Result<Self, InputImageSpecError> {
        Self::from_charged(
            namespace,
            ImageChargedPath::pin_existing(anchor_root, budget)?,
            ImageChargedPath::pin_existing(subtree_root, budget)?,
            policy,
        )
    }
    pub fn from_charged(
        namespace: PathObservationNamespace,
        anchor_root: ImageChargedPath,
        subtree_root: ImageChargedPath,
        policy: InputImagePolicy,
    ) -> Result<Self, InputImageSpecError> {
        if !subtree_root.same_budget(anchor_root.budget()) {
            return Err(ImageDescriptorBudgetError::ForeignBudget.into());
        }
        if !subtree_root.as_path().starts_with(anchor_root.as_path()) {
            return Err(InputImageError::InvalidScope);
        }
        Ok(Self {
            namespace,
            anchor_root,
            source_root: subtree_root,
            policy,
        })
    }
    pub fn namespace(&self) -> PathObservationNamespace {
        self.namespace
    }
    pub fn anchor_root(&self) -> &NormalizedAbsolutePath {
        self.anchor_root.normalized_ref()
    }
    pub fn source_root(&self) -> &NormalizedAbsolutePath {
        self.source_root.normalized_ref()
    }
    pub fn policy(&self) -> &InputImagePolicy {
        &self.policy
    }
}

impl fmt::Display for ObservedInputImageKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "observed-input-image:{:?}:{}:{}:{:?}",
            self.namespace,
            self.anchor_root(),
            self.source_root(),
            self.policy
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum InputImageEntryKind {
    Directory,
    RegularFile(FileContentDigest),
    Symlink,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct InputImageEntry {
    path: ImageChargedPath,
    kind: InputImageEntryKind,
    mode: u32,
    link: Option<ImageObservationResult>,
    resolved_alias: Option<ImageChargedPath>,
}

impl InputImageEntry {
    pub fn path(&self) -> &ImageChargedPath {
        &self.path
    }
    pub fn kind(&self) -> InputImageEntryKind {
        self.kind
    }
    pub fn mode(&self) -> u32 {
        self.mode
    }
    pub fn file_digest(&self) -> Option<FileContentDigest> {
        match self.kind {
            InputImageEntryKind::RegularFile(digest) => Some(digest),
            _ => None,
        }
    }
    pub fn link_target(&self) -> Option<&Path> {
        match self.link.as_ref()?.data() {
            ImageObservationData::ReadLink(PathOperationResult::Present(target)) => {
                Some(target.as_path())
            }
            _ => None,
        }
    }
    pub fn resolved_alias_path(&self) -> Option<&ImageChargedPath> {
        self.resolved_alias.as_ref()
    }
}

#[derive(Debug, PartialEq, Eq, Allocative)]
struct InputImageCertificate {
    spec: ObservedInputImageKey,
    entries: Vec<InputImageEntry>,
    observations: PathObservationEpoch,
    total_file_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedInputImage(BudgetedImageBlock<InputImageCertificate>);

impl ObservedInputImage {
    pub fn namespace(&self) -> PathObservationNamespace {
        self.0.get().spec.namespace()
    }
    pub fn source_namespace(&self) -> PathObservationNamespace {
        self.namespace()
    }
    pub fn anchor_root(&self) -> &NormalizedAbsolutePath {
        self.0.get().spec.anchor_root()
    }
    pub fn source_root(&self) -> &NormalizedAbsolutePath {
        self.0.get().spec.source_root()
    }
    pub fn policy(&self) -> &InputImagePolicy {
        self.0.get().spec.policy()
    }
    pub fn entries(&self) -> &[InputImageEntry] {
        &self.0.get().entries
    }
    pub fn observations(&self) -> &PathObservationEpoch {
        &self.0.get().observations
    }
    pub fn total_file_bytes(&self) -> u64 {
        self.0.get().total_file_bytes
    }
    pub fn same_budget(&self, budget: &ImageDescriptorBudget) -> bool {
        self.0.same_budget(budget)
    }
    pub fn content(&self) -> InputImageContent {
        InputImageContent(self.dupe())
    }
}

#[derive(Debug, Clone, Allocative, Dupe)]
pub struct InputImageContent(ObservedInputImage);
impl PartialEq for InputImageContent {
    fn eq(&self, other: &Self) -> bool {
        let left = self.0.0.get();
        let right = other.0.0.get();
        left.spec == right.spec
            && left.entries == right.entries
            && left.total_file_bytes == right.total_file_bytes
    }
}
impl Eq for InputImageContent {}

#[async_trait]
impl Key for ObservedInputImageKey {
    type Value = PathOutcome<Result<ObservedInputImage, InputImageError>>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        compute_image(self, ctx).await
    }
    fn equality(left: &Self::Value, right: &Self::Value) -> bool {
        Self::validity(left) && Self::validity(right) && left.complete_eq(right)
    }
    fn validity(value: &Self::Value) -> bool {
        match value {
            PathOutcome::Complete(Ok(_)) => true,
            PathOutcome::Complete(Err(error)) => !error.is_transient(),
            PathOutcome::Need(_) => false,
        }
    }
}

type ImageValue = PathOutcome<Result<ObservedInputImage, InputImageError>>;
type ImageStep<T> = Result<T, ImageValue>;
fn failed(error: InputImageError) -> ImageValue {
    PathOutcome::Complete(Err(error))
}

struct ImageBuilder<'key, 'ctx, 'dice> {
    key: &'key ObservedInputImageKey,
    ctx: &'ctx mut DiceComputations<'dice>,
    budget: ImageDescriptorBudget,
    entries: Vec<InputImageEntry>,
    frontier: Vec<(PathObservationDemand, Arc<PathObservationResult>)>,
    descriptor_bytes: usize,
    discovered_entries: usize,
    total_file_bytes: u64,
    needs: Vec<PathObservationDemand>,
    need_reservation: ImageDescriptorReservation,
    reservation: ImageDescriptorReservation,
}

impl ImageBuilder<'_, '_, '_> {
    async fn observe(
        &mut self,
        path: &ImageChargedPath,
        operation: ImageObservationOperation,
    ) -> ImageStep<ImageObservationResult> {
        let request = ImageObservationRequest::new(self.key.anchor_root.dupe(), operation);
        let demand =
            PathObservationDemand::input_image(self.key.namespace, path.dupe(), request.dupe())
                .map_err(|error| failed(error.into()))?;
        let value = self
            .ctx
            .compute(&PathObservationKey::new(demand.dupe()))
            .await
            .map_err(|_| failed(InputImageError::Compute))?;
        let raw = match value {
            PathOutcome::Need(need) => return Err(PathOutcome::Need(need)),
            PathOutcome::Complete(raw) => raw,
        };
        let PathObservationResult::InputImage(result) = raw.as_ref() else {
            return Err(failed(InputImageError::ObservationMismatch));
        };
        if !result.same_budget(&self.budget) {
            return Err(failed(ImageDescriptorBudgetError::ForeignBudget.into()));
        }
        if result.request() != &request || result.issued_path() != path {
            return Err(failed(InputImageError::ObservationMismatch));
        }
        let result = result.dupe();
        budget::reserve_vec_slot(&mut self.reservation, &mut self.frontier)
            .map_err(|error| failed(error.into()))?;
        self.frontier.push((demand, raw));
        if matches!(result.data(), ImageObservationData::LimitExceeded) {
            let limit = match operation {
                ImageObservationOperation::BoundedDigest { .. } => InputImageLimit::FileBytes,
                ImageObservationOperation::BoundedDirectory(_) => InputImageLimit::Entries,
                ImageObservationOperation::BoundedReadLink { .. } => InputImageLimit::PathBytes,
                ImageObservationOperation::Lstat => {
                    return Err(failed(InputImageError::ObservationMismatch));
                }
            };
            return Err(failed(InputImageError::LimitExceeded(limit)));
        }
        Ok(result)
    }

    async fn lstat(&mut self, path: &ImageChargedPath) -> ImageStep<PathLstat> {
        let result = self.observe(path, ImageObservationOperation::Lstat).await?;
        match result.data() {
            ImageObservationData::Lstat(PathOperationResult::Present(stat)) => Ok(*stat),
            ImageObservationData::Lstat(PathOperationResult::Missing) => {
                Err(failed(InputImageError::Missing))
            }
            ImageObservationData::Lstat(PathOperationResult::Error(error)) => {
                Err(failed(InputImageError::Observation(*error)))
            }
            _ => Err(failed(InputImageError::ObservationMismatch)),
        }
    }

    fn descriptor(&mut self, bytes: usize) -> ImageStep<()> {
        self.descriptor_bytes = self.descriptor_bytes.checked_add(bytes).ok_or_else(|| {
            failed(InputImageError::LimitExceeded(
                InputImageLimit::DescriptorBytes,
            ))
        })?;
        if self.descriptor_bytes > self.key.policy.limits.max_descriptor_bytes {
            return Err(failed(InputImageError::LimitExceeded(
                InputImageLimit::DescriptorBytes,
            )));
        }
        Ok(())
    }

    fn path_limit(&self, path: &ImageChargedPath) -> ImageStep<()> {
        if path.as_path().as_os_str().len() > self.key.policy.limits.max_path_bytes {
            Err(failed(InputImageError::LimitExceeded(
                InputImageLimit::PathBytes,
            )))
        } else {
            Ok(())
        }
    }

    async fn build(mut self) -> ImageValue {
        match self.walk().await {
            Ok(()) => {}
            Err(outcome) => return outcome,
        }
        if !self.needs.is_empty() {
            // Arc-slice conversion and sorting storage remain charged through
            // the first demand, including after this failed builder retires.
            let bytes = match self
                .needs
                .len()
                .checked_mul(std::mem::size_of::<PathObservationDemand>())
                .and_then(|n| n.checked_mul(3))
            {
                Some(bytes) => bytes,
                None => return failed(ImageDescriptorBudgetError::ChargeOverflow.into()),
            };
            if let Err(error) = self.need_reservation.try_grow(bytes) {
                return failed(error.into());
            }
            let first = &self.needs[0];
            let PathObservationOperation::InputImage(request) = first.operation() else {
                return failed(InputImageError::ObservationMismatch);
            };
            let path = match first
                .image_path()
                .expect("image-only needs")
                .retain_charge(self.need_reservation)
            {
                Ok(path) => path,
                Err(error) => return failed(error.into()),
            };
            self.needs[0] =
                match PathObservationDemand::input_image(self.key.namespace, path, request) {
                    Ok(demand) => demand,
                    Err(error) => return failed(error.into()),
                };
            return PathOutcome::Need(
                NeedPathObservations::try_from_iter(self.needs).expect("nonempty collected needs"),
            );
        }
        self.entries
            .sort_by(|left, right| left.path.cmp(&right.path));
        if let Err(error) = resolve_aliases(&mut self.entries, self.key, &mut self.reservation) {
            return failed(error);
        }
        // from_shared sorts a vector and constructs its compact immutable map.
        // Reserve both conversion arrays before it allocates either of them.
        let row_bytes = std::mem::size_of::<(PathObservationDemand, Arc<PathObservationResult>)>();
        let conversion = match row_bytes
            .checked_mul(self.frontier.len())
            .and_then(|n| n.checked_mul(3))
        {
            Some(bytes) => bytes,
            None => return failed(ImageDescriptorBudgetError::ChargeOverflow.into()),
        };
        if let Err(error) = self.reservation.try_grow(conversion) {
            return failed(error.into());
        }
        let observations = match PathObservationEpoch::from_shared(self.frontier) {
            Ok(epoch) => epoch,
            Err(_) => return failed(InputImageError::ObservationMismatch),
        };
        let certificate = InputImageCertificate {
            spec: self.key.dupe(),
            entries: self.entries,
            observations,
            total_file_bytes: self.total_file_bytes,
        };
        match self.reservation.freeze(certificate) {
            Ok(block) => PathOutcome::Complete(Ok(ObservedInputImage(block))),
            Err(error) => failed(error.into()),
        }
    }

    async fn walk(&mut self) -> ImageStep<()> {
        let key = self.key;
        let mut path = key.anchor_root.dupe();
        self.path_limit(&path)?;
        for component in key
            .source_root
            .as_path()
            .strip_prefix(key.anchor_root.as_path())
            .expect("validated subtree")
            .components()
        {
            if self.lstat(&path).await?.kind() != PathNodeKind::Directory {
                return Err(failed(InputImageError::WrongKind));
            }
            path = path
                .try_join_name(component.as_os_str())
                .map_err(|error| failed(error.into()))?;
            self.path_limit(&path)?;
        }
        let mut pending = Vec::new();
        budget::reserve_vec_slot(&mut self.reservation, &mut pending)
            .map_err(|error| failed(error.into()))?;
        self.descriptor(path.as_path().as_os_str().len())?;
        self.discovered_entries = 1;
        pending.push((path, 0usize));
        while let Some((path, depth)) = pending.pop() {
            match self.walk_node(&path, depth, &mut pending).await {
                Ok(()) => {}
                Err(PathOutcome::Need(need)) => {
                    for demand in need.demands() {
                        if self.needs.len() == 256 {
                            break;
                        }
                        budget::reserve_vec_slot(&mut self.need_reservation, &mut self.needs)
                            .map_err(|error| failed(error.into()))?;
                        self.needs.push(demand.dupe());
                    }
                    if self.needs.len() == 256 {
                        break;
                    }
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    async fn walk_node(
        &mut self,
        path: &ImageChargedPath,
        depth: usize,
        pending: &mut Vec<(ImageChargedPath, usize)>,
    ) -> ImageStep<()> {
        let key = self.key;
        if depth > key.policy.limits.max_depth {
            return Err(failed(InputImageError::LimitExceeded(
                InputImageLimit::Depth,
            )));
        }
        if self.entries.len() == key.policy.limits.max_entries {
            return Err(failed(InputImageError::LimitExceeded(
                InputImageLimit::Entries,
            )));
        }
        self.path_limit(&path)?;
        let stat = self.lstat(&path).await?;
        let mode = u32::try_from(stat.permissions())
            .map_err(|_| failed(InputImageError::UnsupportedMode))?;
        if mode > 0o7777 {
            return Err(failed(InputImageError::UnsupportedMode));
        }
        let mut entry = InputImageEntry {
            path: path.dupe(),
            kind: InputImageEntryKind::Directory,
            mode,
            link: None,
            resolved_alias: None,
        };
        match stat.kind() {
            PathNodeKind::Directory => {
                let result = self
                    .observe(
                        &path,
                        ImageObservationOperation::BoundedDirectory(DirectoryObservationLimits {
                            max_entries: key.policy.limits.max_entries,
                            max_name_bytes_total: key.policy.limits.max_descriptor_bytes,
                            max_component_bytes: key.policy.limits.max_path_bytes,
                        }),
                    )
                    .await?;
                let rows = match result.data() {
                    ImageObservationData::Directory(PathOperationResult::Present(rows)) => rows,
                    ImageObservationData::Directory(PathOperationResult::Missing) => {
                        return Err(failed(InputImageError::Missing));
                    }
                    ImageObservationData::Directory(PathOperationResult::Error(error)) => {
                        return Err(failed(InputImageError::Observation(*error)));
                    }
                    _ => return Err(failed(InputImageError::ObservationMismatch)),
                };
                // Known membership includes unfinished siblings already popped
                // into Need. Count each discovered child once before joining.
                self.discovered_entries = self
                    .discovered_entries
                    .checked_add(rows.entries().len())
                    .ok_or_else(|| {
                        failed(InputImageError::LimitExceeded(InputImageLimit::Entries))
                    })?;
                if self.discovered_entries > key.policy.limits.max_entries {
                    return Err(failed(InputImageError::LimitExceeded(
                        InputImageLimit::Entries,
                    )));
                }
                if rows
                    .entries()
                    .windows(2)
                    .any(|pair| pair[0].name() == pair[1].name())
                {
                    return Err(failed(InputImageError::ObservationMismatch));
                }
                for row in rows.entries().iter().rev() {
                    let length = path
                        .as_path()
                        .as_os_str()
                        .len()
                        .checked_add(row.name().as_os_str().len())
                        .and_then(|n| n.checked_add(1))
                        .ok_or_else(|| {
                            failed(InputImageError::LimitExceeded(InputImageLimit::PathBytes))
                        })?;
                    if length > key.policy.limits.max_path_bytes {
                        return Err(failed(InputImageError::LimitExceeded(
                            InputImageLimit::PathBytes,
                        )));
                    }
                    if depth == key.policy.limits.max_depth {
                        return Err(failed(InputImageError::LimitExceeded(
                            InputImageLimit::Depth,
                        )));
                    }
                    self.descriptor(length)?;
                    let child = path
                        .try_join_component(row.name())
                        .map_err(|error| failed(error.into()))?;
                    budget::reserve_vec_slot(&mut self.reservation, pending)
                        .map_err(|error| failed(error.into()))?;
                    pending.push((child, depth + 1));
                }
            }
            PathNodeKind::RegularFile => {
                if depth == 0 {
                    return Err(failed(InputImageError::WrongKind));
                }
                let size = u64::try_from(stat.size())
                    .map_err(|_| failed(InputImageError::DigestSizeMismatch))?;
                if size > key.policy.limits.max_file_bytes {
                    return Err(failed(InputImageError::LimitExceeded(
                        InputImageLimit::FileBytes,
                    )));
                }
                self.total_file_bytes =
                    self.total_file_bytes.checked_add(size).ok_or_else(|| {
                        failed(InputImageError::LimitExceeded(
                            InputImageLimit::TotalFileBytes,
                        ))
                    })?;
                if self.total_file_bytes > key.policy.limits.max_total_file_bytes {
                    return Err(failed(InputImageError::LimitExceeded(
                        InputImageLimit::TotalFileBytes,
                    )));
                }
                let result = self
                    .observe(
                        &path,
                        ImageObservationOperation::BoundedDigest {
                            max_bytes: key.policy.limits.max_file_bytes,
                        },
                    )
                    .await?;
                let digest = match result.data() {
                    ImageObservationData::Digest(PathOperationResult::Present(digest))
                        if digest.size_bytes() == size =>
                    {
                        *digest
                    }
                    ImageObservationData::Digest(PathOperationResult::Present(_)) => {
                        return Err(failed(InputImageError::DigestSizeMismatch));
                    }
                    ImageObservationData::Digest(PathOperationResult::Missing) => {
                        return Err(failed(InputImageError::Missing));
                    }
                    ImageObservationData::Digest(PathOperationResult::Error(error)) => {
                        return Err(failed(InputImageError::Observation(*error)));
                    }
                    _ => return Err(failed(InputImageError::ObservationMismatch)),
                };
                entry.kind = InputImageEntryKind::RegularFile(digest);
            }
            PathNodeKind::Symlink => {
                if key.policy.aliases == InputImageAliasPolicy::Deny || depth == 0 {
                    return Err(failed(InputImageError::InvalidAlias));
                }
                let result = self
                    .observe(
                        &path,
                        ImageObservationOperation::BoundedReadLink {
                            max_target_bytes: key.policy.limits.max_path_bytes,
                        },
                    )
                    .await?;
                match result.data() {
                    ImageObservationData::ReadLink(PathOperationResult::Present(target)) => {
                        self.descriptor(target.as_os_str().len())?
                    }
                    ImageObservationData::ReadLink(PathOperationResult::Missing) => {
                        return Err(failed(InputImageError::Missing));
                    }
                    ImageObservationData::ReadLink(PathOperationResult::Error(error)) => {
                        return Err(failed(InputImageError::Observation(*error)));
                    }
                    _ => return Err(failed(InputImageError::ObservationMismatch)),
                }
                entry.kind = InputImageEntryKind::Symlink;
                entry.link = Some(result);
            }
            PathNodeKind::SpecialFile => return Err(failed(InputImageError::WrongKind)),
        }
        budget::reserve_vec_slot(&mut self.reservation, &mut self.entries)
            .map_err(|error| failed(error.into()))?;
        self.entries.push(entry);
        Ok(())
    }
}

async fn compute_image(key: &ObservedInputImageKey, ctx: &mut DiceComputations<'_>) -> ImageValue {
    let budget = match ctx
        .per_transaction_data()
        .data
        .get::<ImageDescriptorBudget>()
    {
        Ok(budget) => budget.dupe(),
        Err(_) => return failed(InputImageError::MissingBudget),
    };
    if !key.anchor_root.same_budget(&budget) || !key.source_root.same_budget(&budget) {
        return failed(ImageDescriptorBudgetError::ForeignBudget.into());
    }
    let reservation = match budget.try_reserve(128) {
        Ok(reservation) => reservation,
        Err(error) => return failed(error.into()),
    };
    let need_reservation = budget.try_reserve(0).expect("zero reservation");
    ImageBuilder {
        key,
        ctx,
        budget,
        entries: Vec::new(),
        frontier: Vec::new(),
        descriptor_bytes: 0,
        discovered_entries: 0,
        total_file_bytes: 0,
        needs: Vec::new(),
        need_reservation,
        reservation,
    }
    .build()
    .await
}

fn find_entry(entries: &[InputImageEntry], path: &Path) -> Option<usize> {
    entries
        .binary_search_by(|entry| entry.path.as_path().cmp(path))
        .ok()
}

fn resolve_aliases(
    entries: &mut [InputImageEntry],
    key: &ObservedInputImageKey,
    reservation: &mut ImageDescriptorReservation,
) -> Result<(), InputImageError> {
    for entry in entries
        .iter_mut()
        .filter(|entry| entry.kind == InputImageEntryKind::Symlink)
    {
        let target = entry.link_target().ok_or(InputImageError::InvalidAlias)?;
        // This first closed profile deliberately refuses parent components.
        // Normalizing `..` before intervening symlink expansion is not POSIX
        // resolution and can hide an escape from the certified subtree.
        if target
            .components()
            .any(|component| component == std::path::Component::ParentDir)
        {
            return Err(InputImageError::InvalidAlias);
        }
        // Compute normalized spelling length without allocating. Literal
        // bytes are already bounded by the scoped ReadLink request; only
        // normal components contribute to the retained target path.
        let mut length = if target.is_absolute() {
            1
        } else {
            entry
                .path
                .as_path()
                .parent()
                .ok_or(InputImageError::InvalidAlias)?
                .as_os_str()
                .len()
        };
        for component in target.components() {
            match component {
                std::path::Component::RootDir | std::path::Component::CurDir => {}
                std::path::Component::Normal(name) => {
                    length = length
                        .checked_add(usize::from(length != 1))
                        .and_then(|n| n.checked_add(name.len()))
                        .ok_or(InputImageError::InvalidAlias)?;
                }
                _ => return Err(InputImageError::InvalidAlias),
            }
        }
        if length > key.policy.limits.max_path_bytes {
            return Err(InputImageError::LimitExceeded(InputImageLimit::PathBytes));
        }
        let target = entry.path.try_resolve_alias(target)?;
        if !target.as_path().starts_with(key.source_root.as_path()) {
            return Err(InputImageError::InvalidAlias);
        }
        entry.resolved_alias = Some(target);
    }
    let count = entries.len();
    reservation.try_grow(
        count
            .checked_mul(std::mem::size_of::<Option<usize>>())
            .ok_or(ImageDescriptorBudgetError::ChargeOverflow)?,
    )?;
    let mut alias_targets = vec![None; count];
    for index in 0..count {
        if let Some(target) = &entries[index].resolved_alias {
            alias_targets[index] = Some(resolve_catalog_target(entries, target, key, index)?);
        }
    }
    // Directory child edges plus aliases must be acyclic (including a link to
    // an ancestor directory, which a link-only cycle check would miss).
    reservation.try_grow(
        count
            .checked_mul(std::mem::size_of::<Vec<usize>>() + 1)
            .ok_or(ImageDescriptorBudgetError::ChargeOverflow)?,
    )?;
    let mut edges = vec![Vec::new(); count];
    for (index, entry) in entries.iter().enumerate() {
        if let Some(parent) = entry
            .path
            .as_path()
            .parent()
            .and_then(|path| find_entry(entries, path))
        {
            budget::reserve_vec_slot(reservation, &mut edges[parent])?;
            edges[parent].push(index);
        }
        if let Some(target) = alias_targets[index] {
            budget::reserve_vec_slot(reservation, &mut edges[index])?;
            edges[index].push(target);
        }
    }
    let mut color = vec![0u8; count];
    let mut stack = Vec::new();
    for root in 0..count {
        if color[root] != 0 {
            continue;
        }
        budget::reserve_vec_slot(reservation, &mut stack)?;
        stack.push((root, 0usize));
        color[root] = 1;
        while let Some((node, next)) = stack.last_mut() {
            if *next == edges[*node].len() {
                color[*node] = 2;
                stack.pop();
                continue;
            }
            let child = edges[*node][*next];
            *next += 1;
            if color[child] == 1 {
                return Err(InputImageError::AliasCycle);
            }
            if color[child] == 0 {
                budget::reserve_vec_slot(reservation, &mut stack)?;
                color[child] = 1;
                stack.push((child, 0));
            }
        }
    }
    for index in 0..count {
        if let Some(target) = alias_targets[index] {
            entries[index].resolved_alias = Some(entries[target].path.dupe());
        }
    }
    Ok(())
}

fn resolve_catalog_target(
    entries: &[InputImageEntry],
    start: &ImageChargedPath,
    key: &ObservedInputImageKey,
    defining_alias: usize,
) -> Result<usize, InputImageError> {
    let mut target = start.dupe();
    // Include the defining alias; subsequent catalog expansion consumes one
    // additional hop. The accepted policy has at most 32 hops.
    let mut visited = [usize::MAX; 32];
    visited[0] = defining_alias;
    let mut hops = 1;
    loop {
        let mut cursor = key.source_root.dupe();
        let mut components = target
            .as_path()
            .strip_prefix(key.source_root.as_path())
            .map_err(|_| InputImageError::InvalidAlias)?
            .components();
        let mut expanded = None;
        let mut final_index =
            find_entry(entries, cursor.as_path()).ok_or(InputImageError::InvalidAlias)?;
        while let Some(component) = components.next() {
            cursor = cursor.try_join_name(component.as_os_str())?;
            final_index =
                find_entry(entries, cursor.as_path()).ok_or(InputImageError::InvalidAlias)?;
            if entries[final_index].kind == InputImageEntryKind::Symlink {
                if visited[..hops].contains(&final_index) {
                    return Err(InputImageError::AliasCycle);
                }
                if hops == key.policy.limits.max_alias_hops {
                    return Err(InputImageError::LimitExceeded(InputImageLimit::AliasHops));
                }
                visited[hops] = final_index;
                hops += 1;
                let mut replacement = entries[final_index]
                    .resolved_alias
                    .as_ref()
                    .ok_or(InputImageError::InvalidAlias)?
                    .dupe();
                for tail in components.by_ref() {
                    if replacement
                        .as_path()
                        .as_os_str()
                        .len()
                        .saturating_add(tail.as_os_str().len())
                        .saturating_add(1)
                        > key.policy.limits.max_path_bytes
                    {
                        return Err(InputImageError::LimitExceeded(InputImageLimit::PathBytes));
                    }
                    replacement = replacement.try_join_name(tail.as_os_str())?;
                }
                expanded = Some(replacement);
                break;
            }
            if !components.as_path().as_os_str().is_empty()
                && entries[final_index].kind != InputImageEntryKind::Directory
            {
                return Err(InputImageError::InvalidAlias);
            }
        }
        match expanded {
            Some(path) => target = path,
            None => return Ok(final_index),
        }
    }
}
