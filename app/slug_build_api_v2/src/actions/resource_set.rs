//! Deferred numeric resource callbacks; remote execution retains without calling.
use std::alloc::Layout;
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;

use crate::PublishedValueError;
use crate::PublishedValueRef;
use crate::analysis_value::PublicationEqState;

/// Server-process Host OS, independent of target and execution platforms.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ResourceSetOs {
    Linux,
    Osx,
    Windows,
    FreeBsd,
    OpenBsd,
    Unknown,
}
impl ResourceSetOs {
    pub fn canonical_name(self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::Osx => "osx",
            Self::Windows => "windows",
            Self::FreeBsd => "freebsd",
            Self::OpenBsd => "openbsd",
            Self::Unknown => "unknown",
        }
    }
}

/// A numeric consumer supplies tracked parent membership, including tools and
/// executable, counting tree artifacts once and excluding virtual paramfiles.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ResourceSetInputs {
    pub os: ResourceSetOs,
    pub parent_input_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResourceEstimate {
    pub memory_mb: f64,
    pub cpu: f64,
    pub local_test: i32,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ResourceSetError {
    Unsupported(CompactString),
    Callback(CompactString),
    InvalidReturn(CompactString),
    Limit(CompactString),
    Cancelled,
}
impl fmt::Display for ResourceSetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(message) => write!(f, "resource_set unsupported: {message}"),
            Self::Callback(message) => write!(f, "resource_set callback: {message}"),
            Self::InvalidReturn(message) => write!(f, "resource_set return: {message}"),
            Self::Limit(message) => write!(f, "resource_set limit: {message}"),
            Self::Cancelled => f.write_str("resource_set evaluation cancelled"),
        }
    }
}
impl std::error::Error for ResourceSetError {}

/// Current consumer policy, borrowed only for this numeric call.
pub trait ResourceSetEvaluationContext {
    fn is_cancelled(&self) -> bool;
    fn print(&self, message: &str) -> Result<(), ResourceSetError>;
    fn mnemonic(&self) -> &str;
}

/// One fresh evaluator per explicit invocation. No producer evaluator, request,
/// cancellation token, action sink or lock may be retained by the backend.
pub trait ResourceSetRuntime: fmt::Debug + Send + Sync {
    fn evaluate(
        &self,
        inputs: ResourceSetInputs,
        context: &dyn ResourceSetEvaluationContext,
    ) -> Result<ResourceEstimate, ResourceSetError>;
}

#[derive(Debug, Clone, Dupe, Allocative)]
pub struct RetainedSpawnResourceSet(Arc<RetainedSpawnResourceSetData>);

#[derive(Debug, Allocative)]
struct RetainedSpawnResourceSetData {
    callable: PublishedValueRef,
    // Executable foreign ownership and allocation belong to the publisher.
    #[allocative(skip)]
    runtime: Arc<dyn ResourceSetRuntime>,
}
impl RetainedSpawnResourceSet {
    pub fn from_published(
        callable: PublishedValueRef,
        runtime: Arc<dyn ResourceSetRuntime>,
    ) -> Result<Self, PublishedValueError> {
        callable.arena().validate_root(callable.node_id())?;
        if callable.original_slot().is_none() {
            return Err(PublishedValueError::Invalid("resource_set original root"));
        }
        Ok(Self(Arc::new(RetainedSpawnResourceSetData {
            callable,
            runtime,
        })))
    }

    pub fn published_callable(&self) -> &PublishedValueRef {
        &self.0.callable
    }

    pub fn evaluate(
        &self,
        inputs: ResourceSetInputs,
        context: &dyn ResourceSetEvaluationContext,
    ) -> Result<ResourceEstimate, ResourceSetError> {
        self.0.runtime.evaluate(inputs, context)
    }

    /// Typed descriptor allocation only: data, alignment and Arc counters.
    /// The publisher separately prepays its runtime allocation and originals.
    /// This layout supplies no admission credit or independent budget.
    pub fn allocation_layout() -> (usize, usize) {
        let (layout, _) = Layout::new::<[usize; 2]>()
            .extend(Layout::new::<RetainedSpawnResourceSetData>())
            .expect("fixed resource_set descriptor layout");
        let bytes = layout.pad_to_align().size();
        (bytes.div_ceil(std::mem::size_of::<usize>()), bytes)
    }

    pub(crate) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.0
            .callable
            .publication_eq_with(&other.0.callable, state)
    }
}
impl PartialEq for RetainedSpawnResourceSet {
    fn eq(&self, other: &Self) -> bool {
        self.publication_eq_with(other, &mut PublicationEqState::default())
    }
}
impl Eq for RetainedSpawnResourceSet {}
