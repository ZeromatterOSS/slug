use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::ImageChargedPath;
use slug_workspace_v2::ImageDescriptorBudgetError;

use super::SelectedScratchResources;
use super::budget::PayloadReservation;
use crate::HostSelectedExtensionOwner;
use crate::RepositoryEnvironmentSnapshot;
use crate::SelectedExtensionDownloadVerified;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Allocative, Dupe)]
pub struct SelectedScratchLimits {
    pub max_operations: usize,
    pub max_entries: usize,
    pub max_depth: usize,
    pub max_path_bytes: usize,
    pub max_file_bytes: usize,
    pub max_descriptor_bytes: usize,
    pub max_occurrence_bytes: usize,
    pub max_argv_entries: usize,
    pub max_argv_bytes: usize,
    pub max_environment_entries: usize,
    pub max_environment_bytes: usize,
}
impl Default for SelectedScratchLimits {
    fn default() -> Self {
        Self {
            max_operations: 8192,
            max_entries: 4096,
            max_depth: 32,
            max_path_bytes: 4096,
            max_file_bytes: 32 << 20,
            max_descriptor_bytes: 4 << 20,
            max_occurrence_bytes: 128 << 20,
            max_argv_entries: 4096,
            max_argv_bytes: 256 << 10,
            max_environment_entries: 1024,
            max_environment_bytes: 256 << 10,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Allocative, Dupe)]
pub struct SelectedScratchPolicy(SelectedScratchLimits);
impl SelectedScratchPolicy {
    pub fn new(limits: SelectedScratchLimits) -> Result<Self, SelectedScratchError> {
        let ceiling = SelectedScratchLimits::default();
        if limits.max_operations == 0
            || limits.max_entries == 0
            || limits.max_depth == 0
            || limits.max_path_bytes < GUEST_ROOT.len()
            || limits.max_operations > ceiling.max_operations
            || limits.max_entries > ceiling.max_entries
            || limits.max_depth > ceiling.max_depth
            || limits.max_path_bytes > ceiling.max_path_bytes
            || limits.max_file_bytes > ceiling.max_file_bytes
            || limits.max_descriptor_bytes > ceiling.max_descriptor_bytes
            || limits.max_occurrence_bytes > ceiling.max_occurrence_bytes
            || limits.max_argv_entries == 0
            || limits.max_argv_entries > ceiling.max_argv_entries
            || limits.max_argv_bytes > ceiling.max_argv_bytes
            || limits.max_environment_entries > ceiling.max_environment_entries
            || limits.max_environment_bytes > ceiling.max_environment_bytes
        {
            return Err(SelectedScratchError::InvalidPolicy);
        }
        Ok(Self(limits))
    }
    pub fn plain() -> Self {
        Self(SelectedScratchLimits::default())
    }
    pub fn limits(&self) -> &SelectedScratchLimits {
        &self.0
    }
    pub fn guest_root(&self) -> &'static str {
        GUEST_ROOT
    }
}
impl Default for SelectedScratchPolicy {
    fn default() -> Self {
        Self::plain()
    }
}
pub(super) const GUEST_ROOT: &str = "/__slug_module";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Allocative, Dupe)]
pub enum SelectedScratchLimit {
    Operations,
    Entries,
    Depth,
    PathBytes,
    FileBytes,
    DescriptorBytes,
    OccurrenceBytes,
    ArgvEntries,
    ArgvBytes,
    EnvironmentEntries,
    EnvironmentBytes,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Allocative, Dupe)]
pub enum SelectedScratchUnsupported {
    ReadBeforeFirstProcess,
    UnknownOperation,
    DistinctDownloads,
    MissingFirstProcess,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Allocative, Dupe)]
pub enum SelectedScratchError {
    MissingResources,
    ForeignResources,
    ForeignOrigin,
    InvalidPolicy,
    InvalidPath,
    InvalidArgument,
    ScopeEscape,
    WrongKind,
    DownloadMismatch,
    LimitExceeded(SelectedScratchLimit),
    Unsupported(SelectedScratchUnsupported),
    PayloadResourceLimit,
    Descriptor(ImageDescriptorBudgetError),
    Allocation,
}
impl SelectedScratchError {
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::MissingResources
                | Self::ForeignResources
                | Self::PayloadResourceLimit
                | Self::Descriptor(_)
                | Self::Allocation
        )
    }
}
impl From<ImageDescriptorBudgetError> for SelectedScratchError {
    fn from(value: ImageDescriptorBudgetError) -> Self {
        Self::Descriptor(value)
    }
}
impl fmt::Display for SelectedScratchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "selected scratch: {self:?}")
    }
}
impl std::error::Error for SelectedScratchError {}

#[derive(Debug, Allocative)]
pub(super) struct Origin {
    pub workspace: ImageChargedPath,
    pub owner: Arc<HostSelectedExtensionOwner>,
    pub policy: SelectedScratchPolicy,
    pub resources: SelectedScratchResources,
}
impl PartialEq for Origin {
    fn eq(&self, rhs: &Self) -> bool {
        self.workspace == rhs.workspace && self.owner == rhs.owner && self.policy == rhs.policy
    }
}
impl Eq for Origin {}

#[derive(Clone, Eq, PartialEq, Allocative, Dupe)]
pub struct SelectedScratchText(pub(super) BudgetedImageBlock<String>);
impl SelectedScratchText {
    pub fn as_str(&self) -> &str {
        self.0.get()
    }
}
impl fmt::Debug for SelectedScratchText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SelectedScratchText")
            .field(&self.as_str().len())
            .finish()
    }
}

#[derive(Debug, Eq, PartialEq, Allocative)]
pub(super) struct PathInner {
    pub origin: BudgetedImageBlock<Origin>,
    pub relative: String,
    pub guest: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Allocative, Dupe)]
pub struct SelectedScratchPath(pub(super) BudgetedImageBlock<PathInner>);
impl SelectedScratchPath {
    pub fn relative(&self) -> &str {
        &self.0.get().relative
    }
    pub fn guest_spelling(&self) -> &str {
        &self.0.get().guest
    }
}

#[derive(Allocative)]
pub(super) enum BlobStorage {
    Owned(Vec<u8>),
    Download(Arc<SelectedExtensionDownloadVerified>),
}
#[derive(Allocative)]
pub(super) struct BlobInner {
    // Owned bytes/pin drop BEFORE releasing payload credit.
    pub storage: BlobStorage,
    pub payload: PayloadReservation,
}
impl BlobInner {
    fn bytes(&self) -> &[u8] {
        match &self.storage {
            BlobStorage::Owned(bytes) => bytes,
            BlobStorage::Download(v) => v.bytes(),
        }
    }
    fn download(&self) -> Option<&Arc<SelectedExtensionDownloadVerified>> {
        match &self.storage {
            BlobStorage::Download(v) => Some(v),
            _ => None,
        }
    }
}
impl PartialEq for BlobInner {
    fn eq(&self, rhs: &Self) -> bool {
        self.bytes() == rhs.bytes()
            && self.download().map(|v| v.request()) == rhs.download().map(|v| v.request())
    }
}
impl Eq for BlobInner {}
#[derive(Clone, Eq, PartialEq, Allocative, Dupe)]
pub struct SelectedScratchBlob(pub(super) BudgetedImageBlock<BlobInner>);
impl SelectedScratchBlob {
    pub fn bytes(&self) -> &[u8] {
        self.0.get().bytes()
    }
    pub fn download(&self) -> Option<&Arc<SelectedExtensionDownloadVerified>> {
        self.0.get().download()
    }
    pub fn same_resources(&self, r: &SelectedScratchResources) -> bool {
        self.0.same_budget(r.descriptor_budget())
            && self
                .0
                .get()
                .payload
                .budget
                .same_instance(r.payload_budget())
    }
}
impl fmt::Debug for SelectedScratchBlob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SelectedScratchBlob")
            .field("bytes", &self.bytes().len())
            .field("download", &self.download().is_some())
            .finish()
    }
}

#[derive(Debug, Eq, PartialEq, Allocative)]
pub(super) struct ExternalInner {
    pub origin: BudgetedImageBlock<Origin>,
    pub event_index: usize,
    pub spelling: SelectedScratchText,
}
#[derive(Clone, Debug, Eq, PartialEq, Allocative, Dupe)]
pub struct SelectedScratchExternalPath(pub(super) BudgetedImageBlock<ExternalInner>);
impl SelectedScratchExternalPath {
    pub fn event_index(&self) -> usize {
        self.0.get().event_index
    }
    pub fn spelling(&self) -> &str {
        self.0.get().spelling.as_str()
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Allocative)]
pub enum SelectedScratchOperation {
    Path {
        literal: SelectedScratchText,
        result: SelectedScratchPath,
    },
    Parent {
        from: SelectedScratchPath,
        result: SelectedScratchPath,
    },
    Child {
        from: SelectedScratchPath,
        literal: SelectedScratchText,
        result: SelectedScratchPath,
    },
    File {
        path: SelectedScratchPath,
        blob: SelectedScratchBlob,
        executable: bool,
    },
    Download {
        path: SelectedScratchPath,
        blob: SelectedScratchBlob,
    },
    ExternalPath {
        path: SelectedScratchExternalPath,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Allocative)]
pub enum SelectedScratchEntry {
    Directory {
        path: SelectedScratchPath,
    },
    File {
        path: SelectedScratchPath,
        blob: SelectedScratchBlob,
        executable: bool,
    },
}
impl SelectedScratchEntry {
    pub fn path(&self) -> &SelectedScratchPath {
        match self {
            Self::Directory { path } | Self::File { path, .. } => path,
        }
    }
    pub fn blob(&self) -> Option<&SelectedScratchBlob> {
        match self {
            Self::File { blob, .. } => Some(blob),
            _ => None,
        }
    }
    pub fn mode(&self) -> u32 {
        match self {
            Self::Directory { .. } => 0o755,
            Self::File {
                executable: true, ..
            } => 0o755,
            Self::File { .. } => 0o644,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Allocative)]
pub enum SelectedScratchOperand {
    Literal(SelectedScratchText),
    Scratch(SelectedScratchPath),
    ExternalPath(SelectedScratchExternalPath),
}
impl SelectedScratchOperand {
    pub fn spelling(&self) -> &str {
        match self {
            Self::Literal(t) => t.as_str(),
            Self::Scratch(p) => p.guest_spelling(),
            Self::ExternalPath(p) => p.spelling(),
        }
    }
    pub fn external_path(&self) -> Option<&SelectedScratchExternalPath> {
        match self {
            Self::ExternalPath(p) => Some(p),
            _ => None,
        }
    }
    pub fn scratch_path(&self) -> Option<&SelectedScratchPath> {
        match self {
            Self::Scratch(p) => Some(p),
            _ => None,
        }
    }
}
pub enum SelectedScratchOperandSpec<'a> {
    Literal(&'a str),
    Scratch(&'a SelectedScratchPath),
    ExternalPath(&'a SelectedScratchExternalPath),
}
pub struct SelectedScratchProcessSpec<'a> {
    pub arguments: &'a [SelectedScratchOperandSpec<'a>],
    pub cwd: &'a str,
    pub environment: &'a [(&'a str, Option<&'a str>)],
    pub timeout_seconds: u64,
    pub quiet: bool,
}
#[derive(Clone, Debug, Eq, PartialEq, Allocative)]
pub struct SelectedScratchEnvironmentChange {
    pub(super) name: SelectedScratchText,
    pub(super) value: Option<SelectedScratchText>,
}
impl SelectedScratchEnvironmentChange {
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
    pub fn value(&self) -> Option<&str> {
        self.value.as_ref().map(SelectedScratchText::as_str)
    }
}
#[derive(Eq, PartialEq, Allocative)]
pub(super) struct ProcessInner {
    pub arguments: Vec<SelectedScratchOperand>,
    pub cwd: SelectedScratchText,
    pub environment: Vec<SelectedScratchEnvironmentChange>,
    pub inherited: RepositoryEnvironmentSnapshot,
    pub timeout: u64,
    pub quiet: bool,
}
#[derive(Clone, Eq, PartialEq, Allocative, Dupe)]
pub struct SelectedScratchProcessCall(pub(super) BudgetedImageBlock<ProcessInner>);
impl SelectedScratchProcessCall {
    pub fn arguments(&self) -> &[SelectedScratchOperand] {
        &self.0.get().arguments
    }
    pub fn cwd(&self) -> &str {
        self.0.get().cwd.as_str()
    }
    pub fn environment(&self) -> &[SelectedScratchEnvironmentChange] {
        &self.0.get().environment
    }
    pub fn inherited_environment(&self) -> &RepositoryEnvironmentSnapshot {
        &self.0.get().inherited
    }
    pub fn timeout_seconds(&self) -> u64 {
        self.0.get().timeout
    }
    pub fn quiet(&self) -> bool {
        self.0.get().quiet
    }
}
impl fmt::Debug for SelectedScratchProcessCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SelectedScratchProcessCall")
            .field("argv", &self.arguments().len())
            .field("env", &self.environment().len())
            .finish_non_exhaustive()
    }
}
#[derive(Debug, Eq, PartialEq, Allocative)]
pub(super) struct PrefixInner {
    pub origin: BudgetedImageBlock<Origin>,
    pub operations: Vec<SelectedScratchOperation>,
    pub entries: Vec<SelectedScratchEntry>,
    pub process: SelectedScratchProcessCall,
    pub occurrence_bytes: usize,
    pub descriptor_bytes: usize,
}
#[derive(Clone, Debug, Eq, PartialEq, Allocative, Dupe)]
pub struct SelectedScratchPrefix(pub(super) BudgetedImageBlock<PrefixInner>);
impl SelectedScratchPrefix {
    pub fn workspace(&self) -> &ImageChargedPath {
        &self.0.get().origin.get().workspace
    }
    pub fn owner(&self) -> &Arc<HostSelectedExtensionOwner> {
        &self.0.get().origin.get().owner
    }
    pub fn policy(&self) -> &SelectedScratchPolicy {
        &self.0.get().origin.get().policy
    }
    pub fn operations(&self) -> &[SelectedScratchOperation] {
        &self.0.get().operations
    }
    pub fn entries(&self) -> &[SelectedScratchEntry] {
        &self.0.get().entries
    }
    pub fn process(&self) -> &SelectedScratchProcessCall {
        &self.0.get().process
    }
    pub fn occurrence_bytes(&self) -> usize {
        self.0.get().occurrence_bytes
    }
    pub fn descriptor_bytes(&self) -> usize {
        self.0.get().descriptor_bytes
    }
    pub fn same_resources(&self, r: &SelectedScratchResources) -> bool {
        self.0.get().origin.get().resources.same_instance(r)
    }
}
