use dice::DetectCycles;
use dice::Dice;

use super::*;
use crate::keys::WorkspaceSnapshot;
use crate::keys::WorkspaceSnapshotKey;

#[tokio::test]
async fn host_load_keys_preserve_raw_alias_order_and_retained_exports() {
    use dice::UserComputationData;
    use slug_bzlmod_v2::RootPackagePolicyInputs;
    use slug_bzlmod_v2::inject_root_package_policy_inputs;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationEpochKey;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;
    use slug_workspace_v2::path_observation_shards;
    use starlark::eval::FileLoader;

    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    let source: Arc<[u8]> = Arc::from(
        concat!(
            "load(':leaf.bzl', left='name')\n",
            "load('//:leaf.bzl', alias='name')\n",
            "load(':leaf.bzl', repeated='name')\n",
            "load(':other.bzl', other='name')\n",
            "def answer():\n    return left() + ':' + alias() + ':' + repeated() + ':' + other()\n",
        )
        .as_bytes(),
    );
    let leaf_source = format!(
        "{}NAME = 'leaf'\ndef name():\n    return NAME\n",
        (0..64)
            .map(|i| format!("load(':other.bzl', ignored_{i}='NAME')\n"))
            .collect::<String>(),
    );
    let demand = |path: &str, operation| {
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(path).unwrap(),
            operation,
        )
    };
    let mut rows = Vec::new();
    for path in ["/", "/workspace"] {
        rows.push((
            demand(path, PathObservationOperation::Lstat),
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                PathNodeKind::Directory,
                1,
                1,
                1,
                1,
                0o755,
            ))),
        ));
    }
    for path in ["/workspace/REPO.bazel", "/workspace/.bazelignore"] {
        rows.push((
            demand(path, PathObservationOperation::Lstat),
            PathObservationResult::Lstat(PathOperationResult::Missing),
        ));
    }
    for (path, bytes) in [
        ("/workspace/BUILD.bazel", Arc::<[u8]>::from(&b""[..])),
        ("/workspace/parent.bzl", source),
        ("/workspace/leaf.bzl", Arc::from(leaf_source.as_bytes())),
        (
            "/workspace/other.bzl",
            Arc::from(&b"NAME = 'other'\ndef name():\n    return NAME\n"[..]),
        ),
    ] {
        rows.push((
            demand(path, PathObservationOperation::Lstat),
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                PathNodeKind::RegularFile,
                1,
                1,
                1,
                1,
                0o644,
            ))),
        ));
        rows.push((
            demand(path, PathObservationOperation::FileBytes),
            PathObservationResult::FileBytes(PathOperationResult::Present(bytes)),
        ));
    }
    let mut failure_rows = rows.clone();
    failure_rows
        .iter_mut()
        .find(|(key, _)| *key == demand("/workspace/leaf.bzl", PathObservationOperation::FileBytes))
        .unwrap()
        .1 = PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
        &b"fail('child fixture failure')\n"[..],
    )));
    let epoch = PathObservationEpoch::new(rows).unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(UserComputationData {
        cycle_detector: Some(crate::bzl_load_cycle_detector()),
        ..Default::default()
    });
    updater.changed_to(path_observation_shards(&epoch)).unwrap();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch)])
        .unwrap();
    inject_root_package_policy_inputs(
        &mut updater,
        RootPackagePolicyInputs::new(
            workspace.clone(),
            [workspace.clone()],
            std::iter::empty::<&str>(),
            None,
            Some("warning"),
        )
        .unwrap(),
    )
    .unwrap();
    let mut transaction = updater.commit().await;
    let label = resolve_host_load_label(&PackagePath::root(), ":parent.bzl").unwrap();
    let outcome = transaction
        .compute(&HostBzlModuleEvalKey::new(workspace.dupe(), label.clone()))
        .await
        .unwrap();
    let SourcePreparationOutcome::Complete(outcome) = outcome else {
        panic!("complete Host fixture returned Need")
    };
    let held = outcome.as_ref().as_ref().unwrap().clone();
    assert_eq!(
        held.loads,
        [":leaf.bzl", "//:leaf.bzl", ":leaf.bzl", ":other.bzl"]
    );
    assert_eq!(held.manifest.direct_children.len(), 2);
    assert_eq!(held.retained_bzl_modules.len(), 2);
    assert_eq!(
        held.retained_bzl_modules[0].identity.label.to_string(),
        "@@//:leaf.bzl"
    );
    assert_eq!(
        held.retained_bzl_modules[1].identity.label.to_string(),
        "@@//:other.bzl"
    );
    // Distinct genuinely evaluated child heaps discriminate first-match order.
    for (first, second, expected) in [(0, 1, "leaf"), (1, 0, "other")] {
        let loader = LocalBzlLoader {
            modules: vec![
                (
                    ":duplicate.bzl",
                    held.retained_bzl_modules[first].module.dupe(),
                ),
                (
                    ":duplicate.bzl",
                    held.retained_bzl_modules[second].module.dupe(),
                ),
            ],
        };
        assert_eq!(
            loader
                .load(":duplicate.bzl")
                .unwrap()
                .get("NAME")
                .unwrap()
                .value()
                .unpack_str(),
            Some(expected)
        );
    }
    let leaf_label = resolve_host_load_label(&PackagePath::root(), ":leaf.bzl").unwrap();
    let SourcePreparationOutcome::Complete(child) = transaction
        .compute(&HostBzlModuleEvalKey::new(
            workspace.dupe(),
            leaf_label.clone(),
        ))
        .await
        .unwrap()
    else {
        panic!("cached child returned Need")
    };
    let SourcePreparationOutcome::Complete(Ok(observed)) = transaction
        .compute(&HostBzlModuleObservationKey::new(
            workspace.dupe(),
            leaf_label.clone(),
        ))
        .await
        .unwrap()
    else {
        panic!("observed child returned Need/error")
    };
    for carrier in [&child, &observed.result] {
        let borrowed = successful_host_bzl_module(carrier);
        assert!(std::ptr::eq(borrowed, carrier.as_ref().as_ref().unwrap()));
        assert_eq!(borrowed.loads, vec![":other.bzl"; 64]);
        let independent: FrozenBzlModule = borrowed.clone();
        assert_ne!(borrowed.loads.as_ptr(), independent.loads.as_ptr());
        assert_ne!(borrowed.loads[0].as_ptr(), independent.loads[0].as_ptr());
        assert_ne!(
            borrowed.path.as_os_str().as_encoded_bytes().as_ptr(),
            independent.path.as_os_str().as_encoded_bytes().as_ptr()
        );
        let scratch = vec![(":leaf.bzl", Arc::clone(carrier))];
        let projected = successful_host_bzl_module(&scratch[0].1);
        assert!(std::ptr::eq(projected, borrowed));
        assert_eq!(projected.loads[0].as_ptr(), borrowed.loads[0].as_ptr());
        assert_eq!(
            projected.path.as_os_str().as_encoded_bytes().as_ptr(),
            borrowed.path.as_os_str().as_encoded_bytes().as_ptr()
        );
        let closure = flattened_module_lifetime_closure([projected]);
        drop(scratch);
        assert_eq!(closure.len(), 2);
        assert_eq!(
            closure[0].module.get("NAME").unwrap().value().unpack_str(),
            Some("leaf")
        );
    }
    drop(observed);
    drop(child);
    let failure_epoch = PathObservationEpoch::new(failure_rows).unwrap();
    let mut failed_updater = dice.updater_with_data(UserComputationData {
        cycle_detector: Some(crate::bzl_load_cycle_detector()),
        ..Default::default()
    });
    failed_updater
        .changed_to(path_observation_shards(&failure_epoch))
        .unwrap();
    failed_updater
        .changed_to(vec![(PathObservationEpochKey, failure_epoch)])
        .unwrap();
    let mut failed_transaction = failed_updater.commit().await;
    let SourcePreparationOutcome::Complete(Ok(failed_parent)) = failed_transaction
        .compute(&HostBzlModuleObservationKey::new(workspace.dupe(), label))
        .await
        .unwrap()
    else {
        panic!("failed parent returned Need/outer error")
    };
    let SourcePreparationOutcome::Complete(Ok(failed_child)) = failed_transaction
        .compute(&HostBzlModuleObservationKey::new(
            workspace,
            leaf_label.clone(),
        ))
        .await
        .unwrap()
    else {
        panic!("failed child returned Need/outer error")
    };
    let Err(HostBzlModuleError::Child { load, label, error }) = failed_parent.result() else {
        panic!("child error was not wrapped")
    };
    assert_eq!(load.as_ref(), ":leaf.bzl");
    assert_eq!(label, &leaf_label);
    assert_eq!(error.as_ref(), failed_child.result().as_ref().unwrap_err());
    for path in ["/workspace/parent.bzl", "/workspace/leaf.bzl"] {
        assert!(
            failed_parent
                .observations()
                .get(&demand(path, PathObservationOperation::FileBytes,))
                .is_some()
        );
    }
    drop(failed_child);
    drop(failed_parent);
    drop(failed_transaction);
    drop(outcome);
    drop(transaction);
    dice.wait_for_idle().await;
    drop(dice);
    // Cancellation is drained; the persistent DICE graph drops asynchronously.
    // This checks retained exports after local handle release, not source retirement.
    let answer = held.module.get("answer").unwrap();
    let caller = Module::new();
    let mut evaluator = Evaluator::new(&caller);
    assert_eq!(
        evaluator
            .eval_function(answer.value(), &[], &[])
            .unwrap()
            .unpack_str(),
        Some("leaf:leaf:leaf:other")
    );
    assert_eq!(held.loads[1], "//:leaf.bzl");
}

#[tokio::test]
async fn parsed_source_shares_workspace_storage_and_survives_its_owners() {
    let workspace = PathBuf::from("/workspace");
    let path = workspace.join("shared.bzl");
    let source = Arc::new(
        "load(\":first.bzl\", \"first\")\nload(\":second.bzl\", \"second\")\nvalue = \"é\"\n"
            .to_owned(),
    );
    let expected_digest = digest(&source);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: workspace.clone(),
            },
            Arc::new(WorkspaceSnapshot {
                files: Arc::new(
                    [(path.clone(), WorkspaceFileValue::Present(source.clone()))]
                        .into_iter()
                        .collect(),
                ),
            }),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    let result = transaction
        .compute(&BzlParseKey { workspace, path })
        .await
        .unwrap();
    let parsed = result.as_ref().as_ref().unwrap();
    assert_eq!(parsed.source.as_ptr(), source.as_ptr());
    assert_eq!(parsed.source.as_str(), source.as_str());
    assert_eq!(parsed.source_digest, expected_digest);
    assert_eq!(parsed.loads, [":first.bzl", ":second.bzl"]);
    let held = parsed.clone();
    assert_eq!(held.source.as_ptr(), parsed.source.as_ptr());

    drop(result);
    drop(transaction);
    drop(dice);
    drop(source);
    assert!(held.source.contains("value = \"é\""));
    assert_eq!(held.source_digest, expected_digest);

    let independent = ParsedBzl {
        source: Arc::new(held.source.as_ref().clone()),
        loads: held.loads.clone(),
        source_digest: expected_digest,
    };
    assert_ne!(held.source.as_ptr(), independent.source.as_ptr());
    assert!(BzlParseKey::equality(
        &Arc::new(Ok(held)),
        &Arc::new(Ok(independent)),
    ));
}

// Opt-in profiles receive only one verified regular-file descriptor as stdin.
// They do not evaluate a module, dispatch child loads, or certify a repository.
#[cfg(target_os = "linux")]
fn native_source_profile(name: &str, expected_bytes: usize, expected_sha256: &str) {
    use std::fs::File;
    use std::io::Read;
    use std::os::fd::AsFd;

    use sha2::Digest;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;
    use starlark::syntax::StringEncoding;

    fn phase(name: &str) {
        let status = std::fs::read_to_string("/proc/self/status").unwrap();
        let rss = status
            .lines()
            .find_map(|line| line.strip_prefix("VmRSS:"))
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse::<usize>()
            .unwrap();
        println!("SLUG_NATIVE_PHASE {name} {rss}");
    }

    phase("baseline");
    assert!(expected_bytes < 16 * 1024 * 1024);
    // A direct owned duplicate shares the supervisor's file offset. Avoid stdin's
    // buffered reader, which could prefetch beyond the one-byte EOF probe.
    let mut file = File::from(std::io::stdin().as_fd().try_clone_to_owned().unwrap());
    assert!(file.metadata().unwrap().is_file());
    assert_eq!(file.metadata().unwrap().len(), expected_bytes as u64);
    let mut bytes = Vec::with_capacity(expected_bytes);
    bytes.resize(expected_bytes, 0);
    file.read_exact(&mut bytes).unwrap();
    let mut probe = [0u8; 1];
    assert_eq!(file.read(&mut probe).unwrap(), 0, "source grew beyond pin");
    drop(file);
    assert_eq!(
        format!("{:x}", sha2::Sha256::digest(&bytes)),
        expected_sha256
    );
    let transferred_pointer = bytes.as_ptr();
    let source = Arc::new(String::from_utf8(bytes).expect("pinned source is UTF8"));
    assert_eq!(source.as_ptr(), transferred_pointer);
    let source_weak = Arc::downgrade(&source);
    println!(
        "SLUG_NATIVE_SOURCE {} {} {}",
        source.len(),
        source.capacity(),
        expected_sha256,
    );
    phase("after_input");

    // This whole String scratch is required by the current production parser API.
    // It is an explicitly measured copy, separate from the retained source Arc.
    let parser_source = source.as_ref().clone();
    let parser_pointer = parser_source.as_ptr();
    assert_ne!(parser_pointer, source.as_ptr());
    println!("SLUG_NATIVE_PARSER_CAPACITY {}", parser_source.capacity());
    let ast = match AstModule::parse_with_string_encoding(
        name,
        parser_source,
        &Dialect::Bazel,
        StringEncoding::BazelInternal,
    ) {
        Ok(ast) => ast,
        Err(_) => panic!("native source syntax refused"),
    };
    assert_eq!(
        ast.file_span(ast.statement().span).file.source().as_ptr(),
        parser_pointer,
    );
    phase("after_parse");
    let compact_loads = ast
        .loads()
        .into_iter()
        .map(|load| {
            assert!(load.module_id.len() <= 1024);
            let span = load.span.resolve_span_for_reporting();
            (
                load.module_id.to_owned(),
                span.begin.line,
                span.begin.column,
                span.end.line,
                span.end.column,
            )
        })
        .collect::<Vec<_>>();
    assert!(compact_loads.len() <= 128);
    drop(ast);
    phase("after_ast_retirement");
    assert_eq!(Arc::strong_count(&source), 1);
    assert!(source_weak.upgrade().is_some());
    println!(
        "SLUG_NATIVE_LOAD_STORAGE {} {}",
        compact_loads.len(),
        compact_loads
            .iter()
            .map(|load| load.0.capacity())
            .sum::<usize>(),
    );
    drop(source);
    assert!(source_weak.upgrade().is_none());
    phase("after_source_retirement");
    // Only owned names and scalar locations remain; no FileSpan/CodeMap is kept.
    for (label, begin_line, begin_column, end_line, end_column) in &compact_loads {
        print!("SLUG_NATIVE_LOAD {begin_line} {begin_column} {end_line} {end_column} ");
        for byte in label.bytes() {
            print!("{byte:02x}");
        }
        println!();
    }
    drop(compact_loads);
    phase("after_final_drop");
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires the bounded native source profile supervisor and verified stdin"]
fn native_bazel_internal_rustc_source_profile() {
    native_source_profile(
        "rust/private/rustc.bzl",
        145_040,
        "a7712508f50e5952f3f51e33c98acbe8ba39554e9c6f6edc26f16820d7c2a9e4",
    );
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires the bounded native source profile supervisor and verified stdin"]
fn native_bazel_internal_known_shas_source_profile() {
    native_source_profile(
        "rust/private/known_shas.bzl",
        4_749_878,
        "f35c95378eb10fd57f719e42fd88fea2b1c15b5049556daf8cdc18ef385ff446",
    );
}
