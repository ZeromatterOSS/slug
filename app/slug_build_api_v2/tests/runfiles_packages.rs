/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file.
 */

use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use compact_str::CompactString;
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::RunfilesPackageDepset;
use slug_build_api_v2::RunfilesPackageMetadata;
use slug_build_api_v2::RunfilesRepositoryMapping;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackageIdentifier;
use slug_identity_v2::PackagePath;

fn package() -> PackageIdentifier {
    PackageIdentifier::new(
        CanonicalRepoName::root(),
        PackagePath::parse("pkg").unwrap(),
    )
}

fn mapping(target: &str, group: Option<&str>) -> Arc<RunfilesRepositoryMapping> {
    Arc::new(RunfilesRepositoryMapping::new(
        Arc::from([(
            ApparentRepoName::new("dep").unwrap(),
            CanonicalRepoName::new(target).unwrap(),
        )]),
        group.map(CompactString::new),
    ))
}

fn hash(value: &RunfilesPackageMetadata) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn metadata_equality_includes_mapping_and_generated_owner_group() {
    let first = RunfilesPackageMetadata::new(package(), mapping("dep+1", Some("+owner")));
    let same = RunfilesPackageMetadata::new(package(), mapping("dep+1", Some("+owner")));
    let remapped = RunfilesPackageMetadata::new(package(), mapping("dep+2", Some("+owner")));
    let other_owner = RunfilesPackageMetadata::new(package(), mapping("dep+1", Some("+other")));

    assert_eq!(first, same);
    assert_ne!(first, remapped);
    assert_ne!(first, other_owner);
    assert_eq!(first.mapping().compact_group(), Some("+owner"));
    assert_eq!(first.mapping().entries()[0].1.as_str(), "dep+1");
}

#[test]
fn package_only_hash_is_lawful_for_mapping_collisions() {
    let first = RunfilesPackageMetadata::new(package(), mapping("dep+1", None));
    let remapped = RunfilesPackageMetadata::new(package(), mapping("dep+2", None));

    assert_ne!(first, remapped);
    assert_eq!(hash(&first), hash(&remapped));
}

#[test]
fn dense_depset_deduplicates_full_metadata_not_hash_collisions() {
    let first = Arc::new(RunfilesPackageMetadata::new(
        package(),
        mapping("dep+1", None),
    ));
    let duplicate = Arc::new(RunfilesPackageMetadata::new(
        package(),
        mapping("dep+1", None),
    ));
    let remapped = Arc::new(RunfilesPackageMetadata::new(
        package(),
        mapping("dep+2", None),
    ));
    let packages = RunfilesPackageDepset::from_direct(
        DepsetOrder::Default,
        vec![first.clone(), duplicate, remapped.clone()],
    )
    .unwrap();

    assert_eq!(packages.to_list(), vec![first, remapped]);
}
