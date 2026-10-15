/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

#[doc(hidden)]
pub mod analysis_fragments;
mod artifact_root;
#[doc(hidden)]
pub use artifact_root::alloc_artifact_root;
pub mod build_info_transform;
mod resource_set;
#[doc(hidden)]
pub use artifact_root::alloc_frozen_artifact_root;
#[doc(hidden)]
pub use artifact_root::artifact_root_value;
pub use artifact_root::artifact_root_value_ref;
pub mod attrs;
mod builtin_restriction;
pub(crate) mod bzl_environment;
pub use bzl_environment::BzlEnvironmentSourceBinding;
pub use bzl_environment::BzlModuleSourceContext;
pub mod bzl_module;
mod bzl_visibility;
mod canonical_repository_load_route;
#[cfg(test)]
mod canonical_repository_load_route_tests;
mod canonical_repository_mapping;
mod canonical_repository_route;
#[cfg(test)]
mod canonical_repository_route_tests;
mod cc_artifact_naming;
mod cc_common;
pub mod cc_compile_action;
pub mod cc_compile_outputs;
pub mod cc_context;
mod cc_features;
mod cc_header_info;
mod cc_variables;
mod dotted_version;
mod evaluator_roots;
pub use dotted_version::alloc_frozen_dotted_version;
pub use dotted_version::dotted_version_ref;
pub mod function_owner;
pub use cc_variables::CcVariablesHeader;
#[doc(hidden)]
pub use cc_variables::CcVariablesView;
#[doc(hidden)]
pub use cc_variables::alloc_frozen_cc_variables;
pub use cc_variables::cc_variables_header;
#[doc(hidden)]
pub use cc_variables::cc_variables_value;
#[doc(hidden)]
pub use cc_variables::is_cc_variables_value;
pub use cc_variables::snapshot_cc_variables_seed_bounded;
pub use cc_variables::visit_cc_variables_locals;
pub mod package_features;
pub use cc_common::CcModuleApi;
pub use cc_common::cc_common_api;
#[doc(hidden)]
pub use cc_features::alloc_frozen_cc_feature_configuration;
#[doc(hidden)]
pub use cc_features::alloc_frozen_cc_toolchain_features;
pub use cc_features::cc_feature_configuration_ref;
#[doc(hidden)]
pub use cc_features::cc_feature_configuration_value;
pub use cc_features::cc_toolchain_features_ref;
#[doc(hidden)]
pub use cc_features::cc_toolchain_features_value;
mod cycle_detector;
mod dependency_cycle;
#[doc(hidden)]
pub use dependency_cycle::compute_selected_label_source;
mod external_subtree_package_set;
pub mod file_discovery;
mod generated_repository_definition;
pub mod glob;
pub mod globals;
mod host_glob;
mod host_package_inventory;
#[cfg(test)]
mod host_package_inventory_tests;
pub mod keys;
pub mod load_label;
mod source_table;
mod module_extension;
mod module_extension_innate_repository;
mod module_extension_repository_file_effect;
mod module_extension_repository_instantiation;
mod module_extension_repository_rule;
mod module_extension_repository_validation;
pub mod package;
#[doc(hidden)]
pub mod provider;
mod py_internal;
mod registration_diagnostic;
mod registration_expansion;
#[cfg(test)]
mod registration_expansion_tests;
#[doc(hidden)]
pub mod repository_rule_context;
mod root_repository_load_route;
mod root_subtree_package_set;
pub mod rule_outputs;
mod starlark_label;
pub use starlark_label::defining_module_identity;
pub use starlark_label::defining_module_source_context;
pub use starlark_label::defining_module_source_manifest;
mod subrule;
#[doc(hidden)]
pub mod subrule_invocation;
#[doc(hidden)]
pub use subrule::SubruleIdentity;
mod coverage_common;
mod testing_bootstrap;
#[cfg(test)]
mod testing_bootstrap_tests;
mod transition;
pub mod visibility;

pub use attrs::AttributeKind;
pub use attrs::AttributeProvenance;
pub use attrs::AttributeQueryValue;
pub use attrs::AttributeSchema;
pub use attrs::AttributeValue;
pub use attrs::CoercedAttributeValue;
pub use attrs::FileAdmissibility;
pub use attrs::NativeAttributeOrder;
pub use attrs::NativeAttributePolicy;
pub use attrs::NativeAttributeSchema;
pub use attrs::NativeAttributeValue;
pub use attrs::NativeRuleAttributes;
pub use attrs::NativeRuleClass;
pub use bzl_module::BuildFileCompanion;
pub use bzl_module::BzlLoadManifest;
pub use bzl_module::BzlModuleEvaluator;
pub use bzl_module::BzlModuleIdentity;
pub use bzl_module::BzlModuleSourceProvenance;
pub use bzl_module::EvaluatedBzlModule;
#[doc(hidden)]
pub use bzl_module::ObservedRepositoryPackageLoad;
#[doc(hidden)]
pub use bzl_module::ObservedRootPackageLoad;
pub use bzl_module::RepositoryPackageLoadError;
pub use bzl_module::RepositoryPackageLoadKey;
#[doc(hidden)]
pub use bzl_module::RepositoryPackageLoadObservationKey;
pub use bzl_module::RootPackageLoadError;
pub use bzl_module::RootPackageLoadKey;
#[doc(hidden)]
pub use bzl_module::RootPackageLoadObservationKey;
pub use bzl_module::discover_build_file_companion;
#[doc(hidden)]
pub use canonical_repository_load_route::HostCanonicalRepositoryLoadRoute;
#[doc(hidden)]
pub use canonical_repository_load_route::HostCanonicalRepositoryLoadRouteError;
#[doc(hidden)]
pub use canonical_repository_load_route::HostCanonicalRepositoryLoadRouteKey;
#[doc(hidden)]
pub use canonical_repository_load_route::HostCanonicalRepositoryLoadRouteObservationError;
#[doc(hidden)]
pub use canonical_repository_load_route::HostCanonicalRepositoryLoadRouteObservationKey;
#[doc(hidden)]
pub use canonical_repository_load_route::HostCanonicalRepositoryLoadRouteOutcome;
#[doc(hidden)]
pub use canonical_repository_load_route::ObservedHostCanonicalRepositoryLoadRoute;
#[doc(hidden)]
pub use canonical_repository_mapping::HostCanonicalRepositoryApparentMapping;
#[doc(hidden)]
pub use canonical_repository_mapping::HostCanonicalRepositoryApparentMappingError;
#[doc(hidden)]
pub use canonical_repository_mapping::HostCanonicalRepositoryApparentMappingErrorDisposition;
#[doc(hidden)]
pub use canonical_repository_mapping::HostCanonicalRepositoryApparentMappingKey;
#[doc(hidden)]
pub use canonical_repository_mapping::HostCanonicalRepositoryApparentMappingObservationError;
#[doc(hidden)]
pub use canonical_repository_mapping::HostCanonicalRepositoryApparentMappingObservationKey;
#[doc(hidden)]
pub use canonical_repository_mapping::ObservedHostCanonicalRepositoryApparentMapping;
#[doc(hidden)]
pub use canonical_repository_route::HostCanonicalRepositoryRouteError;
#[doc(hidden)]
pub use canonical_repository_route::HostCanonicalRepositoryRouteKey;
#[doc(hidden)]
pub use canonical_repository_route::HostCanonicalRepositoryRouteObservationError;
#[doc(hidden)]
pub use canonical_repository_route::HostCanonicalRepositoryRouteObservationKey;
#[doc(hidden)]
pub use canonical_repository_route::HostCanonicalRepositoryRouteOutcome;
#[doc(hidden)]
pub use canonical_repository_route::ObservedHostCanonicalRepositoryRoute;
pub use cc_header_info::alloc_frozen_cc_header_info;
pub use cc_header_info::cc_header_info;
pub use cc_header_info::cc_header_info_ref;
pub use cycle_detector::bzl_load_cycle_detector;
#[doc(hidden)]
pub use dependency_cycle::compute_runtime_source_profile;
#[doc(hidden)]
pub use dependency_cycle::compute_runtime_source_requests;
pub use dependency_cycle::compute_selected_data_label_requests;
pub use dependency_cycle::compute_selected_data_label_sources;
pub use dependency_cycle::compute_selected_guest_namespace_facts;
#[doc(hidden)]
pub use dependency_cycle::compute_selected_preparation_owner;
#[doc(hidden)]
pub use dependency_cycle::compute_selected_scratch_prefix;
#[doc(hidden)]
pub use dependency_cycle::compute_selected_tool_source_requests;
#[doc(hidden)]
pub use dependency_cycle::compute_selected_tool_sources;
#[doc(hidden)]
pub use external_subtree_package_set::ExternalSubtreePackageSet;
#[doc(hidden)]
pub use external_subtree_package_set::ExternalSubtreePackageSetError;
#[doc(hidden)]
pub use external_subtree_package_set::ExternalSubtreePackageSetErrorKind;
#[doc(hidden)]
pub use external_subtree_package_set::ExternalSubtreePackageSetKey;
#[doc(hidden)]
pub use external_subtree_package_set::ExternalSubtreePackageSetObservationKey;
#[doc(hidden)]
pub use external_subtree_package_set::ObservedExternalSubtreePackageSet;
pub use glob::GlobSpec;
pub use glob::PackageListing;
pub use host_package_inventory::HostPackageInventory;
pub use host_package_inventory::HostPackageInventoryErrorRef;
pub use host_package_inventory::HostPackageInventoryKey;
#[doc(hidden)]
pub use host_package_inventory::HostPackageInventoryObservationError;
#[doc(hidden)]
pub use host_package_inventory::HostPackageInventoryObservationKey;
#[doc(hidden)]
pub use host_package_inventory::ObservedHostPackageInventory;
pub use module_extension::guest_namespace::GuestDirectoryCheck;
pub use module_extension::guest_namespace::GuestLookupError;
pub use module_extension::guest_namespace::GuestNamespaceBinding;
pub use module_extension::guest_namespace::GuestNamespaceFactsError;
pub use module_extension::guest_namespace::GuestNamespaceMetadata;
pub use module_extension::guest_namespace::GuestNamespaceNode;
pub use module_extension::guest_namespace::GuestNamespaceNodeKind;
pub use module_extension::guest_namespace::GuestNamespaceOperation;
pub use module_extension::guest_namespace::ObservedGuestNamespaceFacts;
pub use module_extension::guest_namespace::SelectedGuestNamespaceFactsKey;
#[doc(hidden)]
pub use module_extension::runtime_sources::ObservedRuntimeSourceProfile;
#[doc(hidden)]
pub use module_extension::runtime_sources::ObservedRuntimeSourceRequests;
#[doc(hidden)]
pub use module_extension::runtime_sources::RuntimeSourceImageBinding;
#[doc(hidden)]
pub use module_extension::runtime_sources::RuntimeSourceProfileKey;
#[doc(hidden)]
pub use module_extension::runtime_sources::RuntimeSourceRequest;
#[doc(hidden)]
pub use module_extension::runtime_sources::RuntimeSourceRequestsKey;
#[doc(hidden)]
pub use module_extension::runtime_sources::RuntimeSourceTotals;
#[doc(hidden)]
pub use module_extension::runtime_sources::RuntimeSourcesError;
pub use module_extension::selected_label_data::ObservedSelectedDataLabelRequests;
pub use module_extension::selected_label_data::ObservedSelectedDataLabelSources;
pub use module_extension::selected_label_data::SelectedDataLabelBinding;
pub use module_extension::selected_label_data::SelectedDataLabelError;
pub use module_extension::selected_label_data::SelectedDataLabelRequest;
pub use module_extension::selected_label_data::SelectedDataLabelRequestsKey;
pub use module_extension::selected_label_data::SelectedDataLabelSource;
pub use module_extension::selected_label_data::SelectedDataLabelSourcesKey;
#[doc(hidden)]
pub use module_extension::selected_scratch::HostSelectedExtensionScratchPrefixKey;
#[doc(hidden)]
pub use module_extension::selected_scratch::ObservedSelectedExtensionScratchPrefix;
#[doc(hidden)]
pub use module_extension::selected_scratch::SelectedScratchLabelSource;
#[doc(hidden)]
pub use module_extension::selected_scratch::SelectedScratchPrefixError;
#[doc(hidden)]
pub use module_extension::selected_scratch::SelectedScratchPreparationDeadline;
#[doc(hidden)]
pub use module_extension::selected_source_preparation::ObservedSelectedPreparationOwner;
#[doc(hidden)]
pub use module_extension::selected_source_preparation::SelectedPreparationOwnerError;
#[doc(hidden)]
pub use module_extension::selected_source_preparation::SelectedPreparationOwnerKey;
#[doc(hidden)]
pub use module_extension::selected_source_preparation::SelectedPreparationOwnerRefusalSite;
#[doc(hidden)]
pub use module_extension::selected_tools::ObservedSelectedExtensionToolSourceRequests;
#[doc(hidden)]
pub use module_extension::selected_tools::ObservedSelectedExtensionToolSources;
#[doc(hidden)]
pub use module_extension::selected_tools::ObservedSelectedToolSource;
#[doc(hidden)]
pub use module_extension::selected_tools::SelectedExtensionToolSourceRequestsKey;
#[doc(hidden)]
pub use module_extension::selected_tools::SelectedExtensionToolSourcesKey;
#[doc(hidden)]
pub use module_extension::selected_tools::SelectedToolSourceRequest;
#[doc(hidden)]
pub use module_extension::selected_tools::SelectedToolSourcesError;
#[doc(hidden)]
pub use module_extension_repository_file_effect::HostSelectedRepositoryFileEffect;
#[doc(hidden)]
pub use module_extension_repository_file_effect::HostSelectedRepositoryFileEffectError;
#[doc(hidden)]
pub use module_extension_repository_file_effect::HostSelectedRepositoryFileEffectHostBzlError;
#[doc(hidden)]
pub use module_extension_repository_file_effect::HostSelectedRepositoryFileEffectKey;
#[doc(hidden)]
pub use module_extension_repository_file_effect::HostSelectedRepositoryFileEffectObservationError;
#[doc(hidden)]
pub use module_extension_repository_file_effect::HostSelectedRepositoryFileEffectObservationKey;
#[doc(hidden)]
pub use module_extension_repository_file_effect::ObservedHostSelectedRepositoryFileEffect;
#[doc(hidden)]
pub use module_extension_repository_validation::HostGeneratedRepositoryMapping;
#[doc(hidden)]
pub use module_extension_repository_validation::HostSelectedExtensionOwnerCertificate;
#[doc(hidden)]
pub use module_extension_repository_validation::HostSelectedExtensionOwnerCertificateError;
#[doc(hidden)]
pub use module_extension_repository_validation::HostSelectedExtensionOwnerCertificateKey;
#[doc(hidden)]
pub use module_extension_repository_validation::HostSelectedExtensionOwnerCertificateObservationError;
#[doc(hidden)]
pub use module_extension_repository_validation::HostSelectedExtensionOwnerCertificateObservationKey;
#[doc(hidden)]
pub use module_extension_repository_validation::HostValidatedGeneratedRepositorySpecs;
#[doc(hidden)]
pub use module_extension_repository_validation::HostValidatedGeneratedRepositorySpecsError;
#[doc(hidden)]
pub use module_extension_repository_validation::HostValidatedGeneratedRepositorySpecsOutcome;
#[doc(hidden)]
pub use module_extension_repository_validation::HostValidatedModuleExtensionRepositoriesKey;
#[doc(hidden)]
pub use module_extension_repository_validation::HostValidatedModuleExtensionRepositoriesObservationError;
#[doc(hidden)]
pub use module_extension_repository_validation::HostValidatedModuleExtensionRepositoriesObservationKey;
#[doc(hidden)]
pub use module_extension_repository_validation::ObservedHostSelectedExtensionOwnerCertificate;
#[doc(hidden)]
pub use module_extension_repository_validation::ObservedHostValidatedGeneratedRepositorySpecs;
#[doc(hidden)]
pub use package::AliasActual;
pub use package::LegacyLoadedPackage;
pub use package::LoadedPackage;
pub use package::NativeTargetAttributes;
#[doc(hidden)]
pub use package::PackageEvaluation;
pub use package::PackageTarget;
pub use package::PackageTargetKind;
pub use package::RuleCapability;
pub use package::TestMetadata;
pub use package::TestRuleKind;
pub use package::TestSuiteMembership;
pub use provider::AnalysisBuiltinCallableApi;
pub use provider::alloc_frozen_analysis_builtin_callable;
pub use provider::alloc_frozen_template_variable_info;
pub use provider::analysis_builtin_callable_api;
pub use provider::template_variable_info;
pub use provider::template_variable_info_ref;
pub use registration_expansion::CommandRegistrationExpansionKey;
#[doc(hidden)]
pub use registration_expansion::CommandRegistrationExpansionObservationKey;
pub use registration_expansion::ModuleRegistrationAmbiguity;
pub use registration_expansion::ModuleRegistrationExpansion;
pub use registration_expansion::ModuleRegistrationExpansionError;
pub use registration_expansion::ModuleRegistrationExpansionErrorKind;
pub use registration_expansion::ModuleRegistrationExpansionKey;
#[doc(hidden)]
pub use registration_expansion::ModuleRegistrationExpansionObservationError;
#[doc(hidden)]
pub use registration_expansion::ModuleRegistrationExpansionObservationKey;
pub use registration_expansion::ModuleRegistrationFamily;
#[doc(hidden)]
pub use registration_expansion::ObservedCommandRegistrationExpansion;
#[doc(hidden)]
pub use registration_expansion::ObservedModuleRegistrationExpansion;
#[doc(hidden)]
pub use root_repository_load_route::HostRootRepositoryLoadRoute;
#[doc(hidden)]
pub use root_repository_load_route::HostRootRepositoryLoadRouteError;
#[doc(hidden)]
pub use root_repository_load_route::HostRootRepositoryLoadRouteKey;
#[doc(hidden)]
pub use root_repository_load_route::HostRootRepositoryLoadRouteObservationError;
#[doc(hidden)]
pub use root_repository_load_route::HostRootRepositoryLoadRouteObservationKey;
#[doc(hidden)]
pub use root_repository_load_route::HostRootRepositoryLoadRouteOutcome;
#[doc(hidden)]
pub use root_repository_load_route::ObservedHostRootRepositoryLoadRoute;
#[doc(hidden)]
pub use root_subtree_package_set::ObservedRootSubtreePackageSet;
#[doc(hidden)]
pub use root_subtree_package_set::RootSubtreePackageSet;
#[doc(hidden)]
pub use root_subtree_package_set::RootSubtreePackageSetError;
#[doc(hidden)]
pub use root_subtree_package_set::RootSubtreePackageSetKey;
#[doc(hidden)]
pub use root_subtree_package_set::RootSubtreePackageSetObservationKey;
pub use slug_bzlmod_v2::SourcePreparationNeeds as LoadingPreparationNeeds;
pub use slug_bzlmod_v2::SourcePreparationOutcome as LoadingPreparationOutcome;
pub use subrule::ConfiguredDependencyAttribute;
pub use subrule::ConfiguredDependencyDefault;
pub use visibility::PackageGroupContents;
pub use visibility::RestrictedVisibility;
pub use visibility::RuleVisibility;
pub use visibility::VisibilitySource;
