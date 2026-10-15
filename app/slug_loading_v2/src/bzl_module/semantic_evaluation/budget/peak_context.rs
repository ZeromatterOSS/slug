//! Static accounting ownership, formatted only by a contextual refusal.
// No source/caller identity, semantic input or retained owner state is recorded.
#[cfg(test)]
pub(crate) const MAX_LABEL_BYTES: usize = 48;

#[derive(Debug, Clone, Copy)]
pub(crate) enum PeakOwner {
    BuiltinPreparation,
    SemanticInputs,
    FunctionOwner,
    StableOwner,
    EnvironmentSeed,
    BuiltinCatalog,
    CatalogParser,
    ExportSelection,
    SourceProof,
    EnvironmentReferences,
}
impl PeakOwner {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::BuiltinPreparation => "builtin-preparation",
            Self::SemanticInputs => "semantic-inputs",
            Self::FunctionOwner => "function-owner",
            Self::StableOwner => "stable-owner",
            Self::EnvironmentSeed => "environment-seed",
            Self::BuiltinCatalog => "builtin-catalog",
            Self::CatalogParser => "catalog-parser",
            Self::ExportSelection => "export-selection",
            Self::SourceProof => "source-proof",
            Self::EnvironmentReferences => "environment-references",
        }
    }
    #[cfg(test)]
    pub(crate) const ALL: &'static [Self] = &[
        Self::BuiltinPreparation,
        Self::SemanticInputs,
        Self::FunctionOwner,
        Self::StableOwner,
        Self::EnvironmentSeed,
        Self::BuiltinCatalog,
        Self::CatalogParser,
        Self::ExportSelection,
        Self::SourceProof,
        Self::EnvironmentReferences,
    ];
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PeakPhase {
    InputValidation,
    LocalRowForecast,
    CompleteRowForecast,
    InputHeader,
    InputDepth,
    SourceProofHash,
    SourceProofMetadata,
    BuiltinGraph,
    RowCandidateHash,
    RowCollision,
    UniqueRow,
    RowIndexGrowth,
    RowArray,
    ChildArray,
    FacadePath,
    FacadeLoadsArray,
    FacadeLoad,
    FacadeIdentity,
    InputPhysical,
    InputTransfer,
    SeedHash,
    CatalogPreconstructHash,
    CatalogFinalHash,
    CatalogBeforeParse,
    CatalogBeforeLoadCopies,
    ExportFacade,
    ExportGuards,
    SourceProofPrecopy,
    NativeTransfer,
    StablePrecopy,
    ReferenceTables,
    ReferenceMemoHit,
    ReferenceDescriptorStorage,
    ReferencePayloadStorage,
    ReferencePayloadCompare,
    ReferencePayloadCanonical,
    ReferenceDescriptorCompare,
    ReferenceDescriptorCanonical,
    ReferenceMemoCommit,
    ReferencePhysicalFinal,
    ReferenceFinal,
}
impl PeakPhase {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::InputValidation => "input-validation",
            Self::LocalRowForecast => "local-row-forecast",
            Self::CompleteRowForecast => "complete-row-forecast",
            Self::InputHeader => "input-header",
            Self::InputDepth => "input-depth",
            Self::SourceProofHash => "source-proof-hash",
            Self::SourceProofMetadata => "source-proof-metadata",
            Self::BuiltinGraph => "builtin-graph",
            Self::RowCandidateHash => "row-candidate-hash",
            Self::RowCollision => "row-collision",
            Self::UniqueRow => "unique-row",
            Self::RowIndexGrowth => "row-index-growth",
            Self::RowArray => "row-array",
            Self::ChildArray => "child-array",
            Self::FacadePath => "facade-path",
            Self::FacadeLoadsArray => "facade-loads-array",
            Self::FacadeLoad => "facade-load",
            Self::FacadeIdentity => "facade-identity",
            Self::InputPhysical => "input-physical",
            Self::InputTransfer => "input-transfer",
            Self::SeedHash => "seed-hash",
            Self::CatalogPreconstructHash => "catalog-preconstruct-hash",
            Self::CatalogFinalHash => "catalog-final-hash",
            Self::CatalogBeforeParse => "catalog-before-parse",
            Self::CatalogBeforeLoadCopies => "catalog-before-load-copies",
            Self::ExportFacade => "export-facade",
            Self::ExportGuards => "export-guards",
            Self::SourceProofPrecopy => "source-proof-precopy",
            Self::NativeTransfer => "native-transfer",
            Self::StablePrecopy => "stable-precopy",
            Self::ReferenceTables => "reference-tables",
            Self::ReferenceMemoHit => "reference-memo-hit",
            Self::ReferenceDescriptorStorage => "reference-descriptor-storage",
            Self::ReferencePayloadStorage => "reference-payload-storage",
            Self::ReferencePayloadCompare => "reference-payload-compare",
            Self::ReferencePayloadCanonical => "reference-payload-canonical",
            Self::ReferenceDescriptorCompare => "reference-descriptor-compare",
            Self::ReferenceDescriptorCanonical => "reference-descriptor-canonical",
            Self::ReferenceMemoCommit => "reference-memo-commit",
            Self::ReferencePhysicalFinal => "reference-physical-final",
            Self::ReferenceFinal => "reference-final",
        }
    }
    #[cfg(test)]
    pub(crate) const ALL: &'static [Self] = &[
        Self::InputValidation,
        Self::LocalRowForecast,
        Self::CompleteRowForecast,
        Self::InputHeader,
        Self::InputDepth,
        Self::SourceProofHash,
        Self::SourceProofMetadata,
        Self::BuiltinGraph,
        Self::RowCandidateHash,
        Self::RowCollision,
        Self::UniqueRow,
        Self::RowIndexGrowth,
        Self::RowArray,
        Self::ChildArray,
        Self::FacadePath,
        Self::FacadeLoadsArray,
        Self::FacadeLoad,
        Self::FacadeIdentity,
        Self::InputPhysical,
        Self::InputTransfer,
        Self::SeedHash,
        Self::CatalogPreconstructHash,
        Self::CatalogFinalHash,
        Self::CatalogBeforeParse,
        Self::CatalogBeforeLoadCopies,
        Self::ExportFacade,
        Self::ExportGuards,
        Self::SourceProofPrecopy,
        Self::NativeTransfer,
        Self::StablePrecopy,
        Self::ReferenceTables,
        Self::ReferenceMemoHit,
        Self::ReferenceDescriptorStorage,
        Self::ReferencePayloadStorage,
        Self::ReferencePayloadCompare,
        Self::ReferencePayloadCanonical,
        Self::ReferenceDescriptorCompare,
        Self::ReferenceDescriptorCanonical,
        Self::ReferenceMemoCommit,
        Self::ReferencePhysicalFinal,
        Self::ReferenceFinal,
    ];
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum FirstLedger {
    Work,
    Local,
    Validation,
    Logical,
    Physical,
    Catalog,
    NativeRetained,
    AdmittedPhysical,
}
impl FirstLedger {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Work => "work",
            Self::Local => "local",
            Self::Validation => "validation",
            Self::Logical => "logical-facts",
            Self::Physical => "physical-forecast",
            Self::Catalog => "catalog-forecast",
            Self::NativeRetained => "native-retained",
            Self::AdmittedPhysical => "admitted-physical",
        }
    }
    #[cfg(test)]
    pub(crate) const ALL: &'static [Self] = &[
        Self::Work,
        Self::Local,
        Self::Validation,
        Self::Logical,
        Self::Physical,
        Self::Catalog,
        Self::NativeRetained,
        Self::AdmittedPhysical,
    ];
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PeakContext {
    owner: PeakOwner,
    phase: PeakPhase,
    first: FirstLedger,
}
impl PeakContext {
    pub(crate) const fn new(owner: PeakOwner, phase: PeakPhase, first: FirstLedger) -> Self {
        Self {
            owner,
            phase,
            first,
        }
    }
    pub(crate) const fn with_ledger(self, first: FirstLedger) -> Self {
        Self { first, ..self }
    }
    pub(crate) const fn with_phase(self, phase: PeakPhase) -> Self {
        Self { phase, ..self }
    }
    pub(crate) const fn owner(self) -> &'static str {
        self.owner.name()
    }
    pub(crate) const fn phase(self) -> &'static str {
        self.phase.name()
    }
    pub(crate) const fn first_ledger(self) -> &'static str {
        self.first.name()
    }
}
