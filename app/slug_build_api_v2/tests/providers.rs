/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use compact_str::CompactString;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisConfiguredTargetKey;
use slug_build_api_v2::AnalysisDepset;
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::AnalysisValueType;
use slug_build_api_v2::DefaultInfo;
use slug_build_api_v2::Depset;
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::FilesToRunProvider;
use slug_build_api_v2::OutputGroupInfo;
use slug_build_api_v2::PlatformInfo;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::ProviderError;
use slug_build_api_v2::ProviderId;
use slug_build_api_v2::ProviderIdentity;
use slug_build_api_v2::ProviderName;
use slug_build_api_v2::ProviderOccurrence;
use slug_build_api_v2::ProviderValue;
use slug_build_api_v2::RetainedRunfiles;
use slug_build_api_v2::RunEnvironmentInfo;
use slug_build_api_v2::RunfilesConflictPolicy;
use slug_build_api_v2::RunfilesError;
use slug_build_api_v2::RunfilesSupport;
use slug_build_api_v2::RunfilesSymlink;
use slug_identity_v2::CanonicalLabel;

fn files(items: &[&str]) -> Depset<String> {
    Depset::from_direct(
        DepsetOrder::Default,
        items.iter().map(|item| item.to_string()).collect(),
    )
    .unwrap()
}

fn source_artifact(path: &str) -> AnalysisArtifact {
    let (package, target) = path.rsplit_once('/').unwrap_or(("", path));
    AnalysisArtifact::Source(
        CanonicalLabel::parse(&format!("@@//{package}:{target}"))
            .expect("test path forms a source label"),
    )
}

fn derived_artifact(path: &str, kind: ActionOutputKind) -> AnalysisArtifact {
    AnalysisArtifact::Derived {
        owner: AnalysisConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//pkg:owner").unwrap(),
            b"cfg".as_slice(),
        ),
        output: ActionOutput::new(path, kind),
    }
}

fn artifact_files(items: &[&str]) -> AnalysisDepset {
    AnalysisDepset::new(
        DepsetOrder::Default,
        items
            .iter()
            .map(|path| AnalysisValue::artifact(source_artifact(path)))
            .collect(),
        Vec::new(),
    )
    .unwrap()
}

fn default_paths(info: &DefaultInfo) -> Vec<String> {
    info.file_artifacts()
        .into_iter()
        .map(|artifact| artifact.path().into_owned())
        .collect()
}

fn runfiles_paths(runfiles: &RetainedRunfiles) -> Vec<String> {
    runfiles
        .files
        .to_list()
        .into_iter()
        .map(|value| match value.kind() {
            slug_build_api_v2::AnalysisValueKind::Artifact(value) => value.path().into_owned(),
            _ => unreachable!(),
        })
        .collect()
}

fn user_provider(
    id: ProviderId,
    fields: impl IntoIterator<Item = (impl Into<compact_str::CompactString>, AnalysisValue)>,
) -> ProviderValue {
    ProviderValue::Occurrence(ProviderOccurrence::new(ProviderIdentity::user(id), fields))
}

#[test]
fn provider_collection_requires_default_info_for_rules() {
    let err = ProviderCollection::new(vec![user_provider(
        ProviderId::unqualified("MyInfo").unwrap(),
        std::iter::empty::<(String, AnalysisValue)>(),
    )])
    .unwrap_err();

    assert_eq!(err, ProviderError::MissingDefaultInfo);
    assert_eq!(
        err.to_string(),
        "collection did not receive a `DefaultInfo` provider"
    );
}

#[test]
fn provider_collection_rejects_duplicate_provider_keys() {
    let duplicate = ProviderCollection::new(vec![
        ProviderValue::DefaultInfo(DefaultInfo::empty()),
        user_provider(
            ProviderId::unqualified("MyInfo").unwrap(),
            std::iter::empty::<(String, AnalysisValue)>(),
        ),
        user_provider(
            ProviderId::unqualified("MyInfo").unwrap(),
            std::iter::empty::<(String, AnalysisValue)>(),
        ),
    ])
    .unwrap_err();

    assert_eq!(
        duplicate,
        ProviderError::DuplicateProvider {
            name: ProviderName::new("MyInfo").unwrap()
        }
    );
    assert_eq!(duplicate.to_string(), "provider MyInfo specified twice");
}

#[test]
fn provider_collection_exposes_bazel_native_and_user_providers() {
    let collection = ProviderCollection::new(vec![
        ProviderValue::DefaultInfo(
            DefaultInfo::from_files(artifact_files(&["pkg/custom.txt"])).unwrap(),
        ),
        user_provider(
            ProviderId::unqualified("MyInfo").unwrap(),
            [("value", AnalysisValue::string("custom"))],
        ),
        ProviderValue::RunEnvironmentInfo(RunEnvironmentInfo::empty()),
        ProviderValue::PlatformInfo(PlatformInfo::new("@platforms//host:host")),
    ])
    .unwrap();

    let names = collection
        .names()
        .map(|name| name.as_str().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "DefaultInfo".to_owned(),
            "MyInfo".to_owned(),
            "RunEnvironmentInfo".to_owned(),
            "PlatformInfo".to_owned(),
        ]
    );
    assert!(collection.contains(&ProviderIdentity::user(
        ProviderId::unqualified("MyInfo").unwrap()
    )));
    assert_eq!(
        default_paths(collection.default_info().unwrap()),
        ["pkg/custom.txt"]
    );
}

#[test]
fn executable_default_info_uses_the_executable_for_implicit_files_and_runfiles() {
    let info = DefaultInfo::from_executable(source_artifact("pkg/tool"), None).unwrap();

    assert_eq!(default_paths(&info), ["pkg/tool"]);
    assert_eq!(
        info.executable.as_ref().map(|value| value.path()),
        Some("pkg/tool".into())
    );
    assert_eq!(info.files_to_run.executable, info.executable);
    assert!(!info.files_to_run.is_complete());
    assert_eq!(runfiles_paths(&info.default_runfiles), ["pkg/tool"]);
    assert_eq!(runfiles_paths(&info.data_runfiles), ["pkg/tool"]);
    assert!(info.files_to_run.runfiles_manifest().is_none());
    assert!(info.files_to_run.repo_mapping_manifest().is_none());
    assert!(info.default_runfiles.symlinks.is_empty());
    assert!(info.data_runfiles.symlinks.is_empty());
}

#[test]
fn file_target_files_to_run_is_complete_and_typed() {
    let artifact = source_artifact("pkg/input.txt");
    let info = DefaultInfo::from_file_target(artifact.clone());

    assert_eq!(info.executable, Some(artifact.clone()));
    assert_eq!(info.files_to_run.executable, Some(artifact));
    assert!(info.files_to_run.is_complete());
    assert_eq!(
        info.files_to_run.files().element_type(),
        AnalysisValueType::Artifact
    );
    assert!(info.default_runfiles.files.is_empty());
}

#[test]
fn runfiles_support_reserves_the_complete_typed_category_shape() {
    let runfile = source_artifact("pkg/data.txt");
    let runfiles = RetainedRunfiles {
        files: artifact_files(&["pkg/data.txt"]),
        symlinks: Depset::from_direct(
            DepsetOrder::Default,
            vec![RunfilesSymlink::new("logical/data.txt", runfile.clone())],
        )
        .unwrap(),
        root_symlinks: Depset::empty(),
        empty_filenames: Depset::empty(),
        conflict_policy: RunfilesConflictPolicy::Warn,
        repository_prefix: CompactString::new("repo"),
    };
    let support = RunfilesSupport {
        runfiles,
        tree: source_artifact("pkg/tool.runfiles"),
        input_manifest: source_artifact("pkg/tool.runfiles_manifest"),
        manifest: None,
        repo_mapping_manifest: None,
    };

    assert_eq!(support.runfiles.files.to_list().len(), 1);
    assert_eq!(support.runfiles.symlinks.to_list().len(), 1);
    assert_eq!(support.runfiles.repository_prefix, "repo");
    assert!(support.manifest.is_none());
}

#[test]
fn files_to_run_private_carrier_preserves_complete_support_and_hash_law() {
    let executable = derived_artifact("pkg/tool", ActionOutputKind::File);
    let info = DefaultInfo::from_executable(executable.clone(), None).unwrap();
    let incomplete = info.files_to_run.clone();
    let incomplete_round_trip =
        FilesToRunProvider::from_occurrence(&incomplete.to_occurrence()).unwrap();
    assert_eq!(incomplete_round_trip, incomplete);
    assert!(!incomplete_round_trip.is_complete());
    let support = Arc::new(RunfilesSupport {
        runfiles: info.default_runfiles.clone(),
        tree: derived_artifact("pkg/tool.runfiles", ActionOutputKind::RunfilesTree),
        input_manifest: derived_artifact("pkg/tool.runfiles_manifest", ActionOutputKind::File),
        manifest: Some(derived_artifact(
            "pkg/tool.runfiles/MANIFEST",
            ActionOutputKind::File,
        )),
        repo_mapping_manifest: Some(derived_artifact(
            "pkg/tool.repo_mapping",
            ActionOutputKind::File,
        )),
    });
    let provider = info
        .with_runfiles_support(support.clone())
        .unwrap()
        .files_to_run;
    let occurrence = provider.to_occurrence();
    let round_trip = FilesToRunProvider::from_occurrence(&occurrence).unwrap();
    assert!(round_trip.is_complete());
    assert!(Arc::ptr_eq(round_trip.support.as_ref().unwrap(), &support));

    let fabricated = ProviderOccurrence::new(
        occurrence.identity().clone(),
        occurrence
            .fields()
            .iter()
            .map(|(name, value)| (name.clone(), value.clone())),
    );
    assert_ne!(occurrence, fabricated);
    assert!(FilesToRunProvider::from_occurrence(&fabricated).is_none());
    let hash = |value: &ProviderOccurrence| {
        let mut state = DefaultHasher::new();
        value.hash(&mut state);
        state.finish()
    };
    assert_eq!(hash(&occurrence), hash(&fabricated));

    let same_name = ProviderOccurrence::new(
        ProviderIdentity::user(ProviderId::unqualified("FilesToRunProvider").unwrap()),
        occurrence
            .fields()
            .iter()
            .map(|(name, value)| (name.clone(), value.clone())),
    );
    assert!(FilesToRunProvider::from_occurrence(&same_name).is_none());
}

#[test]
fn runfiles_admit_symlink_artifacts_but_reject_tree_shapes() {
    let make = |kind| {
        RetainedRunfiles::from_parts(
            vec![derived_artifact("pkg/value", kind)],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            RunfilesConflictPolicy::Warn,
        )
    };
    assert!(make(ActionOutputKind::File).is_ok());
    assert!(make(ActionOutputKind::Symlink).is_ok());
    for kind in [ActionOutputKind::Directory, ActionOutputKind::RunfilesTree] {
        assert_eq!(make(kind), Err(RunfilesError::InvalidArtifactKind(kind)));
    }
}

#[test]
fn runfiles_merge_preserves_order_conflicts_and_distinct_symlink_occurrences() {
    let artifact = source_artifact("pkg/data.txt");
    let make = |path: &str, policy| {
        RetainedRunfiles::from_parts(
            vec![source_artifact(path)],
            Vec::new(),
            vec![RunfilesSymlink::new("logical/data.txt", artifact.clone())],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            policy,
        )
        .unwrap()
    };
    let left = make("pkg/left.txt", RunfilesConflictPolicy::Warn);
    let right = make("pkg/right.txt", RunfilesConflictPolicy::Error);
    let merged = left.merge(&right).unwrap();

    assert_eq!(runfiles_paths(&merged), ["pkg/left.txt", "pkg/right.txt"]);
    assert_eq!(merged.conflict_policy, RunfilesConflictPolicy::Error);
    assert_eq!(merged.symlinks.to_list().len(), 2);
    assert_eq!(left.merge(&RetainedRunfiles::empty()).unwrap(), left);
}

#[test]
fn runfiles_publication_ignores_temporary_symlink_occurrence_tokens() {
    let graph = |shared_grandchild: bool| {
        let leaf = || {
            Depset::from_direct(
                DepsetOrder::Default,
                vec![
                    RunfilesSymlink::new("logical/data.txt", source_artifact("pkg/data.txt")),
                    RunfilesSymlink::new("logical/config.txt", source_artifact("pkg/config.txt")),
                ],
            )
            .unwrap()
        };
        let grandchild = leaf();
        let other_grandchild = if shared_grandchild {
            grandchild.clone()
        } else {
            leaf()
        };
        let branch = |grandchild| {
            Depset::new(
                DepsetOrder::Default,
                vec![RunfilesSymlink::new(
                    "logical/branch.txt",
                    source_artifact("pkg/branch.txt"),
                )],
                vec![grandchild],
            )
            .unwrap()
        };
        Depset::new(
            DepsetOrder::Default,
            Vec::new(),
            vec![branch(grandchild), branch(other_grandchild)],
        )
        .unwrap()
    };
    let collection = |shared_grandchild: bool, shared_fields: bool| {
        let symlinks = graph(shared_grandchild);
        let root_symlinks = if shared_fields {
            symlinks.clone()
        } else {
            graph(shared_grandchild)
        };
        let runfiles = RetainedRunfiles {
            files: AnalysisDepset::empty(DepsetOrder::Default),
            symlinks,
            root_symlinks,
            empty_filenames: Depset::empty(),
            conflict_policy: RunfilesConflictPolicy::Error,
            repository_prefix: "_main".into(),
        };
        ProviderCollection::new(vec![ProviderValue::DefaultInfo(
            DefaultInfo::from_effective(
                AnalysisDepset::empty(DepsetOrder::Default),
                runfiles,
                RetainedRunfiles::empty(),
                None,
            )
            .unwrap(),
        )])
        .unwrap()
    };

    assert_eq!(collection(true, true), collection(true, true));
    assert_ne!(collection(true, true), collection(false, true));
    assert_ne!(collection(true, true), collection(true, false));
}

#[test]
fn files_to_run_occurrence_round_trip_preserves_private_typed_state() {
    let provider =
        FilesToRunProvider::single_executable_without_support(source_artifact("pkg/input.txt"));
    assert_eq!(
        FilesToRunProvider::from_occurrence(&provider.to_occurrence()),
        Some(provider)
    );
    let fabricated = ProviderOccurrence::new(
        ProviderIdentity::builtin("FilesToRunProvider"),
        [("executable", AnalysisValue::none())],
    );
    assert!(FilesToRunProvider::from_occurrence(&fabricated).is_none());
}

#[test]
fn executable_default_info_preserves_an_explicit_files_override() {
    let info = DefaultInfo::from_executable(
        source_artifact("pkg/tool"),
        Some(artifact_files(&["pkg/explicit.txt"])),
    )
    .unwrap();

    assert_eq!(default_paths(&info), ["pkg/explicit.txt"]);
    let mut files_to_run = info
        .files_to_run
        .files()
        .to_list()
        .into_iter()
        .map(|value| match value.kind() {
            slug_build_api_v2::AnalysisValueKind::Artifact(value) => value.path().into_owned(),
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();
    files_to_run.sort();
    assert_eq!(files_to_run, ["pkg/explicit.txt", "pkg/tool"]);
    assert_eq!(runfiles_paths(&info.default_runfiles), ["pkg/tool"]);
    assert_eq!(runfiles_paths(&info.data_runfiles), ["pkg/tool"]);
}

#[test]
fn default_info_rejects_non_file_depsets() {
    let strings = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::string("pkg/not-a-file")],
        Vec::new(),
    )
    .unwrap();

    let error = DefaultInfo::from_files(strings).unwrap_err();
    assert_eq!(
        error,
        ProviderError::InvalidDefaultInfoFiles {
            element_type: slug_build_api_v2::AnalysisValueType::String,
        }
    );
}

#[test]
fn default_info_rejects_directory_artifacts_in_the_regular_file_slice() {
    let directory = AnalysisArtifact::Derived {
        owner: AnalysisConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//pkg:owner").unwrap(),
            b"configured".as_slice(),
        ),
        output: ActionOutput::new("pkg/tree", ActionOutputKind::Directory),
    };
    let files = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::artifact(directory)],
        Vec::new(),
    )
    .unwrap();

    assert_eq!(
        DefaultInfo::from_files(files).unwrap_err(),
        ProviderError::InvalidDefaultInfoArtifactKind {
            kind: ActionOutputKind::Directory,
        }
    );
}

#[test]
fn provider_collection_looks_up_user_providers_by_structural_export_identity() {
    let constructor_id = ProviderId::new("//rules:defs.bzl", "MyInfo").unwrap();
    let independently_reconstructed_id = ProviderId::new("//rules:defs.bzl", "MyInfo").unwrap();
    let collection = ProviderCollection::new(vec![
        ProviderValue::DefaultInfo(DefaultInfo::empty()),
        user_provider(constructor_id, [("value", AnalysisValue::string("custom"))]),
    ])
    .unwrap();

    assert_eq!(collection.len(), 2);
    assert_eq!(
        collection
            .user(&independently_reconstructed_id)
            .unwrap()
            .field("value"),
        Some(&AnalysisValue::string("custom"))
    );
    assert!(
        collection
            .user(&ProviderId::new("//other:defs.bzl", "MyInfo").unwrap())
            .is_none()
    );
}

#[test]
fn toolchain_info_uses_builtin_identity_not_a_user_provider_name() {
    let collection = ProviderCollection::new(vec![
        ProviderValue::DefaultInfo(DefaultInfo::empty()),
        user_provider(
            ProviderId::unqualified("DefaultInfo").unwrap(),
            std::iter::empty::<(String, AnalysisValue)>(),
        ),
        user_provider(
            ProviderId::unqualified("ToolchainInfo").unwrap(),
            std::iter::empty::<(String, AnalysisValue)>(),
        ),
        ProviderValue::Occurrence(ProviderOccurrence::new(
            ProviderIdentity::builtin("ToolchainInfo"),
            [("marker", AnalysisValue::string("selected"))],
        )),
    ])
    .unwrap();

    assert_eq!(collection.len(), 4);
    assert_eq!(collection.default_info(), Some(&DefaultInfo::empty()));
    assert_eq!(
        collection.toolchain_info().unwrap().field("marker"),
        Some(&AnalysisValue::string("selected"))
    );
    assert!(
        collection
            .user(&ProviderId::unqualified("ToolchainInfo").unwrap())
            .is_some()
    );
    assert!(
        collection
            .user(&ProviderId::unqualified("DefaultInfo").unwrap())
            .is_some()
    );
}

#[test]
fn output_group_info_keeps_named_file_depsets() {
    let mut groups = BTreeMap::new();
    groups.insert("validation".to_owned(), files(&["pkg/validation.txt"]));
    groups.insert("hidden_top_level".to_owned(), files(&["pkg/hidden.txt"]));

    let collection = ProviderCollection::new(vec![
        ProviderValue::DefaultInfo(DefaultInfo::empty()),
        ProviderValue::OutputGroupInfo(OutputGroupInfo::new(groups)),
    ])
    .unwrap();

    match collection
        .get(&ProviderIdentity::builtin("OutputGroupInfo"))
        .unwrap()
    {
        ProviderValue::OutputGroupInfo(info) => {
            assert_eq!(
                info.groups["hidden_top_level"].to_list(),
                vec!["pkg/hidden.txt".to_owned()]
            );
            assert_eq!(
                info.groups["validation"].to_list(),
                vec!["pkg/validation.txt".to_owned()]
            );
        }
        other => panic!("expected OutputGroupInfo, got {other:?}"),
    }
}
