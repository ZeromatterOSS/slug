//! Discriminants only; opaque messages never become semantic/API evidence.
use super::*;

const MAX_STEPS: usize = 32;

enum Cursor<'a> {
    Registration(&'a ModuleRegistrationExpansionError),
    RootPackage(&'a RootPackageLoadError),
    RepositoryPackage(&'a crate::bzl_module::RepositoryPackageLoadError),
    Plan(&'a Plan),
    LabelPath(&'a LabelPath),
    ExternalBzlRoute(&'a ExternalBzlRouteError),
    Loading(Node<'a>),
}

impl ModuleRegistrationExpansionError {
    /// Bounded static classification, separate from presentation and equality.
    #[doc(hidden)]
    #[cfg(any(test, feature = "native-probe-observer"))]
    pub fn terminal_leaf(&self) -> &'static str {
        classify(Cursor::Registration(self))
    }
}

fn classify(mut cursor: Cursor<'_>) -> &'static str {
    // The registration envelope is itself the first visited node. Every causal
    // wrapper, including Factored and Child, consumes another bounded step.
    for _ in 0..MAX_STEPS {
        cursor = match cursor {
            Cursor::Registration(error) => match error.kind() {
                Registration::CanonicalRoute(error) => Cursor::Loading(Node::Load(error)),
                Registration::RootPackage(error) => Cursor::RootPackage(error),
                Registration::CanonicalPackage(error) => Cursor::RepositoryPackage(error),
                Registration::Selected(_) => return "registration_selected_opaque",
                Registration::Configuration(_) => return "registration_configuration_opaque",
                Registration::RootMapping(_) => return "registration_root_mapping_opaque",
                Registration::RootMappingUnavailable => {
                    return "registration_root_mapping_unavailable";
                }
                Registration::Parse(_) => return "registration_parse_opaque",
                Registration::RowOverflow => return "registration_row_overflow",
                Registration::RootSubtree(_) => return "registration_root_subtree_opaque",
                Registration::CanonicalSubtree(_) => return "registration_canonical_subtree_opaque",
                Registration::MissingTarget(_) => return "registration_missing_target",
            },
            Cursor::RootPackage(error) => match error.diagnostic_view() {
                RootPackageLoadDiagnostic::Leaf(_) => return "root_package_leaf_opaque",
                RootPackageLoadDiagnostic::ExternalRoute { error, .. } => {
                    Cursor::Loading(Node::RootLoad(error))
                }
                RootPackageLoadDiagnostic::Bzl { error, .. } => {
                    Cursor::Loading(Node::RootBzl(error))
                }
                RootPackageLoadDiagnostic::ExternalBzl { error, .. } => {
                    Cursor::Loading(Node::ExternalBzl(error))
                }
            },
            Cursor::RepositoryPackage(error) => match error.registration_diagnostic() {
                RepositoryPackageLoadDiagnostic::Leaf(_) => return "repository_package_leaf_opaque",
                RepositoryPackageLoadDiagnostic::AttemptLoading(_) => {
                    return "repository_package_loading_opaque";
                }
                RepositoryPackageLoadDiagnostic::Bzl { error, .. } => {
                    Cursor::Loading(Node::ExternalBzl(error))
                }
            },
            Cursor::Plan(error) => return plan_leaf(error),
            Cursor::LabelPath(error) => return label_path_leaf(error),
            Cursor::ExternalBzlRoute(error) => match error {
                ExternalBzlRouteError::Load(error) => Cursor::Loading(Node::Load(error)),
                ExternalBzlRouteError::Compute(_) => return "external_bzl_route_compute_opaque",
                ExternalBzlRouteError::Observation(_) => {
                    return "external_bzl_route_observation_opaque";
                }
                ExternalBzlRouteError::Admission(_) => return "external_bzl_route_admission_opaque",
            },
            Cursor::Loading(node) => Cursor::Loading(match node {
                Node::RootLoad(error) => match error.diagnostic_view() {
                    RootLoadDiagnostic::Root(_) => return "root_route_opaque",
                    RootLoadDiagnostic::Mapping(error) => Node::ApparentMapping(error),
                    RootLoadDiagnostic::LoadRoute(error) => Node::Load(error),
                    RootLoadDiagnostic::RootCompute(_) => return "root_route_compute_opaque",
                    RootLoadDiagnostic::MappingCompute(_) => return "root_mapping_compute_opaque",
                    RootLoadDiagnostic::LoadRouteCompute(_) => {
                        return "root_load_route_compute_opaque";
                    }
                    RootLoadDiagnostic::Projection(_) => return "root_route_projection_opaque",
                },
                Node::ApparentMapping(error) => match &error.kind {
                    Mapping::RootApparent => return "mapping_root_apparent",
                    Mapping::Missing { .. } => return "mapping_missing",
                    Mapping::ContextMismatch { .. } => return "mapping_context_mismatch",
                    Mapping::RootMapping(_) => return "mapping_root_mapping_opaque",
                    Mapping::RootMappingCompute(_) => return "mapping_root_compute_opaque",
                    Mapping::RouteCompute(_) => return "mapping_route_compute_opaque",
                    Mapping::Route(error) => Node::Route(error),
                },
                Node::Load(error) => match &error.kind {
                    Load::Route(error) => Node::Route(error),
                    Load::RouteCompute(_) => return "load_route_compute_opaque",
                    Load::EffectCompute(_) => return "load_effect_compute_opaque",
                    Load::Effect(error) => Node::Effect(error),
                    Load::Projection(_) => return "load_projection_opaque",
                },
                Node::Effect(error) => match error {
                    Effect::Certificate(error) => Node::Certificate(error),
                    Effect::Compute(_) => return "effect_compute_opaque",
                    Effect::MissingOrdinal { .. } => return "effect_missing_ordinal",
                    Effect::UnsupportedDefiningLabel { .. } => {
                        return "effect_unsupported_defining_label";
                    }
                    Effect::HostBzl { .. } => return "effect_host_bzl_opaque",
                    Effect::Projection { .. } => return "effect_projection_opaque",
                    Effect::HostInput { .. } => return "effect_host_input_opaque",
                    Effect::Path { error, .. } => {
                        cursor = Cursor::Plan(error);
                        continue;
                    }
                    Effect::LabelPathRoute { error, .. } => Node::Load(error),
                    Effect::LabelPath { error, .. } => {
                        cursor = Cursor::LabelPath(error);
                        continue;
                    }
                    Effect::Invocation { .. } => return "effect_invocation_opaque",
                    Effect::Result { .. } => return "effect_result_type_opaque",
                },
                Node::Route(error) => match &error.kind {
                    Route::Generated { error, .. } => Node::Generated(error),
                    Route::GeneratedCompute { .. } => return "route_generated_compute_opaque",
                    Route::Missing { .. } => return "route_missing",
                    Route::BuiltinCompute(_) => return "route_builtin_compute_opaque",
                    Route::SelectedCompute(_) => return "route_selected_compute_opaque",
                    Route::Builtin(_) => return "route_builtin_opaque",
                    Route::Selected(_) => return "route_selected_opaque",
                },
                Node::Generated(error) => match &error.kind {
                    Generated::Demand(_) => return "generated_demand_opaque",
                    Generated::DemandCompute(_) => return "generated_demand_compute_opaque",
                    Generated::Loading(error) => Node::Certificate(error),
                    Generated::LoadingCompute(_) => return "generated_loading_compute_opaque",
                    Generated::Missing {} => return "generated_missing",
                    Generated::Duplicate { .. } => return "generated_duplicate",
                },
                Node::Certificate(error) => match &error.0 {
                    Certificate::Pure(error) => Node::Pure(error),
                    Certificate::InnatePure(error) => Node::Innate(error),
                    Certificate::Compute(_) => return "certificate_compute_opaque",
                    Certificate::Instantiation { .. } => return "certificate_instantiation_opaque",
                    Certificate::InnateInstantiation { .. } => {
                        return "certificate_innate_instantiation_opaque";
                    }
                    Certificate::Validation { .. } => return "certificate_validation_opaque",
                    Certificate::InnateValidation { .. } => {
                        return "certificate_innate_validation_opaque";
                    }
                },
                Node::Pure(error) => match error {
                    Pure::Factored { error, .. } => Node::Pure(error),
                    Pure::Inputs(_) => return "pure_inputs_opaque",
                    Pure::Compute(_) => return "pure_compute_opaque",
                    Pure::AfterInputs { .. } => return "pure_after_inputs_opaque",
                    Pure::SourceOperational { .. } => return "pure_source_operational_opaque",
                },
                Node::Innate(error) => match error {
                    Innate::Inputs(_) => return "innate_inputs_opaque",
                    Innate::Compute(_) => return "innate_compute_opaque",
                    Innate::Label(_) => return "innate_label_opaque",
                    Innate::Export(_) => return "innate_export_opaque",
                    Innate::Call(_) => return "innate_call_opaque",
                    Innate::Drift => return "innate_drift",
                    Innate::LoadRoute(error) => Node::Load(error),
                    Innate::RootBzl(error) => Node::RootBzl(error),
                    Innate::ExternalBzl(error) => Node::ExternalBzl(error),
                },
                Node::RootBzl(error) => match error {
                    HostBzlModuleError::Child { error, .. } => Node::RootBzl(error),
                    HostBzlModuleError::Parse { .. } => return "host_bzl_parse_opaque",
                    HostBzlModuleError::Freeze { .. } => return "host_bzl_freeze_opaque",
                    HostBzlModuleError::Cycle(_) => return "host_bzl_cycle",
                    HostBzlModuleError::Source(_) => return "host_bzl_source_opaque",
                    HostBzlModuleError::Input(_) => return "host_bzl_input_opaque",
                    HostBzlModuleError::LoadLabel { .. } => return "host_bzl_load_label_opaque",
                    HostBzlModuleError::Evaluation(_) => return "host_bzl_evaluation_opaque",
                },
                Node::ExternalBzl(error) => match error {
                    ExternalBzlModuleError::Child { error, .. } => Node::ExternalBzl(error),
                    ExternalBzlModuleError::RepositoryMapping { error, .. }
                    | ExternalBzlModuleError::Route { error, .. } => {
                        cursor = Cursor::ExternalBzlRoute(error);
                        continue;
                    }
                    ExternalBzlModuleError::SourceCompute { .. } => {
                        return "external_bzl_source_compute_opaque";
                    }
                    ExternalBzlModuleError::Parse { .. } => return "external_bzl_parse_opaque",
                    ExternalBzlModuleError::Evaluation { .. } => {
                        return "external_bzl_evaluation_opaque";
                    }
                    ExternalBzlModuleError::Freeze { .. } => return "external_bzl_freeze_opaque",
                    ExternalBzlModuleError::Absent { .. } => return "external_bzl_absent",
                    ExternalBzlModuleError::Encoding { .. } => return "external_bzl_encoding",
                    ExternalBzlModuleError::Cycle(_) => return "external_bzl_cycle",
                    ExternalBzlModuleError::DependencyCycle(_) => {
                        return "external_bzl_dependency_cycle";
                    }
                    ExternalBzlModuleError::Source { .. } => return "external_bzl_source_opaque",
                    ExternalBzlModuleError::SourceObservation { .. } => {
                        return "external_bzl_source_observation_opaque";
                    }
                    ExternalBzlModuleError::LoadLabel { .. } => {
                        return "external_bzl_load_label_opaque";
                    }
                    ExternalBzlModuleError::IncompatibleNeeds { .. } => {
                        return "external_bzl_incompatible_needs_opaque";
                    }
                },
            }),
        };
    }
    "registration_depth_limit"
}

fn plan_leaf(error: &Plan) -> &'static str {
    match error {
        Plan::InvalidPath(_) => "plan_invalid_path",
        Plan::RepeatedPath(_) => "plan_repeated_path",
        Plan::ConflictingPath(_) => "plan_conflicting_path",
        Plan::SymlinkArchiveMix => "plan_symlink_archive_mix",
        Plan::TooManyFilesystemOperations => "plan_filesystem_operation_limit",
        Plan::SymlinkDeleteMix => "plan_symlink_delete_mix",
        Plan::EmptyArchiveUrls => "plan_archive_urls_empty",
        Plan::TooManyArchiveUrls => "plan_archive_url_limit",
        Plan::InvalidArchiveUrl => "plan_archive_url_invalid",
        Plan::InvalidArchiveStripPrefix => "plan_archive_strip_prefix_invalid",
        Plan::MissingArchiveChecksum => "plan_archive_checksum_missing",
        Plan::ConflictingArchiveChecksum => "plan_archive_checksum_conflicting",
        Plan::InvalidArchiveChecksum => "plan_archive_checksum_invalid",
        Plan::UnsupportedArchiveChecksum => "plan_archive_checksum_unsupported",
    }
}

fn label_path_leaf(error: &LabelPath) -> &'static str {
    match error {
        LabelPath::RootRepositoryMismatch(_) => "label_path_root_mismatch",
        LabelPath::ExternalRepositoryMismatch(_, _) => "label_path_external_mismatch",
        LabelPath::PackageAbsent => "label_path_package_absent",
        LabelPath::PackageDeleted => "label_path_package_deleted",
        LabelPath::PackageIgnored => "label_path_package_ignored",
        LabelPath::InvalidPackageName(_) => "label_path_package_name_opaque",
        LabelPath::RootPackageLookup(_) => "label_path_root_lookup_opaque",
        LabelPath::ExternalPackageLookup(_) => "label_path_external_lookup_opaque",
        LabelPath::BuiltinCatalog => "label_path_builtin_catalog",
        LabelPath::Materialization(_) => "label_path_materialization_opaque",
        LabelPath::MaterializationCompute(_) => "label_path_materialization_compute_opaque",
        LabelPath::InvalidMaterializedPath => "label_path_materialized_path_invalid",
        LabelPath::NonUnicodePhysicalPath => "label_path_non_unicode_physical_path",
    }
}
