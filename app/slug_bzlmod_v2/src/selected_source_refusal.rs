//! Fixed-size redacted projection of the selected source owner's retained error.
use allocative::Allocative;
use dupe::Dupe;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpochError;

use crate::registry_dice::RegistryFileError;
use crate::registry_dice::RegistryRefusalCause;
use crate::registry_dice::RegistryRefusalDetail;

const MAX_CODES: usize = 16;
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
pub struct SelectedSourceRefusal {
    codes: [SelectedSourceRefusalCode; MAX_CODES],
    len: u8,
    termination: SelectedSourceRefusalTermination,
    native_cause: Option<RegistryRefusalCause>,
}
// Copy fixes every field to inline storage; Dupe is a constant-sized copy.
impl Dupe for SelectedSourceRefusal {}
impl Default for SelectedSourceRefusal {
    fn default() -> Self {
        Self {
            codes: [SelectedSourceRefusalCode::Unknown; MAX_CODES],
            len: 0,
            termination: SelectedSourceRefusalTermination::Unknown,
            native_cause: None,
        }
    }
}
impl SelectedSourceRefusal {
    pub fn codes(&self) -> &[SelectedSourceRefusalCode] {
        &self.codes[..usize::from(self.len)]
    }
    pub fn termination(&self) -> SelectedSourceRefusalTermination {
        self.termination
    }
    pub fn variant_path_complete(&self) -> bool {
        matches!(
            self.termination,
            SelectedSourceRefusalTermination::Classified
                | SelectedSourceRefusalTermination::OpaqueLeaf
        )
    }
    pub fn native_cause(&self) -> Option<RegistryRefusalCause> {
        self.native_cause
    }
    pub fn native_detail_observed(&self) -> bool {
        self.native_cause
            .is_some_and(|cause| !matches!(cause.detail, RegistryRefusalDetail::Opaque))
    }
}
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum SelectedSourceRefusalTermination {
    Classified,
    OpaqueLeaf,
    Unknown,
    DepthLimit,
    UnexpectedSuccess,
}
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
#[repr(u16)]
pub enum SelectedSourceRefusalCode {
    Unknown = 0,
    PathEpoch = 1,
    PathDuplicateDemand = 2,
    PathConflictingDemand = 3,
    PathInputImagePathMismatch = 4,
    PathOperationMismatch = 5,
    RootMappingPredecessor = 6,
    RootMappingCompute = 7,
    RootMappingInvalid = 8,
    RootInvalidMissing = 9,
    RootInvalidDuplicate = 10,
    RootInvalidContext = 11,
    ExtensionMappingsRoutes = 12,
    ExtensionMappingsRoutesCompute = 13,
    ExtensionMappingsRootFiles = 14,
    ExtensionMappingsRootFilesCompute = 15,
    ExtensionMappingsInvalid = 16,
    RoutesGraph = 17,
    RoutesGraphCompute = 18,
    RoutesRepoSpecs = 19,
    RoutesRepoSpecsCompute = 20,
    RoutesInvalid = 21,
    RoutesCanonicalCollision = 22,
    RoutesRegistryMismatch = 23,
    RepoSpecsGraph = 24,
    RepoSpecsGraphCompute = 25,
    RepoSpecsRegistryPolicy = 26,
    RepoSpecsRegistryPolicyCompute = 27,
    RepoSpecsRegistryFile = 28,
    RepoSpecsRegistryFileCompute = 29,
    RepoSpecsMissingRegistryFile = 30,
    RepoSpecsJson = 31,
    RepoSpecsProjection = 32,
    RepoSpecsEffectiveOverride = 33,
    RepoSpecsEffectiveOverrideCompute = 34,
    RepoSpecsIncompatibleNeeds = 35,
    DemandMappings = 36,
    DemandMissing = 37,
    DemandAmbiguous = 38,
    DemandInconsistent = 39,
    GraphInput = 40,
    GraphDiscoveryCompute = 41,
    GraphDiscoveryLeaf = 42,
    GraphIncompatibleNeeds = 43,
    GraphUnusedOverride = 44,
    GraphAllowedVersionMissing = 45,
    GraphMultipleVersionNoCeiling = 46,
    GraphMissingSelectedModule = 47,
    GraphDuplicateDependency = 48,
    DiscoveryRootModuleFiles = 49,
    DiscoveryExplicitBuiltinOverride = 50,
    DiscoveryInvalidBuiltinVersion = 51,
    DiscoveryMissingVersion = 52,
    DiscoverySourcePreparationCompute = 53,
    DiscoverySourcePreparation = 54,
    DiscoveryInvalidNonRegistryVersion = 55,
    DiscoveryNonRegistryClosureCompute = 56,
    DiscoveryNonRegistryClosure = 57,
    DiscoveryNonRegistryCycle = 58,
    DiscoveryNonRegistryUnsupported = 59,
    DiscoveryBuiltinCompute = 60,
    DiscoveryBuiltin = 61,
    DiscoveryEvaluation = 62,
    ModuleSourceRootModuleFiles = 63,
    ModuleSourceRegistryPolicy = 64,
    ModuleSourceRegistryFileCompute = 65,
    ModuleSourceRegistryFile = 66,
    ModuleSourceRegistryPolicyCompute = 67,
    ModuleSourceSource = 68,
    ModuleSourceSourceCompute = 69,
    ModuleSourceInvalidPatchPath = 70,
    ModuleSourcePatchMissing = 71,
    ModuleSourcePatchWrongKind = 72,
    ModuleSourcePatchResolution = 73,
    ModuleSourcePatchResolutionCompute = 74,
    ModuleSourcePatchFileObservation = 75,
    ModuleSourcePatchFileInconsistentState = 76,
    ModuleSourcePatchFileCompute = 77,
    ModuleSourcePatch = 78,
    ModuleSourceMissingVersion = 79,
    ModuleSourceModuleNotFound = 80,
    NonRegistryRootModuleFiles = 81,
    NonRegistryNonregistryOverrideRequired = 82,
    NonRegistryMaterializationCompute = 83,
    NonRegistryMaterialization = 84,
    NonRegistryRootSourceCompute = 85,
    NonRegistryRootSource = 86,
    NonRegistryRootAbsent = 87,
    NonRegistryRootValidation = 88,
    NonRegistryBadLabel = 89,
    NonRegistryPackage = 90,
    NonRegistryFragment = 91,
    MaterializationRootModuleFiles = 92,
    MaterializationMissingOverride = 93,
    MaterializationUnsupportedOverride = 94,
    MaterializationInvalidCanonicalRepository = 95,
    MaterializationInvalidWorkspace = 96,
    MaterializationResultCompute = 97,
    MaterializationMissingGeneration = 98,
    MaterializationSpec = 99,
    MaterializationTransport = 100,
    MaterializationMaterialization = 101,
    RepositorySourceInvalidRepoRelativePath = 102,
    RepositorySourceMaterializationCompute = 103,
    RepositorySourceMaterialization = 104,
    RepositorySourceInvalidMaterializedPath = 105,
    RepositorySourceObservation = 106,
    RepositorySourceInconsistentState = 107,
    RepositorySourceWrongKind = 108,
    RepositorySourceCycle = 109,
    RepositorySourceInfiniteExpansion = 110,
    RepositorySourceResolutionCompute = 111,
    RepositorySourceFileCompute = 112,
    RegistryPolicyLockfileModeInput = 113,
    RegistryPolicyVendorDirectoryInput = 114,
    RegistryPolicyRefreshTokenInput = 115,
    RegistryPolicyVisibleLockfile = 116,
    RegistryPolicyModuleMirrorsInput = 117,
    RegistryPolicyInvalidPrimaryRegistryUri = 118,
    RegistryPolicyInvalidModuleMirrorUri = 119,
    RootObservationMappings = 120,
    DemandObservationMappings = 121,
    ExtensionObservationRoutes = 122,
    ExtensionObservationRootFiles = 123,
    ExtensionObservationMerge = 124,
    RoutesObservationGraph = 125,
    RoutesObservationMappings = 126,
    RoutesObservationRepoSpecs = 127,
    RoutesObservationMerge = 128,
    MappingsObservationGraph = 129,
    MappingsObservationRoot = 130,
    MappingsObservationMerge = 131,
    RepoSpecsObservationGraph = 132,
    RepoSpecsObservationHostRegistry = 133,
    RepoSpecsObservationRegistryFile = 134,
    RepoSpecsObservationEffectiveOverride = 135,
    RepoSpecsObservationMerge = 136,
    GraphObservationRoot = 137,
    GraphObservationEffective = 138,
    GraphObservationDiscovery = 139,
    GraphObservationMerge = 140,
    DiscoveryObservationEffectiveFrontier = 141,
    DiscoveryObservationClosureFrontier = 142,
    DiscoveryObservationPreparationFrontier = 143,
    DiscoveryObservationMergeFrontier = 144,
    ClosureObservationEffectiveFrontier = 145,
    ClosureObservationEffectiveCompute = 146,
    ClosureObservationMaterializationFrontier = 147,
    ClosureObservationRootSourceFrontier = 148,
    ClosureObservationPreparationFrontier = 149,
    PreparationObservationPath = 150,
    PreparationObservationPackage = 151,
    PreflightObservationEffectiveFrontier = 152,
    PreflightObservationEffectiveCompute = 153,
    PreflightObservationPolicyCompute = 154,
    PreflightObservationIgnoreFrontier = 155,
    PreflightObservationIgnoreCompute = 156,
    PreflightObservationMarkerFrontier = 157,
    PreflightObservationMarkerCompute = 158,
    RegistryFileInvalidFileUrl = 159,
    RegistryFileUnsupportedUrl = 160,
    RegistryFileMissingRegistryUrls = 161,
    RegistryFileMissingLockfileMode = 162,
    RegistryFileMissingRequestGeneration = 163,
    RegistryFileRootModuleFiles = 164,
    RegistryFileMissingIoCapability = 165,
    RegistryFileMissingChecksumInError = 166,
    RegistryFileTransport = 167,
    RegistryFileChecksumMismatch = 168,
    RegistryFileLocalRead = 169,
    RegistryFileInvalidLockfileExpectation = 170,
    ExtensionStageRoutes = 171,
    ExtensionStageRootFiles = 172,
    RoutesStageGraph = 173,
    RoutesStageRepoSpecs = 174,
    RepoSpecsStageGraph = 175,
    RepoSpecsStageHostRegistry = 176,
    RepoSpecsStageSourceRegistryFile = 177,
    RepoSpecsStageRegistryMetadataFile = 178,
    RepoSpecsStageEffectiveOverride = 179,
}
impl SelectedSourceRefusalCode {
    pub fn code(self) -> u16 {
        self as u16
    }
}
// A builder cannot select a cause from a sibling, success, or a deeper truncated path.
pub(crate) struct SelectedSourceRefusalBuilder {
    value: SelectedSourceRefusal,
    stopped: bool,
}
impl SelectedSourceRefusalBuilder {
    pub(crate) fn new() -> Self {
        Self {
            value: SelectedSourceRefusal::default(),
            stopped: false,
        }
    }
    pub(crate) fn push(&mut self, code: SelectedSourceRefusalCode) -> bool {
        if self.stopped {
            return false;
        }
        let len = usize::from(self.value.len);
        if len == MAX_CODES {
            self.terminal(SelectedSourceRefusalTermination::DepthLimit);
            return false;
        }
        self.value.codes[len] = code;
        self.value.len += 1;
        true
    }
    pub(crate) fn terminal(&mut self, termination: SelectedSourceRefusalTermination) {
        if self.stopped {
            return;
        }
        self.value.termination = termination;
        self.value.native_cause = None;
        self.stopped = true;
    }
    pub(crate) fn finish(self) -> SelectedSourceRefusal {
        self.value
    }
    pub(crate) fn path_frontier(&mut self, error: &ObservedPathFrontierError) {
        use SelectedSourceRefusalCode as Code;
        if !self.push(Code::PathEpoch) {
            return;
        }
        let ObservedPathFrontierError::Epoch(error) = error;
        let code = match error {
            PathObservationEpochError::DuplicateDemand(_) => Code::PathDuplicateDemand,
            PathObservationEpochError::ConflictingDemand(_) => Code::PathConflictingDemand,
            PathObservationEpochError::InputImagePathMismatch(_) => {
                Code::PathInputImagePathMismatch
            }
            PathObservationEpochError::OperationMismatch { .. } => Code::PathOperationMismatch,
        };
        if self.push(code) {
            self.terminal(SelectedSourceRefusalTermination::Classified);
        }
    }
    pub(crate) fn registry_file(&mut self, error: &RegistryFileError) {
        // Reject another descent before inspecting a native cause payload.
        if self.stopped {
            return;
        }
        if usize::from(self.value.len) == MAX_CODES {
            self.terminal(SelectedSourceRefusalTermination::DepthLimit);
            return;
        }
        use SelectedSourceRefusalCode as Code;
        let (code, cause, termination) = match error {
            RegistryFileError::InvalidFileUrl(..) => (
                Code::RegistryFileInvalidFileUrl,
                None,
                SelectedSourceRefusalTermination::Classified,
            ),
            RegistryFileError::UnsupportedUrl(..) => (
                Code::RegistryFileUnsupportedUrl,
                None,
                SelectedSourceRefusalTermination::Classified,
            ),
            RegistryFileError::MissingRegistryUrls(..) => (
                Code::RegistryFileMissingRegistryUrls,
                None,
                SelectedSourceRefusalTermination::OpaqueLeaf,
            ),
            RegistryFileError::MissingLockfileMode(..) => (
                Code::RegistryFileMissingLockfileMode,
                None,
                SelectedSourceRefusalTermination::OpaqueLeaf,
            ),
            RegistryFileError::MissingRequestGeneration(..) => (
                Code::RegistryFileMissingRequestGeneration,
                None,
                SelectedSourceRefusalTermination::OpaqueLeaf,
            ),
            RegistryFileError::RootModuleFiles(..) => (
                Code::RegistryFileRootModuleFiles,
                None,
                SelectedSourceRefusalTermination::OpaqueLeaf,
            ),
            RegistryFileError::MissingIoCapability => (
                Code::RegistryFileMissingIoCapability,
                None,
                SelectedSourceRefusalTermination::Classified,
            ),
            RegistryFileError::MissingChecksumInError { .. } => (
                Code::RegistryFileMissingChecksumInError,
                None,
                SelectedSourceRefusalTermination::Classified,
            ),
            RegistryFileError::Transport { cause, .. } => (
                Code::RegistryFileTransport,
                *cause,
                SelectedSourceRefusalTermination::OpaqueLeaf,
            ),
            RegistryFileError::ChecksumMismatch { .. } => (
                Code::RegistryFileChecksumMismatch,
                None,
                SelectedSourceRefusalTermination::Classified,
            ),
            RegistryFileError::LocalRead { cause, .. } => (
                Code::RegistryFileLocalRead,
                *cause,
                SelectedSourceRefusalTermination::OpaqueLeaf,
            ),
            RegistryFileError::InvalidLockfileExpectation { .. } => (
                Code::RegistryFileInvalidLockfileExpectation,
                None,
                SelectedSourceRefusalTermination::OpaqueLeaf,
            ),
        };
        if self.push(code) {
            let termination = if cause
                .is_some_and(|cause| !matches!(cause.detail, RegistryRefusalDetail::Opaque))
            {
                SelectedSourceRefusalTermination::Classified
            } else {
                termination
            };
            self.terminal(termination);
            self.value.native_cause = cause;
        }
    }
}
