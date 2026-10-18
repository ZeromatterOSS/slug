//! Original build-info callback ownership and validated immutable action metadata.
use std::alloc::Layout;
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;

use crate::ActionOutput;
use crate::ActionOutputKind;
use crate::ActionOutputMetadata;
use crate::AnalysisArtifact;
use crate::PublishedValueError;
use crate::PublishedValueRef;
use crate::WorkspaceStatusArtifact;
use crate::WorkspaceStatusArtifactKind;
use crate::analysis_value::PublicationEqState;

/// Native scalar output limit; VM/rendering admission is separate from staging.
pub const BUILD_INFO_MAX_OUTPUT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Allocative)]
pub enum BuildInfoTransformKind {
    Info,
    Version,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum BuildInfoTransformError {
    Unsupported(CompactString),
    Callback(CompactString),
    InvalidReturn(CompactString),
    Limit(CompactString),
    Cancelled,
}
impl fmt::Display for BuildInfoTransformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(m) => write!(f, "build-info unsupported: {m}"),
            Self::Callback(m) => write!(f, "build-info callback: {m}"),
            Self::InvalidReturn(m) => write!(f, "build-info return: {m}"),
            Self::Limit(m) => write!(f, "build-info limit: {m}"),
            Self::Cancelled => f.write_str("build-info evaluation cancelled"),
        }
    }
}
impl std::error::Error for BuildInfoTransformError {}

/// Current native consumer policy, borrowed for one invocation only.
pub trait BuildInfoEvaluationContext {
    fn is_cancelled(&self) -> bool;
    fn print(&self, message: &str) -> Result<(), BuildInfoTransformError>;
    fn mnemonic(&self) -> &str;
}
/// The original frozen backend owns one budget/deadline across parsing, VM,
/// validation, ordered replacement and final encoding. No consumer is retained.
pub trait BuildInfoTransformRuntime: fmt::Debug + Send + Sync {
    fn render(
        &self,
        status: &[u8],
        template: &[u8],
        context: &dyn BuildInfoEvaluationContext,
    ) -> Result<Vec<u8>, BuildInfoTransformError>;
}
#[derive(Debug, Clone, Dupe, Allocative)]
pub struct RetainedBuildInfoTransform(Arc<RetainedBuildInfoTransformData>);
#[derive(Debug, Allocative)]
struct RetainedBuildInfoTransformData {
    callable: PublishedValueRef,
    #[allocative(skip)]
    runtime: Arc<dyn BuildInfoTransformRuntime>,
}
impl RetainedBuildInfoTransform {
    pub fn from_published(
        callable: PublishedValueRef,
        runtime: Arc<dyn BuildInfoTransformRuntime>,
    ) -> Result<Self, PublishedValueError> {
        callable.arena().validate_root(callable.node_id())?;
        if callable.original_slot().is_none() {
            return Err(PublishedValueError::Invalid("build-info original root"));
        }
        Ok(Self(Arc::new(RetainedBuildInfoTransformData {
            callable,
            runtime,
        })))
    }
    pub fn published_callable(&self) -> &PublishedValueRef {
        &self.0.callable
    }
    pub fn render(
        &self,
        status: &[u8],
        template: &[u8],
        context: &dyn BuildInfoEvaluationContext,
    ) -> Result<Vec<u8>, BuildInfoTransformError> {
        self.0.runtime.render(status, template, context)
    }
    /// Typed descriptor and Arc counters; grants no budget credit.
    pub fn allocation_layout() -> (usize, usize) {
        allocation_layout::<RetainedBuildInfoTransformData>()
    }
    pub(crate) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.0
            .callable
            .publication_eq_with(&other.0.callable, state)
    }
}
impl PartialEq for RetainedBuildInfoTransform {
    fn eq(&self, other: &Self) -> bool {
        self.publication_eq_with(other, &mut PublicationEqState::default())
    }
}
impl Eq for RetainedBuildInfoTransform {}
fn allocation_layout<T>() -> (usize, usize) {
    let (layout, _) = Layout::new::<[usize; 2]>()
        .extend(Layout::new::<T>())
        .expect("fixed build-info allocation layout");
    let bytes = layout.pad_to_align().size();
    (bytes.div_ceil(std::mem::size_of::<usize>()), bytes)
}

/// Owned registration data; constructing it never fabricates or calls a function.
#[derive(Debug, Eq, PartialEq, Allocative)]
pub struct BuildInfoTransformHeader {
    kind: BuildInfoTransformKind,
    inputs: [AnalysisArtifact; 2],
    output: ActionOutput,
}
impl BuildInfoTransformHeader {
    pub fn new(
        kind: BuildInfoTransformKind,
        status: WorkspaceStatusArtifact,
        template: AnalysisArtifact,
        output: ActionOutput,
    ) -> Result<Self, String> {
        let volatile = kind == BuildInfoTransformKind::Version;
        let expected_kind = if volatile {
            WorkspaceStatusArtifactKind::Volatile
        } else {
            WorkspaceStatusArtifactKind::Stable
        };
        let expected_metadata = if volatile {
            ActionOutputMetadata::Constant
        } else {
            ActionOutputMetadata::Ordinary
        };
        if status.kind() != expected_kind
            || output.kind() != ActionOutputKind::File
            || output.metadata() != expected_metadata
        {
            return Err(
                "build-info transform status/output metadata must match its role".to_owned(),
            );
        }
        super::registry::validate_output(&output).map_err(|error| error.to_string())?;
        Ok(Self {
            kind,
            inputs: [AnalysisArtifact::WorkspaceStatus(status), template],
            output,
        })
    }
    pub fn kind(&self) -> BuildInfoTransformKind {
        self.kind
    }
    pub fn status(&self) -> &WorkspaceStatusArtifact {
        let AnalysisArtifact::WorkspaceStatus(status) = &self.inputs[0] else {
            unreachable!("typed status input")
        };
        status
    }
    pub fn template(&self) -> &AnalysisArtifact {
        &self.inputs[1]
    }
    pub fn inputs(&self) -> &[AnalysisArtifact] {
        if self.inputs[0] == self.inputs[1] {
            &self.inputs[..1]
        } else {
            &self.inputs
        }
    }
    pub fn output(&self) -> &ActionOutput {
        &self.output
    }
    pub fn is_volatile(&self) -> bool {
        self.kind == BuildInfoTransformKind::Version
    }
    pub fn execute_unconditionally(&self) -> bool {
        self.is_volatile()
    }
    pub fn progress_message(&self) -> &'static str {
        if self.is_volatile() {
            "Translating volatile BuildInfo file"
        } else {
            "Translating stable BuildInfo file"
        }
    }
}

#[derive(Debug, Eq, PartialEq, Allocative)]
struct BuildInfoTransformData {
    header: BuildInfoTransformHeader,
    callback: RetainedBuildInfoTransform,
}
#[derive(Debug, Clone, Dupe, Allocative)]
pub struct BuildInfoTransformSpec(Arc<BuildInfoTransformData>);
impl BuildInfoTransformSpec {
    pub fn new(header: BuildInfoTransformHeader, callback: RetainedBuildInfoTransform) -> Self {
        Self(Arc::new(BuildInfoTransformData { header, callback }))
    }
    pub fn allocation_layout() -> (usize, usize) {
        allocation_layout::<BuildInfoTransformData>()
    }
    pub fn header(&self) -> &BuildInfoTransformHeader {
        &self.0.header
    }
    pub fn kind(&self) -> BuildInfoTransformKind {
        self.header().kind()
    }
    pub fn status(&self) -> &WorkspaceStatusArtifact {
        self.header().status()
    }
    pub fn template(&self) -> &AnalysisArtifact {
        self.header().template()
    }
    pub fn inputs(&self) -> &[AnalysisArtifact] {
        self.header().inputs()
    }
    pub fn output(&self) -> &ActionOutput {
        self.header().output()
    }
    pub fn callback(&self) -> &RetainedBuildInfoTransform {
        &self.0.callback
    }
    pub fn is_volatile(&self) -> bool {
        self.header().is_volatile()
    }
    pub fn execute_unconditionally(&self) -> bool {
        self.header().execute_unconditionally()
    }
    pub fn progress_message(&self) -> &'static str {
        self.header().progress_message()
    }
    pub(crate) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.header() == other.header()
            && self.callback().publication_eq_with(other.callback(), state)
    }
}
impl PartialEq for BuildInfoTransformSpec {
    fn eq(&self, other: &Self) -> bool {
        self.publication_eq_with(other, &mut PublicationEqState::default())
    }
}
impl Eq for BuildInfoTransformSpec {}

#[cfg(test)]
mod tests;
