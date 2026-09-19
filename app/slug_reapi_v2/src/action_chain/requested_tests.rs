use slug_core_v2::runtime::BzlmodCommandPolicyKey;
use slug_core_v2::runtime::BzlmodEnvironmentPolicyKey;
use slug_core_v2::runtime::LockfileMode;
use slug_core_v2::runtime::TargetPattern;
use slug_core_v2::runtime::TerminalOutput;

use super::tests::Workspace;
use super::*;

const DEFS: &str = r#"def _impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + '.out')
    unused = ctx.actions.declare_file(ctx.label.name + '.unused')
    ctx.actions.write(unused, 'unselected')
    if ctx.attr.dep:
        ctx.actions.run(executable=ctx.attr.tool[DefaultInfo].files.to_list()[0], inputs=ctx.attr.dep[DefaultInfo].files, outputs=[out], arguments=['consume'])
    else:
        ctx.actions.write(out, 'shared')
    return [DefaultInfo(files=depset([out]))]
stage = rule(implementation=_impl, attrs={'dep':attr.label(), 'tool':attr.label(allow_single_file=True)})
"#;

pub(super) fn workspace() -> Workspace {
    let workspace = Workspace::new();
    std::fs::write(workspace.root.join("defs.bzl"), DEFS).unwrap();
    std::fs::write(
        workspace.root.join("BUILD.bazel"),
        "load(':defs.bzl', 'stage')\nplatform(name='platform')\nexports_files(['tools/tool', 'input'])\nstage(name='seed', tool='tools/tool')\nstage(name='one', dep=':seed', tool='tools/tool')\nstage(name='two', dep=':seed', tool='tools/tool')\n",
    )
    .unwrap();
    workspace
}

pub(super) fn prepare(workspace: &Workspace, targets: &[&str]) -> Arc<PreparedActionChainInputs> {
    let targets = targets
        .iter()
        .map(|target| TargetPattern::parse(target).unwrap())
        .collect::<Vec<_>>();
    let accepted = workspace
        .runtime
        .prepare_requested_action_inputs_with_repository_environment(
            &targets,
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            &[format!(
                "file://{}/empty-registry",
                workspace.root.display()
            )],
            Default::default(),
            Default::default(),
        )
        .unwrap();
    let mut inputs = None;
    drop(accepted.project(|value| {
        inputs = Some(value.clone());
        TerminalOutput::new(0, String::new(), String::new())
    }));
    inputs.unwrap()
}

fn transport() -> ActionChainReapiTransport {
    ActionChainReapiTransport::new(
        RemoteConfig::from_args(&["--remote_executor=grpc://127.0.0.1:1"]).unwrap(),
    )
    .unwrap()
}

pub(super) fn disconnected_session(
    inputs: Arc<PreparedActionChainInputs>,
) -> ActionChainReapiSession {
    let transport = transport();
    let channel = tonic::transport::Endpoint::from_static("http://127.0.0.1:1").connect_lazy();
    ActionChainReapiSession {
        templates: Template::prepare(&inputs, &Default::default()).unwrap(),
        inputs,
        config: transport.config,
        results: Vec::new(),
        cache: CacheClient::new(channel.clone(), String::new(), TransferPolicy::default()).unwrap(),
        channel,
        identity: Arc::new(()),
    }
}

#[test]
fn requested_templates_bind_shared_producer_and_keep_all_root_results() {
    let workspace = workspace();
    let inputs = prepare(&workspace, &["//:one", "//:two", "//:input", "//:one"]);
    let plan = inputs.plan().unwrap();
    assert!(plan.selected_action().is_none());
    let requested = plan.requested().unwrap();
    assert_eq!(requested.selection().targets().len(), 4);
    assert_eq!(requested.artifact_producers(), [Some(1), Some(2), None]);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let _entered = runtime.enter();
    let mut session = disconnected_session(inputs.clone());
    assert_eq!(
        session.templates.len(),
        3,
        "shared producer once; unrelated writes excluded"
    );
    let digest = ReapiDigest::of_bytes(b"verified shared output");
    for (index, step) in plan.actions().iter().enumerate() {
        if index > 0 {
            assert!(session.templates[index].bind(&[]).is_err());
        }
        let bound = session.templates[index].bind(&session.results).unwrap();
        if index > 0 {
            let shared = bound
                .tree
                .entries()
                .iter()
                .find(|entry| entry.path() == "seed.out")
                .unwrap();
            assert_eq!(shared.digest(), &digest);
            assert!(shared.is_executable());
            assert_eq!(bound.generated, BTreeSet::from([digest.clone()]));
        }
        session.results.push(RemoteExecutionResult {
            action_digest: ReapiDigest::of_bytes(&[index as u8]),
            platform_properties: Default::default(),
            result: ActionResult::new(
                step.action()
                    .outputs()
                    .iter()
                    .map(|output| GeneratedOutput::new(output.path(), digest.clone(), false))
                    .collect(),
            ),
            output_blobs: Default::default(),
            evidence: ExecutionEvidence::reapi("synthetic verified metadata"),
        });
    }
    let result = runtime.block_on(transport().finish(session)).unwrap();
    assert!(result.selected().is_none());
    assert_eq!(result.results().len(), 3);
    for (artifact, producer) in requested
        .selection()
        .artifacts()
        .iter()
        .zip(requested.artifact_producers())
    {
        if let Some(producer) = producer {
            assert_eq!(
                result.results()[*producer].result.output_files()[0].path(),
                artifact.path()
            );
        } else {
            assert!(matches!(artifact, AnalysisArtifact::Source(_)));
        }
    }
}

#[test]
fn requested_later_policy_failure_precedes_connection() {
    let workspace = workspace();
    std::fs::write(workspace.root.join("defs.bzl"), DEFS.replace(
        "arguments=['consume']",
        "arguments=['consume'], execution_requirements={'no-remote':'1'} if ctx.label.name == 'two' else {}",
    )).unwrap();
    let inputs = prepare(&workspace, &["//:one", "//:two"]);
    assert_eq!(inputs.plan().unwrap().actions().len(), 3);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let error = match runtime.block_on(transport().start(inputs)) {
        Ok(_) => panic!("unsupported later root started a session"),
        Err(error) => error,
    };
    assert!(matches!(error, RemoteExecutionError::Command(_)), "{error}");
    assert!(
        error.to_string().contains("execution requirements"),
        "{error}"
    );
}

#[test]
fn requested_source_only_does_not_connect_real_reapi_transport() {
    let workspace = workspace();
    let accepted = workspace
        .runtime
        .execute_requested_actions_with_repository_environment(
            &[TargetPattern::parse("//:input").unwrap()],
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            &[format!(
                "file://{}/empty-registry",
                workspace.root.display()
            )],
            Default::default(),
            Default::default(),
            &transport(),
        )
        .unwrap();
    drop(accepted.project(|result| {
        assert!(result.output().is_none());
        let plan = result.inputs().plan().unwrap();
        assert!(plan.actions().is_empty());
        assert_eq!(plan.requested().unwrap().artifact_producers(), [None]);
        assert_eq!(result.inputs().sources().len(), 1);
        TerminalOutput::new(0, String::new(), String::new())
    }));
}
