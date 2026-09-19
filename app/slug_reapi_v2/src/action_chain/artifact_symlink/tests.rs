use super::*;
use crate::action_chain::binding::BoundInputs;

fn source(executable: bool, required: bool) -> ArtifactSymlinkTemplate {
    ArtifactSymlinkTemplate {
        output: ActionOutput::new("first", ActionOutputKind::File),
        input: Binding::Source {
            path: "source".into(),
            digest: ReapiDigest::of_bytes(b"bytes"),
            index: 4,
        },
        require_executable: required,
        source_executable: Some(executable),
    }
}
fn derived(target: &str, producer: usize, output: &str, required: bool) -> ArtifactSymlinkTemplate {
    ArtifactSymlinkTemplate {
        output: ActionOutput::new(output, ActionOutputKind::File),
        input: Binding::Generated {
            path: target.into(),
            producer,
            output: ActionOutput::new(target, ActionOutputKind::File),
        },
        require_executable: required,
        source_executable: None,
    }
}
fn remote(path: &str) -> ActionChainStepResult {
    ActionChainStepResult::Remote(RemoteExecutionResult {
        action_digest: ReapiDigest::of_bytes(b"action"),
        platform_properties: Default::default(),
        result: ActionResult::new(vec![GeneratedOutput::new(
            path,
            ReapiDigest::of_bytes(b"bytes"),
            false,
        )]),
        output_blobs: Default::default(),
        evidence: ExecutionEvidence::reapi("synthetic verified generated output"),
    })
}

#[test]
fn alias_chains_preserve_source_mode_and_terminal_byte_authority() {
    assert!(source(false, true).resolve(&[]).is_err());
    let mut missing_mode = source(true, false);
    missing_mode.source_executable = None;
    assert!(missing_mode.resolve(&[]).is_err());
    for executable in [false, true] {
        let first = source(executable, false).resolve(&[]).unwrap();
        let results = [ActionChainStepResult::ArtifactSymlink(first)];
        let second = derived("first", 0, "second", false)
            .resolve(&results)
            .unwrap();
        assert_eq!(second.is_executable(), executable);
        assert_eq!(second.digest(), &ReapiDigest::of_bytes(b"bytes"));
        assert!(matches!(second.backing(), FileBacking::Source(4)));
        assert_eq!(
            derived("first", 0, "executable", true)
                .resolve(&results)
                .is_ok(),
            executable
        );
    }
}

#[test]
fn remote_mode_false_alias_is_executable_and_local_manifest_keeps_bytes() {
    let first = derived("generated", 0, "first", true)
        .resolve(&[remote("generated")])
        .unwrap();
    assert!(first.is_executable());
    assert!(matches!(first.backing(), FileBacking::GeneratedCas));
    let second = derived("first", 0, "second", true)
        .resolve(&[ActionChainStepResult::ArtifactSymlink(first)])
        .unwrap();
    assert!(matches!(second.backing(), FileBacking::GeneratedCas));
    let local = ActionChainStepResult::from_runfiles(PreparedRunfilesAction::Manifest {
        output: ActionOutput::new("manifest", ActionOutputKind::File),
        bytes: Arc::from(&b"bytes"[..]),
    });
    let alias = derived("manifest", 0, "alias", true)
        .resolve(&[local])
        .unwrap();
    assert!(alias.is_executable());
    assert!(matches!(alias.backing(), FileBacking::Local(bytes) if bytes.as_ref() == b"bytes"));
}

#[test]
fn alias_results_require_exact_producer_and_keep_equal_digest_provenance() {
    let template = derived("first", 0, "second", false);
    for results in [
        Vec::new(),
        vec![remote("wrong")],
        vec![ActionChainStepResult::RunfilesTree {
            output: ActionOutput::new("first", ActionOutputKind::RunfilesTree),
        }],
        vec![ActionChainStepResult::from_runfiles(
            PreparedRunfilesAction::Manifest {
                output: ActionOutput::new("wrong", ActionOutputKind::File),
                bytes: Arc::from(&b"bytes"[..]),
            },
        )],
    ] {
        assert!(template.resolve(&results).is_err());
    }
    let mut wrong_alias = source(true, false).resolve(&[]).unwrap();
    wrong_alias.output = ActionOutput::new("wrong", ActionOutputKind::File);
    assert!(
        template
            .resolve(&[ActionChainStepResult::ArtifactSymlink(wrong_alias)])
            .is_err()
    );

    let source_alias = source(false, false).resolve(&[]).unwrap();
    let remote_alias = derived("generated", 0, "remote_alias", false)
        .resolve(&[remote("generated")])
        .unwrap();
    let local = ActionChainStepResult::from_runfiles(PreparedRunfilesAction::Manifest {
        output: ActionOutput::new("manifest", ActionOutputKind::File),
        bytes: Arc::from(&b"bytes"[..]),
    });
    let local_alias = derived("manifest", 0, "local_alias", false)
        .resolve(&[local])
        .unwrap();
    let mut bound = BoundInputs::default();
    for alias in [source_alias, remote_alias, local_alias] {
        let binding = Binding::Generated {
            path: alias.output().path().into(),
            producer: 0,
            output: alias.output().clone(),
        };
        bound
            .add(&binding, &[ActionChainStepResult::ArtifactSymlink(alias)])
            .unwrap();
    }
    let digest = ReapiDigest::of_bytes(b"bytes");
    assert_eq!(bound.sources.get(&digest), Some(&4));
    assert!(bound.generated.contains(&digest));
    assert_eq!(bound.local.get(&digest).unwrap().as_ref(), b"bytes");
    assert_eq!(bound.files.len(), 3);
    assert!(
        bound
            .files
            .iter()
            .all(|entry| entry.digest() == &digest && entry.is_executable())
    );
}
