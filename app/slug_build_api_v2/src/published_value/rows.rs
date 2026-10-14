use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;

use super::*;
use crate::AnalysisDepsetOccurrence;
use crate::AnalysisTargetIdentity;
use crate::AnalysisValue;
use crate::AnalysisValueType;
use crate::DepsetOrder;
use crate::FilesToRunProvider;
use crate::ProviderId;
use crate::ProviderIdentity;
use crate::callable_graph::CallableDefinitionSpan;
use crate::callable_graph::CallableGlobalAccess;
use crate::callable_graph::CallableParameterMode;
use crate::callable_graph::CallableSourceAuthority;
use crate::callable_graph::NativeCallableApiId;
#[derive(Debug, Clone, Copy, Eq, PartialEq, Allocative)]
pub enum PublishedParameterRequirement {
    Required,
    Optional,
    Defaulted(PublishedEdge),
    Variadic,
}
#[derive(Debug, Clone, Allocative)]
pub struct PublishedParameter {
    pub name: CompactString,
    pub mode: CallableParameterMode,
    pub requirement: PublishedParameterRequirement,
    pub annotation: Option<PublishedEdge>,
}
#[derive(Debug, Clone, Allocative)]
pub struct PublishedGlobal {
    pub name: CompactString,
    pub access: CallableGlobalAccess,
    pub value: Option<PublishedEdge>,
}
#[derive(Debug, Clone, Allocative)]
pub struct PublishedCode {
    pub source: CallableSourceAuthority,
    pub name: CompactString,
    pub definition: CallableDefinitionSpan,
    pub dependencies: PublishedEdgeRange,
    pub globals: Arc<[PublishedGlobal]>,
}
#[derive(Debug, Clone, Allocative)]
pub struct PublishedFunction {
    pub symbol: PublishedFunctionSymbol,
    pub code: PublishedCode,
    pub parameters: Arc<[PublishedParameter]>,
    pub return_annotation: Option<PublishedEdge>,
    pub capture_names: Arc<[CompactString]>,
    pub captures: PublishedEdgeRange,
}
#[derive(Debug, Clone, Allocative)]
pub struct PublishedIntrinsic {
    pub api: NativeCallableApiId,
    pub parameters: Arc<[PublishedParameter]>,
    pub return_annotation: Option<PublishedEdge>,
    pub receiver: Option<PublishedEdge>,
}
#[derive(Debug, Clone, Allocative)]
pub struct PublishedProvider {
    pub identity: ProviderIdentity,
    pub has_schema: bool,
    pub names: Arc<[CompactString]>,
    pub values: PublishedEdgeRange,
    pub files_to_run: Option<PurePublishedFilesToRun>,
}
#[derive(Debug, Clone, Allocative)]
pub enum PublishedProviderSchema {
    Schemaless,
    List(Arc<[CompactString]>),
    Documented(Arc<[CompactString]>),
    Initialized(Arc<[CompactString]>),
}
#[derive(Debug, Clone, Allocative)]
pub struct PublishedProviderConstructor {
    pub id: ProviderId,
    pub schema: PublishedProviderSchema,
    pub init: Option<PublishedEdge>,
}
#[derive(Debug, Clone, Allocative)]
pub enum PublishedDepsetInput {
    Direct(PublishedEdge),
    Transitive(PublishedEdge),
}
#[derive(Debug, Clone, Allocative)]
pub struct PublishedDepset {
    pub order: DepsetOrder,
    pub element_type: AnalysisValueType,
    pub depth: usize,
    pub occurrence: AnalysisDepsetOccurrence,
    pub successors: Arc<[PublishedDepsetInput]>,
}
#[derive(Debug, Clone, Allocative)]
pub enum PublishedVariableSeed {
    Value(PublishedEdge),
    Sequence(PublishedEdgeRange),
    Missing,
    Error(CompactString),
}
#[derive(Debug, Clone, Allocative)]
pub struct PublishedCcVariables {
    pub occurrence: crate::cc_variables::CcVariablesOccurrence,
    pub kind: crate::cc_variables::CcVariablesKind,
    pub parent: Option<PublishedEdge>,
    pub locals: Arc<[(CompactString, PublishedEdge)]>,
    pub seed: Arc<[(CompactString, PublishedVariableSeed)]>,
}
/// Complete successful strict-audit facts, independent of producer history.
/// Depth is relative ordinary containment, never typed depset DAG height.
#[derive(Debug, Clone, Copy, Allocative)]
pub(super) struct NativeCertificate {
    pub(super) visits: usize,
    pub(super) ordinary_depth: usize,
}
#[derive(Debug, Clone, Allocative)]
pub struct PurePublishedAtom(pub(super) AnalysisValue, pub(super) NativeCertificate);
impl PurePublishedAtom {
    /// Canonical complete strict-audit contribution, independent of producer history.
    pub(super) fn retained_visits(&self) -> usize {
        self.1.visits
    }
    pub(super) fn ordinary_depth(&self) -> usize {
        self.1.ordinary_depth
    }

    pub fn value(&self) -> &AnalysisValue {
        &self.0
    }
}
#[derive(Debug, Clone, Allocative)]
pub struct PurePublishedFilesToRun(pub(super) FilesToRunProvider, pub(super) NativeCertificate);
impl PurePublishedFilesToRun {
    /// Canonical complete strict-audit contribution, independent of producer history.
    pub(super) fn retained_visits(&self) -> usize {
        self.1.visits
    }
    pub(super) fn ordinary_depth(&self) -> usize {
        self.1.ordinary_depth
    }

    pub fn value(&self) -> &FilesToRunProvider {
        &self.0
    }
}
#[derive(Debug, Clone, Allocative)]
pub enum PublishedNode {
    Function(PublishedFunction),
    Intrinsic(PublishedIntrinsic),
    Code(PublishedCode),
    ProviderConstructor(PublishedProviderConstructor),
    RawProviderConstructor {
        provider: PublishedEdge,
        occurrence: RawProviderConstructorOccurrence,
    },
    Cell {
        contents: Option<PublishedEdge>,
    },
    Set(PublishedEdgeRange),
    List(PublishedEdgeRange),
    Tuple(PublishedEdgeRange),
    Dictionary(PublishedEdgeRange),
    Struct {
        names: Arc<[CompactString]>,
        values: PublishedEdgeRange,
    },
    Provider(PublishedProvider),
    ConfiguredTarget {
        identity: AnalysisTargetIdentity,
        providers: Arc<[PublishedTargetProvider]>,
    },
    Depset(PublishedDepset),
    CcVariables(PublishedCcVariables),
    Native(PurePublishedAtom),
}

#[derive(Debug, Clone, Allocative)]
pub struct PublishedProviderConstructorSymbol {
    pub id: ProviderId,
    pub schema: PublishedProviderSchema,
}
impl PartialEq for PublishedProviderConstructorSymbol {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for PublishedProviderConstructorSymbol {}
impl std::hash::Hash for PublishedProviderConstructorSymbol {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.id, state)
    }
}

#[derive(Debug, Clone, dupe::Dupe, Allocative)]
pub struct RawProviderConstructorOccurrence(Arc<()>);
impl RawProviderConstructorOccurrence {
    pub(crate) fn pointer(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
    pub fn new() -> Self {
        Self(Arc::new(()))
    }
}
impl Default for RawProviderConstructorOccurrence {
    fn default() -> Self {
        Self::new()
    }
}
impl PartialEq for RawProviderConstructorOccurrence {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for RawProviderConstructorOccurrence {}
#[derive(Debug, Clone, Allocative)]
pub struct PublishedRawProviderConstructorSymbol {
    pub provider: ProviderId,
    pub occurrence: RawProviderConstructorOccurrence,
}
impl PartialEq for PublishedRawProviderConstructorSymbol {
    fn eq(&self, other: &Self) -> bool {
        self.occurrence == other.occurrence
    }
}
impl Eq for PublishedRawProviderConstructorSymbol {}

#[derive(Debug, Clone, Allocative)]
pub struct PurePublishedProviderValue(
    pub(super) crate::ProviderValue,
    pub(super) NativeCertificate,
);
impl PurePublishedProviderValue {
    /// Canonical complete strict-audit contribution, independent of producer history.
    pub(super) fn retained_visits(&self) -> usize {
        self.1.visits
    }
    pub(super) fn ordinary_depth(&self) -> usize {
        self.1.ordinary_depth
    }

    pub fn value(&self) -> &crate::ProviderValue {
        &self.0
    }
}
#[derive(Debug, Clone, Allocative)]
pub enum PublishedTargetProvider {
    Native {
        provider: PurePublishedProviderValue,
        view: PublishedEdge,
    },
    Occurrence(PublishedEdge),
}
