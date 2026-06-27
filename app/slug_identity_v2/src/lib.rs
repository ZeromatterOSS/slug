/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

pub mod label;
pub mod layout;
pub mod package;
pub mod pattern;
pub mod repo;
pub mod repo_mapping;
pub mod serialization;

pub use label::ApparentLabel;
pub use label::CanonicalLabel;
pub use package::PackageIdentifier;
pub use package::PackagePath;
pub use package::TargetName;
pub use pattern::TargetPattern;
pub use repo::ApparentRepoName;
pub use repo::CanonicalRepoName;
pub use repo_mapping::RepositoryMapping;
pub use repo_mapping::RepositoryMappingId;
