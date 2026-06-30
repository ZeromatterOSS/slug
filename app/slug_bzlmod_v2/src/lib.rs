/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

pub mod dice;
pub mod lockfile;
pub mod parser;
pub mod registry;
pub mod resolution;

pub use dice::BzlmodCommandPolicyKey;
pub use dice::BzlmodDiceInputs;
pub use dice::BzlmodEnvironmentPolicyKey;
pub use dice::BzlmodExtensionDefinitionDigest;
pub use dice::BzlmodExtensionUsageDigest;
pub use dice::BzlmodModuleFileDigest;
pub use dice::BzlmodRegistryPolicyEntry;
pub use dice::LockfileMode;
pub use dice::ResolvedBzlmodGraphDiceKey;
pub use dice::digest_included_module_files;
pub use dice::digest_module_extension_definitions;
pub use dice::digest_module_extension_usages;
pub use dice::digest_module_file_content;
pub use dice::digest_registry_policy;
pub use lockfile::BazelLockfile;
pub use lockfile::BazelLockfileModuleExtension;
pub use lockfile::BazelLockfileModuleExtensionGeneral;
pub use lockfile::BazelLockfileRepoSpec;
pub use lockfile::parse_bazel_lockfile;
pub use lockfile::validate_module_extension_bzl_transitive_digests;
pub use lockfile::validate_module_extension_usage_digests;
pub use lockfile::validate_registry_file_hashes;
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
pub use parser::MultipleVersionOverride;
pub use parser::OverrideRepo;
pub use parser::Registration;
pub use parser::RepoImport;
pub use parser::RepoRuleInvocation;
pub use parser::SingleVersionOverride;
pub use parser::UseExtension;
pub use parser::UseRepo;
pub use parser::UseRepoRule;
pub use registry::RegistryCatalog;
pub use registry::RegistryMetadata;
pub use registry::RegistryModule;
pub use registry::RegistrySourceSpec;
pub use registry::SelectedYankedVersion;
pub use registry::YankedVersionPolicy;
pub use registry::parse_registry_metadata_json;
pub use registry::parse_registry_source_json;
pub use registry::resolve_registry_mvs;
pub use registry::select_ordered_registry_modules;
pub use registry::validate_yanked_versions;
pub use resolution::ModuleKey;
pub use resolution::ModuleSource;
pub use resolution::ResolvedDependency;
pub use resolution::ResolvedGraph;
pub use resolution::ResolvedModule;
pub use resolution::bazel_canonical_module_repo_name;
pub use resolution::resolve_local_module_graph;
