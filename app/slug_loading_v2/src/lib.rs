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
mod cycle_detector;
pub mod file_discovery;
pub mod glob;
pub mod globals;
mod host_glob;
pub mod keys;
pub mod load_label;
mod module_extension;
mod module_extension_repository_instantiation;
mod module_extension_repository_rule;
mod module_extension_repository_validation;
pub mod package;
#[doc(hidden)]
pub mod provider;
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
pub use module_extension_repository_validation::HostGeneratedRepositoryMapping;
#[doc(hidden)]
pub use module_extension_repository_validation::HostValidatedGeneratedRepositorySpecs;
#[doc(hidden)]
pub use module_extension_repository_validation::HostValidatedGeneratedRepositorySpecsError;
#[doc(hidden)]
pub use module_extension_repository_validation::HostValidatedGeneratedRepositorySpecsOutcome;
#[doc(hidden)]
pub use module_extension_repository_validation::HostValidatedModuleExtensionRepositoriesKey;
pub use package::LoadedPackage;
pub use package::NativeTargetAttributes;
pub use package::PackageTarget;
pub use package::PackageTargetKind;
pub use package::RuleCapability;
pub use package::TestMetadata;
pub use package::TestRuleKind;
pub use package::TestSuiteMembership;
pub use slug_bzlmod_v2::SourcePreparationNeeds as LoadingPreparationNeeds;
pub use slug_bzlmod_v2::SourcePreparationOutcome as LoadingPreparationOutcome;
pub use visibility::PackageGroupContents;
pub use visibility::RestrictedVisibility;
pub use visibility::RuleVisibility;
pub use visibility::VisibilitySource;
