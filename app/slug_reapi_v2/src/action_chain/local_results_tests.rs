use super::tests::Workspace;
use super::*;

fn local(path: &str, bytes: &[u8]) -> ActionChainStepResult {
    ActionChainStepResult::from_runfiles(PreparedRunfilesAction::Manifest {
        output: ActionOutput::new(path, ActionOutputKind::File),
        bytes: bytes.into(),
    })
}

#[test]
fn local_manifest_binding_preserves_plan_ordinals_and_remote_cas_provenance() {
    let workspace = requested_tests::workspace();
    let inputs = requested_tests::prepare(&workspace, &["//:one", "//:two"]);
    let mut templates = Template::prepare(&inputs, &Default::default()).unwrap();
    let local_result = local("seed.out", b"manifest bytes\n");
    let digest = local_result.local_file().unwrap().digest().clone();
    let Template::Spawn {
        inputs: bindings, ..
    } = &mut templates[2]
    else {
        panic!("consumer")
    };
    // A later consumer sees a local file at ordinal zero and remote metadata at
    // ordinal one. Equal bytes must retain the remote producer's CAS obligation.
    bindings.push(Binding::Generated {
        producer: 1,
        path: "remote".to_owned(),
        output: ActionOutput::new("remote", ActionOutputKind::File),
    });
    let remote = ActionChainStepResult::Remote(RemoteExecutionResult {
        action_digest: ReapiDigest::of_bytes(b"action"),
        platform_properties: Default::default(),
        result: ActionResult::new(vec![GeneratedOutput::new("remote", digest.clone(), false)]),
        output_blobs: Default::default(),
        evidence: ExecutionEvidence::reapi("synthetic verified remote metadata"),
    });
    let mut results = vec![local_result, remote];
    let bound = templates[2].bind(&results).unwrap();
    assert_eq!(
        bound.local.get(&digest).unwrap().as_ref(),
        b"manifest bytes\n"
    );
    assert!(bound.generated.contains(&digest));
    for path in ["seed.out", "remote"] {
        let entry = bound
            .tree
            .entries()
            .iter()
            .find(|entry| entry.path() == path)
            .unwrap();
        assert_eq!(entry.digest(), &digest);
        assert!(entry.is_executable());
    }
    results[0] = local("other", b"manifest bytes\n");
    assert!(templates[2].bind(&results).is_err());
    results[0] = ActionChainStepResult::RunfilesTree {
        output: ActionOutput::new("seed.out", ActionOutputKind::RunfilesTree),
    };
    assert!(templates[2].bind(&results).is_err());
}

#[test]
#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn requested_local_manifest_completes_without_backend_or_remote_result() {
    let workspace = Workspace::new();
    std::fs::write(workspace.root.join("defs.bzl"), r##"def _impl(ctx):
    out = ctx.actions.declare_file("binary")
    ctx.actions.write(out, "#!/bin/sh\n", is_executable = True)
    return [DefaultInfo(executable = out, runfiles = ctx.runfiles(files = ctx.attr.input[DefaultInfo].files.to_list()))]
stage = rule(implementation = _impl, executable = True, attrs = {"input": attr.label(allow_single_file = True), "mapping": attr.output(), "manifest": attr.output()})
"##).unwrap();
    std::fs::write(workspace.root.join("BUILD.bazel"), "load(':defs.bzl', 'stage')\nplatform(name='platform')\nexports_files(['input'])\nstage(name='one', input='input', mapping='binary.repo_mapping', manifest='binary.runfiles/MANIFEST')\n").unwrap();
    let requested = requested_tests::prepare(&workspace, &["//:one"]);
    let plan = requested.plan().unwrap();
    assert_eq!(plan.actions().len(), 5);
    // Native analysis may claim configured-output sidecars beneath bazel-out.
    // No declared action output may be created by preparation or local execution.
    let outputs = plan
        .actions()
        .iter()
        .flat_map(|step| {
            let action = step.action();
            let root = slug_core_v2::runtime::configured_output_root(
                &workspace.root,
                action
                    .context()
                    .owner()
                    .configuration()
                    .slug_configuration()
                    .unwrap(),
            );
            action
                .outputs()
                .iter()
                .map(move |output| root.join(output.path()))
        })
        .collect::<Vec<_>>();
    let no_outputs = || {
        for path in &outputs {
            assert_eq!(
                std::fs::symlink_metadata(path).unwrap_err().kind(),
                std::io::ErrorKind::NotFound,
                "unexpected action output {}",
                path.display()
            );
        }
    };
    no_outputs();
    let templates = Template::prepare(&requested, &Default::default()).unwrap();
    assert_eq!(
        templates
            .iter()
            .filter(|template| matches!(template, Template::Runfiles(_)))
            .count(),
        4
    );
    let inputs = requested_tests::prepare(&workspace, &["//:binary.repo_mapping"]);
    assert_eq!(inputs.plan().unwrap().actions().len(), 1);
    let transport = ActionChainReapiTransport::new(
        RemoteConfig::from_args(&["--remote_executor=grpc://127.0.0.1:1"]).unwrap(),
    )
    .unwrap();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut session = transport.start(inputs.clone()).await.unwrap();
            let mut other = transport.start(inputs).await.unwrap();
            let foreign = transport.stage(&mut other, 0).await.unwrap();
            assert!(transport.execute(&mut session, 0, foreign).await.is_err());
            let staged = transport.stage(&mut session, 0).await.unwrap();
            transport.execute(&mut session, 0, staged).await.unwrap();
            assert!(transport.stage(&mut session, 0).await.is_err());
            let result = transport.finish(session).await.unwrap();
            assert_eq!(result.results().len(), 1);
            assert!(result.selected().is_none());
            let selected = &result.results()[0];
            assert!(selected.remote().is_none());
            let local = selected.local_file().unwrap();
            assert_eq!(local.digest(), &ReapiDigest::of_bytes(local.bytes()));
            assert_eq!(local.output().path(), "binary.repo_mapping");
        });
    no_outputs();
}
