use std::sync::Arc;

use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan;
use slug_bzlmod_v2::OverrideAttributeValue;
use slug_bzlmod_v2::RepoRuleId;
use slug_bzlmod_v2::RepoSpec;
use slug_bzlmod_v2::RepositoryMaterializationRequest;
use slug_bzlmod_v2::RepositoryMaterializationRequestId;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NeedPathObservations;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;
use starlark_map::small_map::SmallMap;

use super::*;

pub(super) fn request(
    root: &NormalizedAbsolutePath,
    name: &str,
    files: &[(&str, &[u8], bool)],
) -> RepositoryMaterializationRequest {
    RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: root.clone(),
            canonical_repo: CanonicalRepoName::new(name).unwrap(),
        },
        repo_spec: RepoSpec {
            rule_id: RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@//:arbitrary_repository.bzl").unwrap(),
                rule_name: "ordinary_repository".into(),
            },
            attributes: Arc::new(SmallMap::<_, OverrideAttributeValue>::new()),
        },
        kind: RepositoryMaterializationKind::GeneratedFileEffects(
            GeneratedRepositoryFileEffectPlan::build(files.iter().map(
                |(path, bytes, executable)| {
                    ((*path).into(), Arc::<[u8]>::from(*bytes), *executable)
                },
            ))
            .unwrap(),
        ),
    }
}

#[cfg(unix)]
#[tokio::test]
async fn actual_file_batch_preserves_prior_inputs_and_distinct_request_guards() {
    use std::os::unix::fs::PermissionsExt;
    let workspace = super::super::super::scratch();
    let root = NormalizedAbsolutePath::new(workspace.clone()).unwrap();
    let first = request(
        &root,
        "first+",
        &[
            ("BUILD.bazel", b"exports_files(['marker'])\n", false),
            ("marker", b"first", false),
        ],
    );
    let second = request(
        &root,
        "second+",
        &[("bin/ordinary_tool", b"#!/bin/sh\nexit 0\n", true)],
    );
    let need = SourcePreparationNeeds::repository(first)
        .try_union(&SourcePreparationNeeds::repository(second))
        .unwrap()
        .try_union(&SourcePreparationNeeds::repository(request(
            &root,
            "empty+",
            &[],
        )))
        .unwrap();
    assert!(file_only(&need));
    let originals = need
        .repository_materializations()
        .values()
        .cloned()
        .collect::<Vec<_>>();
    let sentinel = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(workspace.join("kept-absent")).unwrap(),
        PathObservationOperation::Lstat,
    );
    let withheld = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(workspace.join("a.bzl")).unwrap(),
        PathObservationOperation::FileBytes,
    );
    let value = Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing));
    let epoch = PathObservationEpoch::from_shared([(sentinel.clone(), value.clone())]).unwrap();
    let mut entries = super::super::super::local_repository_materialization_entries(
        &root,
        &[("prior+", "prior")],
    );
    let prior = entries[0].request.clone();
    let dice = dice::Dice::builder().build(dice::DetectCycles::Enabled);
    let mut updater = dice.updater();
    updater
        .changed_to([(PathObservationEpochKey, epoch)])
        .unwrap();
    let transaction = updater.commit().await;
    let mut tx = materialize(
        transaction,
        &workspace,
        &root,
        &need,
        &mut entries,
        &mut Budget::default(),
        &mut None,
        0,
        &[],
    )
    .await
    .unwrap();
    assert_eq!(entries.len(), 4);
    assert!(Arc::ptr_eq(&prior, &entries[0].request));
    let actual_epoch = tx.compute(&PathObservationEpochKey).await.unwrap();
    assert!(Arc::ptr_eq(actual_epoch.get(&sentinel).unwrap(), &value));
    assert!(
        actual_epoch.get(&withheld).is_none(),
        "preparation filled a withheld caller input"
    );
    assert!(
        actual_epoch
            .observations()
            .iter()
            .all(|(demand, _)| demand == &sentinel
                || matches!(
                    demand.namespace(),
                    PathObservationNamespace::Materialization(_)
                ))
    );
    let stored = tx
        .compute(&RepositoryMaterializationResultEpochKey {
            workspace: root.clone(),
        })
        .await
        .unwrap();
    let mut identities = Vec::new();
    for original in originals {
        let entry = entries
            .iter()
            .find(|entry| entry.request.id == original.id)
            .unwrap();
        assert!(Arc::ptr_eq(&entry.request, &original));
        let slug_bzlmod_v2::RepositoryMaterializationResult::Success(
            slug_bzlmod_v2::RepositoryMaterializationSuccess::Immutable {
                path_policy: _,
                source_identity,
                generation_root,
                observation_instance,
            },
        ) = &entry.result
        else {
            panic!("generated request lost Immutable result")
        };
        identities.push(source_identity.clone());
        assert_eq!(
            stored
                .borrowed_instance_for_namespace(&root, *observation_instance, 4)
                .unwrap()
                .unwrap()
                .request(),
            original.as_ref()
        );
        let RepositoryMaterializationKind::GeneratedFileEffects(plan) = &original.kind else {
            unreachable!()
        };
        for file in plan.effects() {
            let path = generation_root.join(file.path());
            assert_eq!(std::fs::read(&path).unwrap(), file.content());
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o111 != 0,
                file.executable()
            );
        }
    }
    assert_eq!(
        identities
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        3
    );
}

#[test]
fn mixed_needs_and_file_tree_conflicts_are_refused_before_writes() {
    let workspace = super::super::super::scratch();
    let root = NormalizedAbsolutePath::new(workspace.clone()).unwrap();
    let need =
        SourcePreparationNeeds::repository(request(&root, "ordinary+", &[("a", b"value", false)]));
    let path = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(workspace.join("missing")).unwrap(),
        PathObservationOperation::Lstat,
    );
    let mixed = need
        .try_union(&SourcePreparationNeeds::path(
            NeedPathObservations::singleton(path),
        ))
        .unwrap();
    let before = std::fs::read_dir(&workspace).unwrap().count();
    assert!(!file_only(&mixed));
    let conflict = SourcePreparationNeeds::repository(request(
        &root,
        "conflict+",
        &[("a", b"file", false), ("a/child", b"child", false)],
    ));
    assert!(files::preflight(&workspace, &conflict, &mut Budget::default()).is_err());
    assert_eq!(std::fs::read_dir(&workspace).unwrap().count(), before);
}

#[test]
fn materialization_request_prepayment_is_paid_before_guard_clone() {
    let root = NormalizedAbsolutePath::new(Path::new("/fixture").to_path_buf()).unwrap();
    let original = Arc::new(request(&root, "generated+", &[("marker", b"body", false)]));
    let strong = Arc::strong_count(&original);
    let mut charged = Budget::default();
    assert!(super::request::admit(&original, &mut charged).unwrap());
    assert_eq!(charged.records, 3); // One file record and two lookup/clone units.
    assert!(charged.bytes > std::mem::size_of::<SourcePreparationNeeds>());
    assert_eq!(Arc::strong_count(&original), strong);
    let mut exact = Budget {
        records: 0,
        bytes: 64 * 1024 * 1024 - charged.bytes,
    };
    assert!(super::request::admit(&original, &mut exact).unwrap());
    assert_eq!(exact.bytes, 64 * 1024 * 1024);
    let mut bytes_short = Budget {
        records: 0,
        bytes: 64 * 1024 * 1024 - charged.bytes + 1,
    };
    let before = bytes_short.bytes;
    assert!(super::request::admit(&original, &mut bytes_short).is_err());
    assert_eq!(bytes_short.bytes, before);
    let mut work_short = Budget {
        records: 65_536 - charged.records + 1,
        bytes: 0,
    };
    assert!(super::request::admit(&original, &mut work_short).is_err());
    assert_eq!(work_short.bytes, 0);
    assert_eq!(Arc::strong_count(&original), strong);
    let mut local = original.as_ref().clone();
    local.kind = RepositoryMaterializationKind::Local { logical_root: root };
    let mut untouched = Budget::default();
    assert!(!super::request::admit(&Arc::new(local), &mut untouched).unwrap());
    assert_eq!((untouched.records, untouched.bytes), (0, 0));
}
