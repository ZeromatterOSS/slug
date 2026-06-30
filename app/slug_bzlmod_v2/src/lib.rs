/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

pub mod parser;

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
pub use parser::RepoImport;
pub use parser::RepoRuleInvocation;
pub use parser::SingleVersionOverride;
pub use parser::UseExtension;
pub use parser::UseRepo;
pub use parser::UseRepoRule;
