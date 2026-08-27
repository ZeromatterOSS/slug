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
mod cc_common;
mod cycle_detector;
pub mod file_discovery;
pub mod glob;
pub mod globals;
mod host_glob;
pub mod keys;
pub mod load_label;
mod module_extension;
mod module_extension_repository_file_effect;
mod module_extension_repository_instantiation;
mod module_extension_repository_rule;
mod module_extension_repository_validation;
pub mod package;
#[doc(hidden)]
pub mod provider;
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
pub use cycle_detector::bzl_load_cycle_detector;
pub use glob::GlobSpec;
pub use glob::PackageListing;
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
