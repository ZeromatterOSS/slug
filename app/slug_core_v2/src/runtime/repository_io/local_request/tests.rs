use std::cell::Cell;

use compact_str::CompactString;
use slug_bzlmod_v2::RepoRuleId;
use slug_identity_v2::ApparentLabel;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;
use starlark_map::small_map::SmallMap;

use super::*;

fn request(path: &str, root: &str) -> RepositoryMaterializationRequest {
    RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: NormalizedAbsolutePath::new("/__wp899_local_workspace__").unwrap(),
            canonical_repo: CanonicalRepoName::new("dep").unwrap(),
        },
        repo_spec: RepoSpec {
            rule_id: RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:local.bzl")
                    .unwrap(),
                rule_name: "local_repository".into(),
            },
            attributes: Arc::new(SmallMap::from_iter([(
                CompactString::new("path"),
                OverrideAttributeValue::String(path.into()),
            )])),
        },
        kind: RepositoryMaterializationKind::Local {
            logical_root: NormalizedAbsolutePath::new(root).unwrap(),
        },
    }
}

fn begin(materializer: &RepositoryMaterializer) -> RepositorySessionToken {
    let token = materializer.begin().unwrap();
    materializer
        .validate(token, &mut |_, _| {
            panic!("empty state has no source observations")
        })
        .unwrap();
    token
}

#[test]
fn native_local_request_binds_relative_absolute_root_and_normalized_dot_without_io() {
    let workspace = Path::new("/__wp899_local_workspace__");
    for (path, root) in [
        ("child", "/__wp899_local_workspace__/child"),
        ("nested/child", "/__wp899_local_workspace__/nested/child"),
        ("child/.", "/__wp899_local_workspace__/child"),
        ("child/./nested", "/__wp899_local_workspace__/child/nested"),
        ("/__wp899_external_missing__", "/__wp899_external_missing__"),
        (
            "/__wp899_external_missing__/.",
            "/__wp899_external_missing__",
        ),
        ("/", "/"),
    ] {
        assert!(validate(workspace, &request(path, root)).is_ok(), "{path}");
    }
    assert!(
        super::super::validate_local_spec(
            workspace,
            &request("/__wp899_external_missing__", "/__wp899_external_missing__").repo_spec
        )
        .is_err()
    );
}

#[test]
fn native_local_request_rejects_malformed_binding_kind_rule_and_attributes() {
    let workspace = Path::new("/__wp899_local_workspace__");
    for path in [
        "",
        ".",
        "..",
        "./child",
        "child/..",
        "child/../other",
        "/child/../other",
    ] {
        assert!(
            validate(workspace, &request(path, "/other")).is_err(),
            "{path}"
        );
    }
    for (path, root) in [("child", "/other"), ("/child", "/other")] {
        assert!(validate(workspace, &request(path, root)).is_err());
    }
    let base = request("child", "/__wp899_local_workspace__/child");
    let mut wrong = base.clone();
    wrong.kind = RepositoryMaterializationKind::Immutable;
    assert!(validate(workspace, &wrong).is_err());
    let mut wrong = base.clone();
    wrong.repo_spec.rule_id.rule_name = "http_archive".into();
    assert!(validate(workspace, &wrong).is_err());
    let mut wrong = base.clone();
    wrong.repo_spec.rule_id.bzl_file =
        CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:http.bzl").unwrap();
    assert!(validate(workspace, &wrong).is_err());
    let mut wrong = base.clone();
    wrong.repo_spec.rule_id.bzl_file =
        ApparentLabel::parse("@bazel_tools//tools/build_defs/repo:local.bzl")
            .unwrap()
            .resolve(&RepositoryMapping::new(
                RepositoryMappingId::new("mapped-local-rule").unwrap(),
            ));
    assert_eq!(
        wrong.repo_spec.rule_id.bzl_file.to_string(),
        base.repo_spec.rule_id.bzl_file.to_string()
    );
    assert!(validate(workspace, &wrong).is_err());
    let mut wrong = base.clone();
    wrong.id.workspace = NormalizedAbsolutePath::new("/foreign").unwrap();
    assert!(validate(workspace, &wrong).is_err());
    for attributes in [
        SmallMap::new(),
        SmallMap::from_iter([(
            CompactString::new("path"),
            OverrideAttributeValue::Bool(true),
        )]),
        SmallMap::from_iter([
            (
                CompactString::new("path"),
                OverrideAttributeValue::String("child".into()),
            ),
            (
                CompactString::new("extra"),
                OverrideAttributeValue::String("value".into()),
            ),
        ]),
    ] {
        let mut wrong = base.clone();
        wrong.repo_spec.attributes = Arc::new(attributes);
        assert!(validate(workspace, &wrong).is_err());
    }
}

#[test]
fn native_local_gateway_absolute_and_root_success_remain_rootless() {
    for path in ["/__wp899_external_missing__", "/"] {
        let request = Arc::new(request(path, path));
        let materializer = RepositoryMaterializer::new(request.id.workspace.clone());
        let token = begin(&materializer);
        let epoch = materializer
            .materialize_native(
                token,
                request.clone(),
                RepositoryMaterializationGeneration(1),
            )
            .unwrap();
        assert_eq!(
            epoch,
            RepositoryMaterializationResultEpoch::new(
                request.id.workspace.clone(),
                [RepositoryMaterializationEpochEntry {
                    request,
                    result: RepositoryMaterializationResult::Success(
                        RepositoryMaterializationSuccess::Local
                    ),
                }],
            )
            .unwrap()
        );
        {
            let state = materializer.state.lock().unwrap();
            assert!(state.accepted_roots.is_empty());
            assert!(state.active.as_ref().unwrap().provisional_roots.is_empty());
        }
        assert!(matches!(
            materializer.image_source(
                token,
                PathObservationNamespace::Host,
                &NormalizedAbsolutePath::new(path).unwrap(),
                None,
            ),
            Err(RepositorySessionError::InvalidImageAuthority)
        ));
        materializer.discard(token).unwrap();
    }
}

#[test]
fn native_local_gateway_refuses_before_callback_and_epoch_publication() {
    let workspace = NormalizedAbsolutePath::new("/__wp899_local_workspace__").unwrap();
    let materializer = RepositoryMaterializer::new(workspace.clone());
    let token = begin(&materializer);
    let calls = Cell::new(0);
    let mut invalid = vec![request("/outside", "/different")];
    let mut wrong_rule = request("/outside", "/outside");
    wrong_rule.repo_spec.rule_id.rule_name = "unknown_rule".into();
    invalid.push(wrong_rule);
    let mut wrong_label = request("/outside", "/outside");
    wrong_label.repo_spec.rule_id.bzl_file =
        CanonicalLabel::parse("@@foreign//tools/build_defs/repo:local.bzl").unwrap();
    invalid.push(wrong_label);
    let mut mapped = request("/outside", "/outside");
    mapped.repo_spec.rule_id.bzl_file =
        ApparentLabel::parse("@bazel_tools//tools/build_defs/repo:local.bzl")
            .unwrap()
            .resolve(&RepositoryMapping::new(
                RepositoryMappingId::new("mapped-local-rule").unwrap(),
            ));
    invalid.push(mapped);
    for attributes in [
        SmallMap::new(),
        SmallMap::from_iter([(
            CompactString::new("path"),
            OverrideAttributeValue::Bool(true),
        )]),
        SmallMap::from_iter([
            (
                CompactString::new("path"),
                OverrideAttributeValue::String("/outside".into()),
            ),
            (
                CompactString::new("extra"),
                OverrideAttributeValue::String("value".into()),
            ),
        ]),
    ] {
        let mut forged = request("/outside", "/outside");
        forged.repo_spec.attributes = Arc::new(attributes);
        invalid.push(forged);
    }
    for path in ["", ".", "..", "child/../outside", "/child/../outside"] {
        invalid.push(request(path, "/outside"));
    }
    let mut wrong_kind = request("/outside", "/outside");
    wrong_kind.kind = RepositoryMaterializationKind::Immutable;
    invalid.push(wrong_kind);
    for forged in invalid {
        assert_eq!(
            materializer
                .materialize_native_with(
                    token,
                    Arc::new(forged),
                    RepositoryMaterializationGeneration(1),
                    || {
                        calls.set(calls.get() + 1);
                        RepositoryMaterializationAttempt::Local
                    },
                )
                .unwrap_err(),
            RepositorySessionError::KindMismatch
        );
        assert_eq!(calls.get(), 0);
        assert_eq!(
            materializer.epoch(token).unwrap(),
            RepositoryMaterializationResultEpoch::new(workspace.clone(), []).unwrap()
        );
    }
    let mut foreign = request("/outside", "/outside");
    foreign.id.workspace = NormalizedAbsolutePath::new("/foreign").unwrap();
    assert_eq!(
        materializer
            .materialize_native_with(
                token,
                Arc::new(foreign),
                RepositoryMaterializationGeneration(1),
                || {
                    calls.set(calls.get() + 1);
                    RepositoryMaterializationAttempt::Local
                },
            )
            .unwrap_err(),
        RepositorySessionError::WrongWorkspace
    );
    assert_eq!(calls.get(), 0);
    materializer.discard(token).unwrap();
    assert!(matches!(
        materializer.materialize_native_with(
            token,
            Arc::new(request("/outside", "/outside")),
            RepositoryMaterializationGeneration(1),
            || {
                calls.set(calls.get() + 1);
                RepositoryMaterializationAttempt::Local
            },
        ),
        Err(RepositorySessionError::StaleToken { .. })
    ));
    assert_eq!(calls.get(), 0);
    let repaired = begin(&materializer);
    materializer
        .materialize_native_with(
            repaired,
            Arc::new(request("/outside", "/outside")),
            RepositoryMaterializationGeneration(1),
            || {
                calls.set(calls.get() + 1);
                RepositoryMaterializationAttempt::Local
            },
        )
        .unwrap();
    assert_eq!(calls.get(), 1, "valid control reaches the same callback");
    materializer.discard(repaired).unwrap();
}
