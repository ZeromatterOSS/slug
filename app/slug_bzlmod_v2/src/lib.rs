/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

mod builtin_repository;
mod canonical_repository_route;
pub mod dice;
mod generated_repository_file_effect;
mod host_external_package_boundary;
mod host_file;
mod host_include;
mod host_lockfile;
mod host_module;
mod host_package;
mod host_package_boundary;
mod host_registry;
mod host_registry_inputs;
pub mod interim_module;
pub mod lockfile;
mod lockfile_v28;
#[cfg(test)]
mod lockfile_v28_tests;
pub mod module_eval;
pub mod module_patch;
mod module_version;
mod package_policy;
pub mod parser;
pub mod registry;
pub mod registry_dice;
mod repo_file;
mod repository_host_input;
mod repository_ignore;
pub mod resolution;
mod root_bootstrap;
mod selected_graph;
mod selected_repo_spec;
pub mod source_preparation;

pub use builtin_repository::BuiltinBazelToolsRouteIdentity;
pub use builtin_repository::BuiltinBazelToolsSnapshot;
pub use builtin_repository::BuiltinBazelToolsSourceFileError;
pub use builtin_repository::BuiltinBazelToolsSourceFileKey;
pub use builtin_repository::BuiltinBazelToolsSourceFileValue;
pub use builtin_repository::BuiltinBazelToolsSourceKind;
#[doc(hidden)]
pub use canonical_repository_route::HostCanonicalRepositoryRoute;
#[doc(hidden)]
pub use canonical_repository_route::HostCanonicalRepositoryRouteKind;
#[doc(hidden)]
pub use canonical_repository_route::HostCanonicalRepositoryRouteView;
#[doc(hidden)]
pub use canonical_repository_route::HostGeneratedRepositoryEffectSeed;
pub use dice::BzlmodCommandPolicyKey;
pub use dice::BzlmodDiceInputs;
pub use dice::BzlmodEnvironmentPolicyKey;
pub use dice::BzlmodExtensionDefinitionDigest;
pub use dice::BzlmodExtensionUsageDigest;
pub use dice::BzlmodGeneratedRepoSpecDigest;
pub use dice::BzlmodHiddenLockfileDigest;
pub use dice::BzlmodModuleFileDigest;
pub use dice::BzlmodRegistryModuleFileDigest;
pub use dice::BzlmodRegistryPolicyEntry;
pub use dice::BzlmodRegistrySourceSpecDigest;
pub use dice::BzlmodRepoMappingDigest;
pub use dice::BzlmodVisibleLockfileDigest;
pub use dice::LockfileMode;
pub use dice::ResolvedBzlmodGraphDiceKey;
pub use dice::digest_generated_repo_specs;
pub use dice::digest_included_module_files;
pub use dice::digest_module_extension_definitions;
pub use dice::digest_module_extension_usages;
pub use dice::digest_module_file_content;
pub use dice::digest_registry_module_files;
pub use dice::digest_registry_policy;
pub use dice::digest_registry_source_specs;
pub use dice::digest_repo_mapping_entries;
pub use dice::digest_repo_mappings;
#[doc(hidden)]
pub use generated_repository_file_effect::GeneratedRepositoryFileEffect;
#[doc(hidden)]
pub use generated_repository_file_effect::GeneratedRepositoryFileEffectPlan;
#[doc(hidden)]
pub use generated_repository_file_effect::GeneratedRepositoryFileEffectPlanBuilder;
#[doc(hidden)]
pub use generated_repository_file_effect::GeneratedRepositoryFileEffectPlanError;
pub use host_external_package_boundary::HostExternalPackageBoundary;
pub use host_external_package_boundary::HostExternalPackageBoundaryError;
pub use host_external_package_boundary::HostExternalPackageBoundaryKey;
pub use host_external_package_boundary::HostExternalPackageBoundaryKind;
#[doc(hidden)]
pub use host_external_package_boundary::HostExternalPackageBoundaryObservationKey;
#[doc(hidden)]
pub use host_external_package_boundary::ObservedHostExternalPackageBoundary;
#[doc(hidden)]
pub use host_module::HostRepositorySourceCapability;
#[doc(hidden)]
pub use host_module::HostRepositorySourceCapabilitySource;
#[doc(hidden)]
pub use host_module::ObservedRootModuleLoadingAnchor;
#[doc(hidden)]
pub use host_module::ObservedRootRepositoryRoute;
pub use host_module::RootModuleLoadingAnchor;
pub use host_module::RootModuleLoadingAnchorError;
pub use host_module::RootModuleLoadingAnchorKey;
#[doc(hidden)]
pub use host_module::RootModuleLoadingAnchorObservationKey;
pub use host_module::RootRepositoryRoute;
pub use host_module::RootRepositoryRouteError;
pub use host_module::RootRepositoryRouteKey;
#[doc(hidden)]
pub use host_module::RootRepositoryRouteObservationError;
#[doc(hidden)]
pub use host_module::RootRepositoryRouteObservationKey;
pub use host_module::RootRepositorySource;
#[doc(hidden)]
pub use host_package::ObservedRepositoryPackageSource;
#[doc(hidden)]
pub use host_package::ObservedRootPackageSource;
pub use host_package::RepositoryPackageSource;
pub use host_package::RepositoryPackageSourceAddress;
pub use host_package::RepositoryPackageSourceError;
pub use host_package::RepositoryPackageSourceKey;
#[doc(hidden)]
pub use host_package::RepositoryPackageSourceObservationKey;
pub use host_package::RootPackageBzlTarget;
pub use host_package::RootPackageBzlTargetError;
pub use host_package::RootPackageSource;
pub use host_package::RootPackageSourceError;
pub use host_package::RootPackageSourceKey;
#[doc(hidden)]
pub use host_package::RootPackageSourceObservationKey;
pub use host_package_boundary::HostRootPackageBoundary;
pub use host_package_boundary::HostRootPackageBoundaryError;
pub use host_package_boundary::HostRootPackageBoundaryKey;
pub use host_package_boundary::HostRootPackageBoundaryKind;
#[doc(hidden)]
pub use host_package_boundary::HostRootPackageBoundaryObservationKey;
#[doc(hidden)]
pub use host_package_boundary::ObservedHostRootPackageBoundary;
pub use interim_module::EvaluatedNonrootModule;
pub use interim_module::LogicalModuleFileId;
pub use interim_module::LogicalSpan;
pub use interim_module::ModuleRegistrationPattern;
pub use interim_module::NonrootAttributeInt;
pub use interim_module::NonrootAttributeKey;
pub use interim_module::NonrootAttributeValue;
pub use interim_module::NonrootDependency;
pub use interim_module::NonrootExtensionIsolationKey;
pub use interim_module::NonrootExtensionProxy;
pub use interim_module::NonrootExtensionTag;
pub use interim_module::NonrootExtensionUsage;
pub use interim_module::NonrootModuleBase;
pub use interim_module::NonrootModuleBuilder;
pub use interim_module::NonrootModuleKey;
pub use interim_module::NonrootRepoImports;
pub use interim_module::NonrootRepoOverride;
pub use lockfile::AdapterDomain;
pub use lockfile::BAZEL_9_LOCK_FILE_VERSION;
pub use lockfile::BazelLockfile;
pub use lockfile::HiddenLockfileInput;
pub use lockfile::LockfileParseError;
pub use lockfile::LockfileParseErrorKind;
pub use lockfile::LockfileParseErrorSurface;
pub use lockfile::LockfileReadInputs;
pub use lockfile::LockfileReadSnapshot;
pub use lockfile::LockfileRenderError;
pub use lockfile::LockfileRenderErrorKind;
pub use lockfile::RegistryFileExpectation;
pub use lockfile::SourcePosition;
pub use lockfile::VisibleLockfileApply;
pub use lockfile::VisibleLockfileInput;
pub use lockfile::VisibleLockfilePlan;
pub use lockfile::VisibleLockfileRead;
pub use lockfile::apply_visible_lockfile_plan;
pub use lockfile::empty_bazel_lockfile;
pub use lockfile::parse_bazel_lockfile;
pub use lockfile::parse_hidden_lockfile_fail_open;
pub use lockfile::parse_visible_lockfile_for_mode;
pub use lockfile::plan_visible_lockfile;
pub use lockfile::render_bazel_lockfile;
pub use module_eval::EvaluatedRootModule;
pub use module_eval::NonrootIncludeRequest;
pub use module_eval::NonrootModuleFileInspection;
pub use module_eval::OverrideAttributeKey;
pub use module_eval::OverrideAttributeValue;
pub use module_eval::RegistryMultipleOverride;
pub use module_eval::RegistrySingleOverride;
pub use module_eval::RepoRuleId;
pub use module_eval::RepoSpec;
pub use module_eval::RootModuleCommandPolicy;
pub use module_eval::RootModuleCommandPolicyKey;
pub use module_eval::RootModuleEnvironmentPolicy;
pub use module_eval::RootModuleEnvironmentPolicyKey;
pub use module_eval::RootModuleFiles;
pub use module_eval::RootModuleFilesKey;
pub use module_eval::RootModuleGraph;
pub use module_eval::RootModuleGraphKey;
pub use module_eval::RootModuleLockfileMode;
pub use module_eval::RootModuleLockfileModeKey;
pub use module_eval::RootModuleOverride;
pub use module_eval::RootModuleOverrides;
pub use module_eval::RootModuleRegistrations;
pub use module_eval::VisibleLockfileKey;
pub use module_eval::inject_root_module_request_inputs;
pub use module_eval::inspect_nonroot_module_file;
pub use module_patch::ModulePatchError;
pub use module_patch::apply_unified_patch;
pub use package_policy::RootPackageLookupInputs;
pub use package_policy::RootPackageLookupInputsProjectionKey;
pub use package_policy::RootPackagePolicyInputs;
pub use package_policy::RootPackagePolicyNormalizationError;
pub use package_policy::RootPackagePolicyProjectionError;
pub use package_policy::RootRepoFileSemantics;
pub use package_policy::RootRepoFileSemanticsProjectionKey;
pub use package_policy::RootRepoFileUtf8Mode;
pub use package_policy::RootRepositoryIgnoreInputs;
pub use package_policy::RootRepositoryIgnoreInputsProjectionKey;
pub use package_policy::inject_root_package_policy_inputs;
pub use parser::ArchiveOverride;
pub use parser::BazelDep;
pub use parser::Directive;
pub use parser::ExtensionTag;
pub use parser::GitOverride;
pub use parser::InjectRepo;
pub use parser::LocalPathOverride;
pub use parser::ModuleAttributeValue;
pub use parser::ModuleFile;
pub use parser::ModuleHeader;
pub use parser::ModuleRegistrationDirective;
pub use parser::MultipleVersionOverride;
pub use parser::OverrideRepo;
pub use parser::Registration;
pub use parser::RegistrationKind;
pub use parser::RepoImport;
pub use parser::RepoRuleInvocation;
pub use parser::SingleVersionOverride;
pub use parser::UseExtension;
pub use parser::UseRepo;
pub use parser::UseRepoRule;
pub use parser::expand_included_module_files;
pub use parser::module_registration_directives;
pub use registry::RegistryBaseUrl;
pub use registry::RegistryCatalog;
pub use registry::RegistryContentDigests;
pub use registry::RegistryContentSnapshot;
pub use registry::RegistryMetadata;
pub use registry::RegistryModule;
pub use registry::RegistrySource;
pub use registry::RegistrySourceCatalog;
pub use registry::RegistrySourceSpec;
pub use registry::RegistryUrls;
pub use registry::SelectedYankedVersion;
pub use registry::YankedVersionPolicy;
pub use registry::digest_selected_registry_modules;
pub use registry::digest_selected_registry_sources;
pub use registry::observed_registry_file_hashes;
pub use registry::observed_registry_policy_file_hashes;
pub use registry::parse_registry_metadata_json;
pub use registry::parse_registry_source_json;
pub use registry::registry_bazel_registry_json_url;
pub use registry::registry_module_file_url;
pub use registry::registry_source_json_url;
pub use registry::resolve_registry_mvs;
pub use registry::resolve_registry_mvs_with_dev_dependency_mode;
pub use registry::select_ordered_registry_modules;
pub use registry::select_ordered_registry_sources;
pub use registry::selected_registry_file_hash_urls;
pub use registry::snapshot_registry_contents;
pub use registry::validate_yanked_versions;
pub use registry_dice::RegistryFileError;
pub use registry_dice::RegistryFileKey;
pub use registry_dice::RegistryFileUrl;
pub use registry_dice::RegistryFileValue;
pub use registry_dice::RegistryIo;
pub use registry_dice::RegistryIoOutcome;
pub use registry_dice::RegistryNotFoundSource;
pub use registry_dice::RegistryPolicy;
pub use registry_dice::RegistryPolicyKey;
pub use registry_dice::RegistryRequestGeneration;
pub use registry_dice::RegistryRequestGenerationKey;
pub use registry_dice::RegistryTransportError;
pub use registry_dice::RootModuleRegistryUrls;
pub use registry_dice::RootModuleRegistryUrlsKey;
pub use registry_dice::inject_registry_request_inputs;
pub use registry_dice::install_registry_io;
#[doc(hidden)]
pub use repository_host_input::NeedRepositoryEnvironmentNames;
#[doc(hidden)]
pub use repository_host_input::RepositoryEnvironmentCanonicalError;
#[doc(hidden)]
pub use repository_host_input::RepositoryEnvironmentCell;
#[doc(hidden)]
pub use repository_host_input::RepositoryEnvironmentCellKey;
#[doc(hidden)]
pub use repository_host_input::RepositoryEnvironmentEntry;
#[doc(hidden)]
pub use repository_host_input::RepositoryEnvironmentNameFrontier;
#[doc(hidden)]
pub use repository_host_input::RepositoryEnvironmentSnapshot;
#[doc(hidden)]
pub use repository_host_input::RepositoryHostInputTransaction;
#[doc(hidden)]
pub use repository_host_input::RepositoryPlatform;
#[doc(hidden)]
pub use repository_host_input::RepositoryPlatformKey;
pub use resolution::DevDependencyMode;
pub use resolution::ModuleDirectiveOwner;
pub use resolution::ModuleKey;
pub use resolution::ModuleSource;
pub use resolution::ResolvedDependency;
pub use resolution::ResolvedGraph;
pub use resolution::ResolvedModule;
pub use resolution::active_module_registration_directives;
pub use resolution::bazel_canonical_module_repo_name;
pub use resolution::parse_bazel_dump_repo_mapping_json_lines;
pub use resolution::resolve_local_module_graph;
pub use resolution::resolve_local_module_graph_with_dev_dependency_mode;
pub use resolution::resolve_local_module_graph_with_includes;
pub use resolution::resolve_local_module_graph_with_includes_and_dev_dependency_mode;
pub use root_bootstrap::ROOT_MODULE_BOOTSTRAP_REMINDER_BYTES;
pub use root_bootstrap::ROOT_MODULE_BOOTSTRAP_REMINDER_SHA256;
pub use root_bootstrap::ROOT_MODULE_BOOTSTRAP_WARNING_TEXT;
pub use root_bootstrap::RootModuleBootstrapApplyResult;
pub use root_bootstrap::RootModuleBootstrapCreateError;
pub use root_bootstrap::RootModuleBootstrapRequest;
pub use root_bootstrap::RootModuleBootstrapWarning;
#[doc(hidden)]
pub use selected_repo_spec::HostBuiltinBazelToolsRepositoryMapping;
#[doc(hidden)]
pub use selected_repo_spec::HostBuiltinBazelToolsRepositoryMappingError;
#[doc(hidden)]
pub use selected_repo_spec::HostBuiltinBazelToolsRepositoryMappingKey;
#[doc(hidden)]
pub use selected_repo_spec::HostBuiltinBazelToolsRepositoryMappingObservationError;
#[doc(hidden)]
pub use selected_repo_spec::HostBuiltinBazelToolsRepositoryMappingObservationKey;
#[doc(hidden)]
pub use selected_repo_spec::HostBuiltinBazelToolsRepositoryMappingOutcome;
#[doc(hidden)]
pub use selected_repo_spec::HostCanonicalSelectedModuleDefinition;
#[doc(hidden)]
pub use selected_repo_spec::HostCanonicalSelectedModuleDefinitionError;
#[doc(hidden)]
pub use selected_repo_spec::HostCanonicalSelectedModuleDefinitionErrorDisposition;
#[doc(hidden)]
pub use selected_repo_spec::HostCanonicalSelectedModuleDefinitionKey;
#[doc(hidden)]
pub use selected_repo_spec::HostCanonicalSelectedModuleDefinitionObservationError;
#[doc(hidden)]
pub use selected_repo_spec::HostCanonicalSelectedModuleDefinitionObservationKey;
#[doc(hidden)]
pub use selected_repo_spec::HostCanonicalSelectedModuleDefinitionOutcome;
#[doc(hidden)]
pub use selected_repo_spec::HostCanonicalSelectedModuleDefinitionView;
#[doc(hidden)]
pub use selected_repo_spec::HostCanonicalSelectedModuleIdentity;
#[doc(hidden)]
pub use selected_repo_spec::HostCanonicalSelectedModuleKind;
#[doc(hidden)]
pub use selected_repo_spec::HostCanonicalSelectedModuleMappingIter;
#[doc(hidden)]
pub use selected_repo_spec::HostRootRepositoryMapping;
#[doc(hidden)]
pub use selected_repo_spec::HostRootRepositoryMappingError;
#[doc(hidden)]
pub use selected_repo_spec::HostRootRepositoryMappingIter;
#[doc(hidden)]
pub use selected_repo_spec::HostRootRepositoryMappingKey;
#[doc(hidden)]
pub use selected_repo_spec::HostRootRepositoryMappingOutcome;
#[doc(hidden)]
pub use selected_repo_spec::HostRootRepositoryMappingView;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedRegistrationPatternView;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedRegistrationPatterns;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedRegistrationPatternsError;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedRegistrationPatternsKey;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedRegistrationPatternsObservationError;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedRegistrationPatternsObservationKey;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedRegistrationPatternsOutcome;
#[doc(hidden)]
pub use selected_repo_spec::ObservedHostSelectedRegistrationPatterns;
#[rustfmt::skip]
#[doc(hidden)]
pub use selected_repo_spec::HostRootRepositoryMappingObservationError;
#[rustfmt::skip]
#[doc(hidden)]
pub use selected_repo_spec::HostRootRepositoryMappingObservationKey;
#[rustfmt::skip]
#[doc(hidden)]
pub use selected_repo_spec::ObservedHostRootRepositoryMapping;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionDefinitionImport;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionDefinitionLoadRequest;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionDefinitionLoadRequests;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionDefinitionLoadRequestsError;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionDefinitionLoadRequestsKey;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionDefinitionLoadRequestsObservationError;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionDefinitionLoadRequestsObservationKey;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionDefinitionOverride;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionDefinitionSource;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionDemand;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionDemandError;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionDemandErrorDisposition;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionDemandKey;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionDemandObservationError;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionDemandObservationKey;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionEvaluationInput;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionEvaluationInputError;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionEvaluationInputRequests;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionEvaluationInputRequestsError;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionEvaluationInputRequestsKey;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionEvaluationInputRequestsObservationError;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionEvaluationInputRequestsObservationKey;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionOwner;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionOwnerInputs;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionOwnerInputsError;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionOwnerInputsKey;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionOwnerInputsObservationError;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionOwnerInputsObservationKey;
#[doc(hidden)]
pub use selected_repo_spec::HostSelectedExtensionOwnerModuleInput;
#[doc(hidden)]
pub use selected_repo_spec::ObservedHostBuiltinBazelToolsRepositoryMapping;
#[doc(hidden)]
pub use selected_repo_spec::ObservedHostCanonicalSelectedModuleDefinition;
#[doc(hidden)]
pub use selected_repo_spec::ObservedHostSelectedExtensionDefinitionLoadRequests;
#[doc(hidden)]
pub use selected_repo_spec::ObservedHostSelectedExtensionDemand;
#[doc(hidden)]
pub use selected_repo_spec::ObservedHostSelectedExtensionEvaluationInputRequests;
#[doc(hidden)]
pub use selected_repo_spec::ObservedHostSelectedExtensionOwnerInputs;
#[doc(hidden)]
pub use source_preparation::HostCanonicalRepositorySourceInput;
#[doc(hidden)]
pub use source_preparation::HostCanonicalRepositorySourceInputError;
#[doc(hidden)]
pub use source_preparation::HostCanonicalRepositorySourceInputView;
#[doc(hidden)]
pub use source_preparation::HostRepositoryDirectoryListing;
#[doc(hidden)]
pub use source_preparation::HostRepositoryDirectoryListingError;
#[doc(hidden)]
pub use source_preparation::HostRepositoryDirectoryListingKey;
#[doc(hidden)]
pub use source_preparation::HostRepositoryDirectoryListingObservationKey;
#[doc(hidden)]
pub use source_preparation::HostRepositoryLocalPathPolicy;
#[doc(hidden)]
pub use source_preparation::HostRepositoryMaterializationDisposition;
#[doc(hidden)]
pub use source_preparation::HostRepositoryRelativePath;
#[doc(hidden)]
pub use source_preparation::HostRepositoryRelativePathError;
#[doc(hidden)]
pub use source_preparation::HostRepositorySourceFileKey;
#[doc(hidden)]
pub use source_preparation::HostRepositorySourceFileObservationKey;
pub use source_preparation::HostRepositorySourceFileValue;
#[doc(hidden)]
pub use source_preparation::HostRepositorySourceInput;
#[doc(hidden)]
pub use source_preparation::HostRepositorySourceInputDispositionView;
#[doc(hidden)]
pub use source_preparation::HostRepositorySourceInputError;
#[doc(hidden)]
pub use source_preparation::HostRepositorySourceInputView;
#[doc(hidden)]
pub use source_preparation::HostRepositorySourceObservation;
#[doc(hidden)]
pub use source_preparation::HostRepositorySourceObservationEpochKey;
#[doc(hidden)]
pub use source_preparation::HostRepositorySourceObservationError;
#[doc(hidden)]
pub use source_preparation::HostRepositorySourceObservationInput;
#[doc(hidden)]
pub use source_preparation::HostRepositorySourceObservationInputView;
#[doc(hidden)]
pub use source_preparation::HostRepositorySourceObservationKey;
#[doc(hidden)]
pub use source_preparation::HostRepositorySourceObservationOutcome;
#[doc(hidden)]
pub use source_preparation::HostRepositorySourceObservationResult;
#[doc(hidden)]
pub use source_preparation::HostRepositorySourceObservationView;
#[doc(hidden)]
pub use source_preparation::HostRepositorySourceRoute;
#[doc(hidden)]
pub use source_preparation::HostSelectedObservationFrontier;
pub use source_preparation::ModuleSourcePreparation;
pub use source_preparation::ModuleSourcePreparationError;
pub use source_preparation::ModuleSourcePreparationKey;
#[doc(hidden)]
pub use source_preparation::ObservedHostRepositoryDirectoryListing;
#[doc(hidden)]
pub use source_preparation::ObservedHostRepositorySourceFile;
#[doc(hidden)]
pub use source_preparation::ObservedHostRepositorySourceObservation;
pub use source_preparation::RegistryModuleFileAttempt;
pub use source_preparation::RepositoryIo;
pub use source_preparation::RepositoryIoOutcome;
pub use source_preparation::RepositoryMaterialization;
pub use source_preparation::RepositoryMaterializationEpochEntry;
pub use source_preparation::RepositoryMaterializationError;
pub use source_preparation::RepositoryMaterializationGeneration;
pub use source_preparation::RepositoryMaterializationGenerationKey;
pub use source_preparation::RepositoryMaterializationKey;
pub use source_preparation::RepositoryMaterializationKind;
pub use source_preparation::RepositoryMaterializationRequest;
pub use source_preparation::RepositoryMaterializationRequestId;
pub use source_preparation::RepositoryMaterializationResult;
pub use source_preparation::RepositoryMaterializationResultEpoch;
pub use source_preparation::RepositoryMaterializationResultEpochError;
pub use source_preparation::RepositoryMaterializationResultEpochKey;
pub use source_preparation::RepositoryMaterializationSuccess;
pub use source_preparation::RepositorySourceFileError;
pub use source_preparation::RepositorySourceFileKey;
pub use source_preparation::RepositorySourceFileValue;
pub use source_preparation::RepositoryTransportError;
pub use source_preparation::SourcePreparationNeeds;
pub use source_preparation::SourcePreparationNeedsError;
pub use source_preparation::SourcePreparationOutcome;
pub use source_preparation::SourcePreparationResult;
#[doc(hidden)]
pub use source_preparation::host_canonical_repository_source_input;
#[doc(hidden)]
pub use source_preparation::host_repository_materialization_request;
#[doc(hidden)]
pub use source_preparation::host_repository_relative_path;
#[doc(hidden)]
pub use source_preparation::host_repository_source_input;
pub use source_preparation::install_repository_io;
pub use source_preparation::source_identity;
