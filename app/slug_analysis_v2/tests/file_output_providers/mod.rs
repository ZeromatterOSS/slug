//! Authoritative source/generated providers and their alias/dependency projections.

use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::ConfiguredTargetValue;

use super::*;

struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let path = scratch();
        fs::write(path.join("MODULE.bazel"), "module(name = 'root')\n").unwrap();
        fs::write(path.join("source.txt"), "source\n").unwrap();
        fs::write(
            path.join("defs.bzl"),
            DEFINITIONS.replace("VALIDATION_NAME", "validation_a"),
        )
        .unwrap();
        fs::write(path.join("BUILD.bazel"), BUILD).unwrap();
        Self(path)
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

const DEFINITIONS: &str = r#"
Capture = provider(fields = ["target", "file"])
def _capture(ctx):
    return [Capture(target = ctx.attr.dep, file = ctx.attr.single[DefaultInfo].files.to_list()[0])]
capture = rule(implementation = _capture, attrs = {
    "dep": attr.label(allow_files = True),
    "single": attr.label(allow_single_file = True),
})
def _empty(ctx): return []
suffix = rule(implementation = _empty, attrs = {"dep": attr.label(allow_files = [".rs"])})
skip_suffix = rule(implementation = _empty, attrs = {"dep": attr.label(allow_files = [".rs"], flags = ["SKIP_ANALYSIS_TIME_FILETYPE_CHECK"])})
def _setting(ctx): return []
setting = rule(implementation = _setting, build_setting = config.string(flag = True))
def _transition(settings, attr):
    return {"//:mode": "changed"}
change = transition(implementation = _transition, inputs = ["//:mode"], outputs = ["//:mode"])
def _generator(ctx):
    ctx.actions.write(ctx.outputs.out, "generated")
    spare = ctx.actions.declare_file("spare")
    ctx.actions.write(spare, "spare")
    validation = ctx.actions.declare_file("VALIDATION_NAME")
    ctx.actions.write(validation, "validation")
    return [DefaultInfo(files = depset([spare])), OutputGroupInfo(
        _validation = depset([validation]) if ctx.attr.validate else depset(),
        unrelated = depset([spare]), default = depset([spare]))]
generator = rule(implementation = _generator, cfg = change, attrs = {
    "out": attr.output(mandatory = True), "validate": attr.bool(default = True),
})
"#;

const BUILD: &str = r#"
load(":defs.bzl", "capture", "suffix", "skip_suffix", "generator", "setting")
exports_files(["source.txt"])
alias(name = "source_inner", actual = ":source.txt")
alias(name = "source_alias.rs", actual = ":source_inner")
capture(name = "source_consumer", dep = ":source_alias.rs", single = ":source_alias.rs")
capture(name = "direct_source_consumer", dep = ":source.txt", single = ":source.txt")
suffix(name = "source_bad", dep = ":source_alias.rs")
skip_suffix(name = "source_skip_bad", dep = ":source_alias.rs")
platform(name = "source_not_constraint", constraint_values = [":source_alias.rs"])
setting(name = "mode", build_setting_default = "original")
generator(name = "producer", out = "generated.txt")
alias(name = "generated_alias", actual = ":generated.txt")
capture(name = "generated_consumer", dep = ":generated_alias", single = ":generated_alias")
"#;

fn key(name: &str) -> ConfiguredTargetKey {
    ConfiguredTargetKey::new(
        CanonicalLabel::parse(&format!("@@//:{name}")).unwrap(),
        typed_action_test_configuration(),
    )
}

fn singleton(result: &ConfiguredNodeResult) -> AnalysisArtifact {
    let files = result.providers().default_info().unwrap().file_artifacts();
    let [artifact] = files.as_slice() else {
        panic!("expected singleton File: {files:?}")
    };
    artifact.clone()
}

fn captured_target(result: &ConfiguredNodeResult) -> &ConfiguredTargetValue {
    let provider = result
        .providers()
        .user(&ProviderId::new("//:defs.bzl", "Capture").unwrap())
        .unwrap();
    match provider.field("target").unwrap().kind() {
        AnalysisValueKind::ConfiguredTarget(target) => target,
        other => panic!("expected configured target, got {other:?}"),
    }
}

#[tokio::test]
async fn source_and_alias_publish_canonical_singleton_providers() {
    let workspace = Workspace::new();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let source = analyze_request(&dice, &workspace.0, &key("source.txt"), None, false)
        .await
        .unwrap();
    let label = CanonicalLabel::parse("@@//:source.txt").unwrap();
    assert_eq!(source.kind(), &ConfiguredNodeKind::SourceFile);
    assert_eq!(
        source.actual_target(),
        &ConfiguredNodeKey::null(label.clone())
    );
    assert_eq!(singleton(&source), AnalysisArtifact::Source(label.clone()));
    assert!(source.providers().output_group_info().is_none());
    let alias = analyze_request(&dice, &workspace.0, &key("source_alias.rs"), None, false)
        .await
        .unwrap();
    assert_eq!(alias.kind(), &ConfiguredNodeKind::Alias);
    assert_eq!(alias.actual_target(), source.actual_target());
    assert!(alias.actual_configured_target().is_none());
    assert_eq!(alias.providers(), source.providers());
    for consumer_name in ["source_consumer", "direct_source_consumer"] {
        let consumer = analyze_request(&dice, &workspace.0, &key(consumer_name), None, false)
            .await
            .unwrap();
        let target = captured_target(&consumer);
        assert_eq!(target.identity().label(), &label);
        assert!(target.identity().configured().is_none());
        assert_eq!(target.providers(), source.providers());
        let capture = consumer
            .providers()
            .user(&ProviderId::new("//:defs.bzl", "Capture").unwrap())
            .unwrap();
        assert!(
            matches!(capture.field("file").unwrap().kind(), AnalysisValueKind::Artifact(artifact) if artifact == &singleton(&source))
        );
    }
}

#[tokio::test]
async fn source_alias_file_policy_uses_actual_suffix_and_never_skips_source_checks() {
    let workspace = Workspace::new();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    for target in ["source_bad", "source_skip_bad"] {
        let error = analyze_request(&dice, &workspace.0, &key(target), None, false)
            .await
            .unwrap_err();
        assert!(
            error.contains("source.txt") && error.contains("admitted file types"),
            "{target}: {error}"
        );
    }
    let error = analyze_request(
        &dice,
        &workspace.0,
        &key("source_not_constraint"),
        None,
        false,
    )
    .await
    .unwrap_err();
    assert!(
        error.contains("references a non-constraint value"),
        "{error}"
    );
}

#[tokio::test]
async fn generated_providers_keep_transitioned_owner_validation_and_alias_a_b_a() {
    let workspace = Workspace::new();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut snapshots = Vec::new();
    for validation in ["validation_a", "validation_b", "validation_a"] {
        fs::write(
            workspace.0.join("defs.bzl"),
            DEFINITIONS.replace("VALIDATION_NAME", validation),
        )
        .unwrap();
        let generated = analyze_request(&dice, &workspace.0, &key("generated.txt"), None, false)
            .await
            .unwrap();
        let producer = analyze_request(&dice, &workspace.0, &key("producer"), None, false)
            .await
            .unwrap();
        assert_ne!(
            producer.configured_target_key().unwrap().configuration(),
            key("producer").configuration()
        );
        let artifact = singleton(&generated);
        let AnalysisArtifact::Derived { owner, output } = &artifact else {
            panic!("generated artifact lost its owner")
        };
        let AnalysisArtifact::Derived {
            owner: producer_owner,
            ..
        } = singleton(&producer)
        else {
            panic!("producer default output lost owner")
        };
        assert_eq!(owner, &producer_owner);
        assert_eq!(output.path(), "generated.txt");
        assert_eq!(output.kind(), ActionOutputKind::File);
        let groups = generated.providers().output_group_info().unwrap();
        assert_eq!(
            groups
                .groups()
                .keys()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            ["_validation"]
        );
        assert_eq!(
            groups.groups().get("_validation").unwrap().to_list(),
            producer
                .providers()
                .output_group_info()
                .unwrap()
                .groups()
                .get("_validation")
                .unwrap()
                .to_list()
        );
        let alias = analyze_request(&dice, &workspace.0, &key("generated_alias"), None, false)
            .await
            .unwrap();
        assert_eq!(alias.providers(), generated.providers());
        assert_eq!(alias.actual_target(), generated.actual_target());
        let consumer =
            analyze_request(&dice, &workspace.0, &key("generated_consumer"), None, false)
                .await
                .unwrap();
        assert_eq!(
            captured_target(&consumer).providers(),
            generated.providers()
        );
        snapshots.push(generated);
    }
    assert_ne!(snapshots[0], snapshots[1]);
    assert_eq!(snapshots[0], snapshots[2]);
    fs::write(
        workspace.0.join("BUILD.bazel"),
        BUILD.replace(
            "out = \"generated.txt\"",
            "out = \"generated.txt\", validate = False",
        ),
    )
    .unwrap();
    let empty = analyze_request(&dice, &workspace.0, &key("generated.txt"), None, false)
        .await
        .unwrap();
    assert!(empty.providers().output_group_info().is_none());
    assert_eq!(singleton(&empty), singleton(&snapshots[0]));
}
