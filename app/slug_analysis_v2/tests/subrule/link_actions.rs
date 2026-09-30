//! Default link-action adapters preserve existing FileWrite owners and guards.
use super::*;

const TARGET: &str = "@@//subject:link_actions";

fn epoch(marker: &str) -> PathObservationEpoch {
    let mut epoch = Epoch::fixture();
    epoch.file(
        "/workspace/MODULE.bazel",
        "module(name='root')\nbazel_dep(name='rules_cc', version='1.0')\nlocal_path_override(module_name='rules_cc', path='rules_cc')\n",
    );
    epoch.directory("/workspace/rules_cc");
    epoch.file(
        "/workspace/rules_cc/MODULE.bazel",
        "module(name='rules_cc', version='1.0')\n",
    );
    epoch.missing("/workspace/rules_cc/REPO.bazel");
    epoch.missing("/workspace/rules_cc/.bazelignore");
    epoch.package("rules_cc/rules", "");
    epoch.file(
        "/workspace/rules_cc/rules/link_actions.bzl",
        &format!(
            r#"i = cc_common.internal_DO_NOT_USE()
Proof = provider(fields = ["root", "sub", "root_soname", "sub_soname", "marker"])
def _wrap(actions):
    first = i.wrap_link_actions(actions)
    second = i.wrap_link_actions(first, None)
    return i.wrap_link_actions(second, None, False)
def _child(ctx, parent):
    wrapped = _wrap(ctx.actions)
    if i.actions2ctx_cheat(wrapped) != parent:
        fail("wrapped subrule actions did not recover the root ctx")
    output = wrapped.declare_file("sub_{marker}.pic.a")
    if output != ctx.actions.declare_file("sub_{marker}.pic.a"):
        fail("wrapped subrule declaration lost File identity")
    soname = i.dynamic_library_soname(wrapped, output.short_path, True)
    wrapped.write(output, "sub:{marker}")
    return output, soname
child = subrule(implementation = _child)
def _root(ctx):
    wrapped = _wrap(ctx.actions)
    if i.actions2ctx_cheat(wrapped) != ctx:
        fail("nested wrapped actions did not recover their ctx")
    output = ctx.actions.declare_file("root_{marker}.a")
    if output != wrapped.declare_file("root_{marker}.a"):
        fail("wrapped root declaration lost File identity")
    root_soname = i.dynamic_library_soname(wrapped, output.short_path, True)
    wrapped.write(output, "root:{marker}")
    sub, sub_soname = child(ctx)
    if sub != ctx.actions.declare_file("sub_{marker}.pic.a"):
        fail("root and subrule wrappers used different declaration owners")
    return [DefaultInfo(files = depset([output, sub])), Proof(root = output, sub = sub, root_soname = root_soname, sub_soname = sub_soname, marker = "{marker}")]
subject = rule(implementation = _root, subrules = [child])
def _locked(ctx, outer):
    wrapped = _wrap(outer)
    wrapped.declare_file("forbidden.txt")
locked = subrule(implementation = _locked)
def _escaped(ctx, method = False):
    wrapped = _wrap(ctx.actions)
    return wrapped.declare_file if method else wrapped
escaped = subrule(implementation = _escaped)
def _repeat(ctx, old = None):
    if old != None:
        old.declare_file("forbidden.txt")
    return _wrap(ctx.actions)
repeat = subrule(implementation = _repeat)
def _scope(ctx):
    wrapped = _wrap(ctx.actions)
    output = wrapped.declare_file("scope.txt")
    wrapped.run(outputs = [output], executable = "runner", toolchain = None)
scope = subrule(implementation = _scope)
def _negative(ctx):
    mode = ctx.attr.mode
    if mode == "locked":
        locked(ctx.actions)
    elif mode == "expired":
        escaped().declare_file("forbidden.txt")
    elif mode == "method":
        escaped(True)("forbidden.txt")
    elif mode == "repeat":
        repeat(repeat())
    elif mode == "scope":
        scope()
    elif mode == "publication":
        return [DefaultInfo(), Proof(root = _wrap(ctx.actions), sub = None, marker = "{marker}")]
    else:
        wrapped = _wrap(ctx.actions)
        output = ctx.actions.declare_file("conflict.txt")
        if output != wrapped.declare_file("conflict.txt"):
            fail("conflicting declarations lost File identity")
        ctx.actions.write(output, "base")
        wrapped.write(output, "wrapper")
    fail("wrapped action guard was bypassed")
negative = rule(implementation = _negative, attrs = {{"mode": attr.string()}}, subrules = [locked, escaped, repeat, scope])
"#
        ),
    );
    let mut build = String::from(
        "load('@rules_cc//rules:link_actions.bzl', 'subject', 'negative')\nsubject(name='link_actions')\n",
    );
    for mode in [
        "conflict",
        "locked",
        "expired",
        "method",
        "repeat",
        "scope",
        "publication",
    ] {
        build.push_str(&format!("negative(name='bad_{mode}', mode='{mode}')\n"));
    }
    epoch.package("subject", &build);
    epoch.build()
}

async fn request(
    dice: &Arc<Dice>,
    tracker: &Arc<Tracker>,
    route: AnalysisRoute,
    marker: &str,
    configuration: ConfigurationKey,
) -> Arc<slug_analysis_v2::ConfiguredNodeResult> {
    configured_request(dice, tracker, route, marker, TARGET, configuration)
        .await
        .unwrap()
}

async fn configured_request(
    dice: &Arc<Dice>,
    tracker: &Arc<Tracker>,
    route: AnalysisRoute,
    marker: &str,
    target: &str,
    configuration: ConfigurationKey,
) -> Result<Arc<slug_analysis_v2::ConfiguredNodeResult>, String> {
    let mut attributes = SmallMap::new();
    attributes.insert(
        "path".into(),
        OverrideAttributeValue::String("rules_cc".into()),
    );
    let materialization = RepositoryMaterializationEpochEntry {
        request: Arc::new(RepositoryMaterializationRequest {
            id: RepositoryMaterializationRequestId {
                workspace: workspace(),
                canonical_repo: CanonicalRepoName::new("rules_cc+").unwrap(),
            },
            repo_spec: RepoSpec {
                rule_id: RepoRuleId {
                    bzl_file: CanonicalLabel::parse(
                        "@@bazel_tools//tools/build_defs/repo:local.bzl",
                    )
                    .unwrap(),
                    rule_name: "local_repository".into(),
                },
                attributes: Arc::new(attributes),
            },
            kind: RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new("/workspace/rules_cc").unwrap(),
            },
        }),
        result: RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Local),
    };
    analyze_result_with_materializations(
        dice,
        epoch(marker),
        target,
        configuration,
        tracker.clone(),
        route,
        vec![materialization],
    )
    .await
}

fn assert_writes(result: &slug_analysis_v2::ConfiguredNodeResult, marker: &str) {
    assert_eq!(result.actions().len(), 2);
    let proof = result
        .providers()
        .occurrences()
        .find(|provider| provider.field("marker").is_some())
        .unwrap();
    assert!(
        matches!(proof.field("marker").unwrap().kind(), AnalysisValueKind::String(actual) if actual == marker)
    );
    for (action, (field, suffix)) in result
        .actions()
        .iter()
        .zip([("root", ".a"), ("sub", ".pic.a")])
    {
        let basename = format!("{field}_{marker}{suffix}");
        assert!(
            matches!(proof.field(&format!("{field}_soname")).unwrap().kind(), AnalysisValueKind::String(actual) if actual == basename)
        );
        assert_eq!(action.mnemonic(), "FileWrite");
        assert_eq!(
            action.kind(),
            &slug_build_api_v2::ActionKind::Write {
                content: format!("{field}:{marker}"),
                is_executable: false,
            }
        );
        assert_eq!(action.outputs().len(), 1);
        assert_eq!(
            action.outputs()[0].root_relative_path(),
            format!("subject/{basename}")
        );
        let AnalysisValueKind::Artifact(file) = proof.field(field).unwrap().kind() else {
            panic!("{field} must retain its File identity");
        };
        assert_eq!(file.path().as_ref(), action.outputs()[0].path());
        assert_eq!(
            action.outputs()[0].root().configuration(),
            result
                .configured_target_key()
                .unwrap()
                .configuration()
                .slug_configuration()
                .unwrap()
        );
    }
}

#[tokio::test]
async fn wrapped_link_actions_share_filewrite_owners_and_restore_source_configuration() {
    for route in [AnalysisRoute::Legacy, AnalysisRoute::Observed] {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        let configuration = coverage_configuration(false, None);
        let first = request(&dice, &tracker, route, "A", configuration.clone()).await;
        assert_writes(&first, "A");
        let configured = request(
            &dice,
            &tracker,
            route,
            "A",
            coverage_configuration(true, None),
        )
        .await;
        assert_writes(&configured, "A");
        assert_ne!(configured, first);
        assert_eq!(
            request(&dice, &tracker, route, "A", configuration.clone()).await,
            first
        );

        let changed = request(&dice, &tracker, route, "B", configuration.clone()).await;
        assert_writes(&changed, "B");
        assert_ne!(changed, first);
        assert_eq!(
            request(&dice, &tracker, route, "A", configuration).await,
            first
        );
    }
}

#[tokio::test]
async fn wrapped_link_actions_preserve_conflicts_locked_outer_and_expired_subrule_scope() {
    for route in [AnalysisRoute::Legacy, AnalysisRoute::Observed] {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        for (mode, expected) in [
            ("conflict", "action output declared more than once"),
            (
                "locked",
                "cannot access field or method 'declare_file' of rule context",
            ),
            (
                "expired",
                "cannot access field or method 'declare_file' of subrule context",
            ),
            (
                "method",
                "cannot access field or method 'declare_file' of subrule context",
            ),
            (
                "repeat",
                "cannot access field or method 'declare_file' of subrule context",
            ),
            ("scope", "toolchain may not be supplied in a subrule"),
            (
                "publication",
                "unsupported analysis value of type `analysis_actions`",
            ),
        ] {
            let error = configured_request(
                &dice,
                &tracker,
                route,
                "A",
                &format!("@@//subject:bad_{mode}"),
                configuration(None),
            )
            .await
            .unwrap_err();
            assert!(error.contains(expected), "{mode}: {error}");
        }
    }
}
