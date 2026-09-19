//! Pinned OutputGroupInfo semantics through the ordinary configured rule boundary.

use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::ProviderIdentity;

use super::*;

struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let path = scratch();
        fs::write(path.join("MODULE.bazel"), "module(name = \"root\")\n").unwrap();
        Self(path)
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

const DEFINITIONS: &str = r#"
load(":builtin.bzl", "Groups")
OutputGroupInfo = provider(fields = ["value"])
Payload = provider(fields = ["groups", "shared"])
FROZEN = Groups(empty = depset(order = "preorder"))

def _check(groups, shared):
    if groups.default != shared or groups["default"] != shared or groups.same != shared:
        fail("group depset identity changed")
    if "default" not in groups or "missing" in groups or 7 in groups:
        fail("group membership changed")
    names = [name for name in groups]
    if names != dir(groups):
        fail("group iteration differs from fields")
    if "empty" not in groups or groups.empty.to_list() != [] or hasattr(groups, "missing"):
        fail("empty group differs from missing")
    copy = Groups(**{name: groups[name] for name in groups})
    if copy != groups or groups != copy or {groups: "ok"}[copy] != "ok":
        fail("provider equality/hash round trip changed")
    if Groups(empty = depset(order = "postorder")) != FROZEN:
        fail("all-empty normalization changed")

def _leaf(ctx):
    alternatives = [ctx.actions.declare_file("produced_0"), ctx.actions.declare_file("produced_1")]
    for alternative in alternatives:
        ctx.actions.write(alternative, "value")
    out = alternatives[VARIANT_INDEX]
    tree = ctx.actions.declare_directory("tree")
    ctx.actions.run_shell(outputs = [tree], command = "mkdir -p tree")
    shared = depset([out], transitive = [depset([tree])])
    groups = Groups(default = shared, same = shared, seq = [out], tup = (tree,), empty = depset(order = "preorder"), _validation = depset([]))
    _check(groups, shared)
    if groups.seq.to_list() != [out] or groups.tup.to_list() != [tree]:
        fail("sequence was not normalized to depset")
    independent = Groups(default = depset([out, tree]))
    if independent.default == shared:
        fail("independent depsets lost occurrence identity")
    return [DefaultInfo(files = shared), groups, Payload(groups = groups, shared = shared), OutputGroupInfo(value = "user")]

def _forward(ctx):
    dependency = ctx.attr.dep
    groups = dependency[Groups]
    _check(groups, dependency[Payload].shared)
    nested = dependency[Payload]
    _check(nested.groups, nested.shared)
    if groups != nested.groups or {groups: "ok"}[nested.groups] != "ok":
        fail("nested group retained a different builtin identity")
    if OutputGroupInfo in dependency and dependency[OutputGroupInfo].value != "user":
        fail("user provider was confused with builtin OutputGroupInfo")
    return [groups, Payload(groups = nested.groups, shared = nested.shared)]

leaf = rule(implementation = _leaf)
forward = rule(implementation = _forward, attrs = {"dep": attr.label(mandatory = True)})
"#;

#[tokio::test]
async fn configured_output_groups_preserve_artifacts_sharing_forwarding_and_a_b_a() {
    let workspace = Workspace::new();
    fs::write(
        workspace.0.join("builtin.bzl"),
        "Groups = OutputGroupInfo\n",
    )
    .unwrap();
    fs::write(workspace.0.join("BUILD.bazel"), "load(\":defs.bzl\", \"leaf\", \"forward\")\nleaf(name = \"leaf\")\nforward(name = \"middle\", dep = \":leaf\")\nforward(name = \"top\", dep = \":middle\")\n").unwrap();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let key = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//:top").unwrap(),
        typed_action_test_configuration(),
    );
    let mut results = Vec::new();
    for index in [0, 1, 0] {
        fs::write(
            workspace.0.join("defs.bzl"),
            DEFINITIONS.replace("VARIANT_INDEX", &index.to_string()),
        )
        .unwrap();
        let result = analyze_request(&dice, &workspace.0, &key, None, false)
            .await
            .unwrap();
        let groups = result
            .providers()
            .output_group_info()
            .expect("forwarded builtin stays typed");
        let values = groups.groups().get("default").unwrap().to_list();
        let artifacts = values
            .iter()
            .map(|value| match value.kind() {
                AnalysisValueKind::Artifact(artifact) => artifact,
                _ => panic!("output group contains a non-artifact"),
            })
            .collect::<Vec<_>>();
        let mut derived = artifacts
            .iter()
            .filter_map(|artifact| match artifact {
                AnalysisArtifact::Derived { owner, output } => Some((owner, output)),
                _ => None,
            })
            .collect::<Vec<_>>();
        derived.sort_by_key(|(_, output)| output.path());
        assert_eq!(derived.len(), 2);
        assert!(
            derived
                .iter()
                .all(|(owner, _)| owner.label() == &CanonicalLabel::parse("@@//:leaf").unwrap())
        );
        assert_eq!(derived[0].1.kind(), ActionOutputKind::File);
        assert_eq!(derived[0].1.path(), format!("produced_{index}"));
        assert_eq!(derived[1].1.kind(), ActionOutputKind::Directory);
        assert_eq!(
            groups.groups().get("default").unwrap(),
            groups.groups().get("same").unwrap()
        );
        results.push(result);
    }
    assert_ne!(results[0], results[1]);
    assert_eq!(results[0], results[2]);
}

#[tokio::test]
async fn configured_output_groups_reject_duplicate_builtin_and_private_override_returns() {
    let workspace = Workspace::new();
    fs::write(
        workspace.0.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"sample\")\nsample(name = \"sample\")\n",
    )
    .unwrap();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let key = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//:sample").unwrap(),
        test_configuration(),
    );
    for (expression, expected) in [
        (
            "[OutputGroupInfo(x=[]), OutputGroupInfo(y=[])]",
            "specified twice",
        ),
        (
            "[OutputGroupInfo(_validation_transitive=[])]",
            "configured _validation_transitive override is unsupported",
        ),
    ] {
        fs::write(
            workspace.0.join("defs.bzl"),
            format!(
                "def _impl(ctx):\n    return {expression}\nsample = rule(implementation = _impl)\n"
            ),
        )
        .unwrap();
        let error = analyze_request(&dice, &workspace.0, &key, None, false)
            .await
            .unwrap_err();
        assert!(error.contains(expected), "{error}");
    }
    fs::write(workspace.0.join("defs.bzl"),"def _impl(ctx):\n    return [OutputGroupInfo(_validation=[], **{\"_hidden_top_level_INTERNAL_\": (), \"arbitrary-name\": []})]\nsample = rule(implementation = _impl)\n").unwrap();
    let restored = analyze_request(&dice, &workspace.0, &key, None, false)
        .await
        .unwrap();
    assert_eq!(
        restored
            .providers()
            .output_group_info()
            .unwrap()
            .groups()
            .len(),
        3
    );
    assert!(
        restored
            .providers()
            .get(&ProviderIdentity::builtin("OutputGroupInfo"))
            .is_some()
    );
}
