/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

pub mod attrs;
pub mod bzl_module;
mod bzl_visibility;
mod canonical_repository_load_route;
#[cfg(test)]
mod canonical_repository_load_route_tests;
mod canonical_repository_mapping;
mod canonical_repository_route;
#[cfg(test)]
mod canonical_repository_route_tests;
mod cc_common;
mod cycle_detector;
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
mod module_extension;
mod module_extension_innate_repository;
mod module_extension_repository_file_effect;
mod module_extension_repository_instantiation;
mod module_extension_repository_rule;
mod module_extension_repository_validation;
pub mod package;
#[doc(hidden)]
pub mod provider;
mod registration_expansion;
#[cfg(test)]
mod registration_expansion_tests;
mod root_subtree_package_set;
mod starlark_label;
pub mod visibility;

pub use attrs::AllowSingleFile;
pub use attrs::AttributeKind;
pub use attrs::AttributeProvenance;
pub use attrs::AttributeQueryValue;
pub use attrs::AttributeSchema;
pub use attrs::AttributeValue;
pub use attrs::CoercedAttributeValue;
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
pub use cycle_detector::bzl_load_cycle_detector;
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
pub use package::LoadedPackage;
pub use package::NativeTargetAttributes;
pub use package::PackageTarget;
pub use package::PackageTargetKind;
pub use package::RuleCapability;
pub use package::TestMetadata;
pub use package::TestRuleKind;
pub use package::TestSuiteMembership;
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
pub use visibility::PackageGroupContents;
pub use visibility::RestrictedVisibility;
pub use visibility::RuleVisibility;
pub use visibility::VisibilitySource;
