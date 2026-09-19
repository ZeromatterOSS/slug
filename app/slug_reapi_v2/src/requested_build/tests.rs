use super::*;
use crate::ActionResult;
use crate::ExecutionEvidence;
use crate::GeneratedDirectory;
use crate::GeneratedOutput;
use crate::RemoteConfig;

fn file(path: &str, bytes: &[u8]) -> GeneratedOutput {
    GeneratedOutput::new(path, ReapiDigest::of_bytes(bytes), false)
}

fn result(
    files: Vec<GeneratedOutput>,
    trees: Vec<GeneratedDirectory>,
    hit: bool,
) -> RemoteExecutionResult {
    let mut evidence = ExecutionEvidence::reapi("test").record_action();
    if hit {
        evidence.ac_hits = 1;
    } else {
        evidence.ac_misses = 1;
    }
    // This old all-output field must never override accepted publication selection.
    evidence
        .materialized_outputs
        .push(ReapiDigest::of_bytes(b"unselected"));
    RemoteExecutionResult {
        action_digest: ReapiDigest::of_bytes(if hit { b"hit" } else { b"miss" }),
        platform_properties: BTreeMap::from([("quoted\"key".into(), "line\nvalue".into())]),
        result: ActionResult::new(files).with_output_directories(trees),
        output_blobs: Default::default(),
        evidence,
    }
}

#[test]
fn evidence_counts_all_prerequisites_but_only_published_producer_subsets() {
    let selected = file("same", b"selected");
    let child = file("nested/value", b"tree child");
    let tree = GeneratedDirectory {
        path: "tree".into(),
        tree_digest: ReapiDigest::of_bytes(b"tree"),
        root_digest: ReapiDigest::of_bytes(b"root"),
        directories: vec!["".into(), "empty".into(), "nested".into()],
        files: vec![child.clone()],
    };
    let empty = GeneratedDirectory {
        path: "emptytree".into(),
        files: vec![],
        ..tree.clone()
    };
    let mut results = vec![
        result(vec![file("same", b"other producer")], vec![], true),
        result(
            vec![selected.clone(), file("cooutput", b"unselected")],
            vec![tree, empty],
            false,
        ),
    ];
    let uploaded = ReapiDigest::of_bytes(b"uploaded");
    results[0].evidence.uploaded_digests.push(uploaded.clone());
    let outputs = [
        ActionOutput::new("same", ActionOutputKind::File),
        ActionOutput::new("tree", ActionOutputKind::Directory),
        ActionOutput::new("emptytree", ActionOutputKind::Directory),
    ];
    let materialized = materialized_digests(&results, [(1, outputs.as_slice())]).unwrap();
    assert_eq!(materialized, [selected.digest(), child.digest()]);
    let json: serde_json::Value = serde_json::from_str(&success_json(
        1,
        9,
        &results,
        &materialized,
        "daemon",
        Some(2),
    ))
    .unwrap();
    assert_eq!(json["analyzed_target_count"], 1);
    assert_eq!(json["declared_action_count"], 9);
    assert_eq!(json["reapi_actions"], 2);
    assert_eq!(json["direct_local_actions"], 0);
    assert_eq!(json["ac_hits"], 1);
    assert_eq!(json["ac_misses"], 1);
    assert_eq!(
        json["action_digests"],
        serde_json::json!(
            results
                .iter()
                .map(|r| r.action_digest.to_string())
                .collect::<Vec<_>>()
        )
    );
    assert_eq!(
        json["uploaded_digests"],
        serde_json::json!([uploaded.to_string()])
    );
    assert_eq!(
        json["materialized_outputs"],
        serde_json::json!([selected.digest().to_string(), child.digest().to_string()])
    );
    assert_eq!(json["platform_properties"]["quoted\"key"], "line\nvalue");
    assert_eq!(json["runtime_mode"], "daemon");
    assert_eq!(json["invalidated_files"], 2);
    assert_eq!(json["completed_boundary"], "reapi_native_execution");
    assert!(materialized_digests(&results, [(2, outputs.as_slice())]).is_err());
    assert!(materialized_digests(&results, [(0, outputs.as_slice())]).is_err());
}

#[test]
fn zero_action_evidence_succeeds_without_materialization_or_daemon_fields() {
    let materialized = materialized_digests(&[], []).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&success_json(1, 0, &[], &materialized, "one-shot", None)).unwrap();
    assert_eq!(json["success"], true);
    for key in [
        "reapi_actions",
        "direct_local_actions",
        "ac_hits",
        "ac_misses",
    ] {
        assert_eq!(json[key], 0);
    }
    for key in ["action_digests", "uploaded_digests", "materialized_outputs"] {
        assert_eq!(json[key], serde_json::json!([]));
    }
    assert_eq!(json["platform_properties"], serde_json::json!({}));
    assert!(json.get("invalidated_files").is_none());
}

#[test]
fn malformed_header_errors_never_retain_or_render_the_value() {
    for arg in [
        "--remote_header=sentinel-secret",
        "--remote_header==sentinel-secret",
        "--remote_header=sentinel-secret=",
    ] {
        let error = RemoteConfig::from_args(&[arg]).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("--remote_header expects key=value")
        );
        assert!(!error.to_string().contains("sentinel-secret"));
        assert!(!format!("{error:?}").contains("sentinel-secret"));
    }
}

#[test]
fn bare_remote_options_cannot_silently_drop_policy() {
    for flag in [
        "--remote_executor",
        "--remote_cache",
        "--remote_instance_name",
        "--remote_header",
        "--remote_timeout",
        "--remote_retries",
        "--remote_default_exec_properties",
    ] {
        let error = RemoteConfig::from_args(&[
            "--remote_executor=grpc://127.0.0.1:1",
            flag,
            "sentinel-secret",
        ])
        .unwrap_err();
        assert_eq!(error.to_string(), format!("{flag} must not be empty"));
        assert!(!format!("{error:?}").contains("sentinel-secret"));
    }
}
