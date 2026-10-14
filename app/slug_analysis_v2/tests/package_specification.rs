//! Native allowlist providers, with hidden membership retained across publication.
use slug_build_api_v2::AnalysisValue;

use super::*;

mod args_capture;

const DEFS: &str = r#"
Proof = provider(fields = ["info", "nested", "external", "mapped"])
Answers = provider(fields = ["value"])

def _producer(ctx):
    info = ctx.attr.group[PackageSpecificationInfo]
    other = ctx.attr.other[PackageSpecificationInfo]
    external = ctx.attr.external[PackageSpecificationInfo]
    mapped = ctx.attr.mapped.label
    if type(info) != "PackageSpecificationInfo" or repr(info) != "struct()" or dir(info) != ["contains"]:
        fail("native provider reflection changed")
    lookup = {info: 1}
    lookup[other] = 2
    if info != other or len(lookup) != 1 or lookup[info] != 2:
        fail("native method-only equality/hash changed")
    if not info.contains("//allowed:target") or other.contains("//allowed:target"):
        fail("equal providers lost distinct hidden membership")
    if not info.contains("//allowed/blocked:target") or info.contains("//allowed/local_only:target"):
        fail("negative specification escaped its owning group")
    if info.contains("//allowed_sibling:target"):
        fail("subtree path segment boundary changed")
    if not external.contains(mapped) or external.contains("@alias//allowed:target"):
        fail("canonical string incorrectly used apparent mapping")
    if not external.contains("@dep+//allowed:target") or not external.contains("@@dep+//allowed:target"):
        fail("canonical external spelling changed")
    if external.contains("//allowed:target") or info.contains(mapped):
        fail("repository membership leaked")
    if ctx.attr.failure == "relative": info.contains(":relative")
    elif ctx.attr.failure == "bare": info.contains("relative")
    elif ctx.attr.failure == "type": info.contains([])
    elif ctx.attr.failure == "keyword": info.contains(target = "//allowed:target")
    elif ctx.attr.failure == "missing": info.contains()
    elif ctx.attr.failure == "extra": info.contains("//allowed:target", "//allowed:other")
    elif ctx.attr.failure == "construct": PackageSpecificationInfo()
    return [info, Proof(info = info, external = external, mapped = mapped,
        nested = struct(values = [info], keys = {info: "value"}, set = depset([info, other])))]

producer = rule(implementation = _producer, attrs = {
    "group": attr.label(default = "//:group"), "other": attr.label(default = "//:empty"),
    "external": attr.label(mandatory = True), "mapped": attr.label(mandatory = True), "failure": attr.string(),
})

def _forward(ctx):
    return [ctx.attr.dep[PackageSpecificationInfo], ctx.attr.dep[Proof]]
forward = rule(implementation = _forward, attrs = {"dep": attr.label()})

def _consumer(ctx):
    proof = ctx.attr.dep[Proof]
    infos = [ctx.attr.dep[PackageSpecificationInfo], proof.info, proof.nested.values[0],
        proof.nested.keys.keys()[0], proof.nested.set.to_list()[0]]
    if len(proof.nested.set.to_list()) != 1:
        fail("native equality no longer deduplicates depset")
    answers = [str(info.contains("//switch:target")) for info in infos]
    answers.append(str(proof.external.contains(proof.mapped)))
    return [Answers(value = "|".join(answers))]
consumer = rule(implementation = _consumer, attrs = {"dep": attr.label()})

def _check(ctx):
    info = ctx.attr.dep[PackageSpecificationInfo]
    return [Answers(value = str(info.contains(ctx.attr.target)))]
check = rule(implementation = _check, attrs = {"dep": attr.label(), "target": attr.string(default = "@@foreign+//any:target")})
"#;

const BUILD: &str = r#"
load(":defs.bzl", "producer", "forward", "consumer", "check")
package_group(name = "group", packages = ["//allowed/...", "-//allowed/blocked", "-//allowed/local_only"], includes = ["//groups:left", "//groups:right"])
package_group(name = "empty", packages = ["private"])
package_group(name = "public", packages = ["public"])
producer(name = "producer", external = "@alias//groups:group", mapped = "@alias//allowed:target")
forward(name = "forward", dep = ":producer")
consumer(name = "consumer", dep = ":forward")
check(name = "public_check", dep = ":public")
check(name = "empty_check", dep = ":empty")
exports_files(["plain.txt"])
package_group(name = "wrong", includes = [":plain.txt"])
package_group(name = "cross", includes = ["@alias//groups:group"])
check(name = "cross_check", dep = ":cross", target = "@@dep+//allowed:target")
package_group(name = "missing", includes = ["//missing:group"])
package_group(name = "cycle_a", includes = [":cycle_b"])
package_group(name = "cycle_b", includes = [":cycle_a"])
check(name = "wrong_check", dep = ":wrong")
check(name = "missing_check", dep = ":missing")
check(name = "cycle_check", dep = ":cycle_a")
"#;

const GROUPS: &str = "package_group(name='leaf',packages=['//allowed/blocked','//switch'])\npackage_group(name='left',includes=[':leaf'])\npackage_group(name='right',includes=[':leaf'])\n";
const EXTERNAL: &str = "package_group(name='group',packages=['//allowed/...'])\n";

fn workspace() -> PathBuf {
    let root = scratch();
    fs::create_dir_all(root.join("groups")).unwrap();
    fs::create_dir_all(root.join("dep/groups")).unwrap();
    fs::create_dir_all(root.join("dep/allowed")).unwrap();
    fs::write(root.join("MODULE.bazel"), "module(name='root')\nbazel_dep(name='dep',version='1.0.0',repo_name='alias')\nlocal_path_override(module_name='dep',path='dep')\n").unwrap();
    fs::write(
        root.join("dep/MODULE.bazel"),
        "module(name='dep',version='1.0.0')\n",
    )
    .unwrap();
    fs::write(root.join("dep/REPO.bazel"), "").unwrap();
    fs::write(root.join("dep/.bazelignore"), "").unwrap();
    fs::write(root.join("dep/groups/BUILD.bazel"), EXTERNAL).unwrap();
    fs::write(
        root.join("dep/allowed/BUILD.bazel"),
        "package_group(name='target')\n",
    )
    .unwrap();
    fs::write(root.join("groups/BUILD.bazel"), GROUPS).unwrap();
    fs::write(root.join("defs.bzl"), DEFS).unwrap();
    fs::write(root.join("BUILD.bazel"), BUILD).unwrap();
    fs::write(root.join("plain.txt"), "source\n").unwrap();
    root
}

async fn request(
    dice: &Arc<Dice>,
    root: &PathBuf,
    name: &str,
) -> Result<ConfiguredNodeResult, String> {
    root_target_request_with_inputs(
        dice,
        root,
        &format!("@@//:{name}"),
        typed_action_test_configuration(),
        Arc::new(RootActivationTracker::default()),
        root_epoch(root),
        &[("dep+", "dep")],
    )
    .await
    .map(|value| value.as_ref().clone())
}

fn answer(result: &ConfiguredNodeResult) -> &str {
    result
        .providers()
        .user(&ProviderId::new("//:defs.bzl", "Answers").unwrap())
        .unwrap()
        .field("value")
        .unwrap()
        .as_str()
        .unwrap()
}

fn nested(result: &ConfiguredNodeResult) -> &AnalysisValue {
    result
        .providers()
        .user(&ProviderId::new("//:defs.bzl", "Proof").unwrap())
        .unwrap()
        .field("nested")
        .unwrap()
}

#[tokio::test]
async fn native_package_groups_publish_membership_and_constant_starlark_equality() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let producer = request(&dice, &root, "producer").await.unwrap();
    let forwarded = request(&dice, &root, "forward").await.unwrap();
    assert!(nested(&producer).publication_eq(nested(&forwarded)));
    assert!(
        forwarded
            .providers()
            .contains(&slug_build_api_v2::ProviderIdentity::builtin(
                "PackageSpecificationInfo"
            ))
    );
    assert_eq!(
        answer(&request(&dice, &root, "consumer").await.unwrap()),
        "True|True|True|True|True|True"
    );
    assert_eq!(
        answer(&request(&dice, &root, "public_check").await.unwrap()),
        "True"
    );
    assert_eq!(
        answer(&request(&dice, &root, "empty_check").await.unwrap()),
        "False"
    );
    assert_eq!(
        answer(&request(&dice, &root, "cross_check").await.unwrap()),
        "True"
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn package_group_hidden_membership_invalidates_nested_publication_and_restores() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let first = request(&dice, &root, "forward").await.unwrap();
    assert_eq!(
        answer(&request(&dice, &root, "consumer").await.unwrap()),
        "True|True|True|True|True|True"
    );
    let groups = root.join("groups/BUILD.bazel");
    fs::write(&groups, GROUPS.replace("'//switch'", "'//changed'")).unwrap();
    let changed = request(&dice, &root, "forward").await.unwrap();
    let info = |result: &ConfiguredNodeResult| {
        result
            .providers()
            .user(&ProviderId::new("//:defs.bzl", "Proof").unwrap())
            .unwrap()
            .field("info")
            .unwrap()
            .clone()
    };
    assert_eq!(
        info(&first),
        info(&changed),
        "Starlark equality ignores membership"
    );
    assert!(
        !nested(&first).publication_eq(nested(&changed)),
        "publication must retain hidden membership"
    );
    assert_ne!(first, changed);
    assert_eq!(
        answer(&request(&dice, &root, "consumer").await.unwrap()),
        "False|False|False|False|False|True"
    );
    fs::write(&groups, GROUPS).unwrap();
    assert_eq!(first, request(&dice, &root, "forward").await.unwrap());
    fs::remove_file(&groups).unwrap();
    let error = request(&dice, &root, "consumer").await.unwrap_err();
    assert!(error.contains("groups"), "{error}");
    fs::write(&groups, GROUPS).unwrap();
    assert_eq!(first, request(&dice, &root, "forward").await.unwrap());
    assert_eq!(
        answer(&request(&dice, &root, "consumer").await.unwrap()),
        "True|True|True|True|True|True"
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn external_package_group_contents_restore_without_starlark_equality_cutoff() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let first = request(&dice, &root, "forward").await.unwrap();
    let external = root.join("dep/groups/BUILD.bazel");
    fs::write(
        &external,
        EXTERNAL.replace("'//allowed/...'", "'//allowed/...','//extra'"),
    )
    .unwrap();
    let changed = request(&dice, &root, "forward").await.unwrap();
    assert_ne!(first, changed);
    let value = |result: &ConfiguredNodeResult| {
        result
            .providers()
            .user(&ProviderId::new("//:defs.bzl", "Proof").unwrap())
            .unwrap()
            .field("external")
            .unwrap()
            .clone()
    };
    assert_eq!(value(&first), value(&changed));
    assert!(!value(&first).publication_eq(&value(&changed)));
    fs::write(&external, EXTERNAL).unwrap();
    assert_eq!(first, request(&dice, &root, "forward").await.unwrap());
    fs::remove_file(&external).unwrap();
    let error = request(&dice, &root, "consumer").await.unwrap_err();
    assert!(error.contains("groups"), "{error}");
    fs::write(&external, EXTERNAL).unwrap();
    assert_eq!(first, request(&dice, &root, "forward").await.unwrap());
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn package_group_include_and_native_binding_errors_fail_closed() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    for (name, expected) in [
        ("wrong_check", "package group"),
        ("missing_check", "missing"),
        ("cycle_check", "cycle"),
    ] {
        let error = request(&dice, &root, name).await.unwrap_err();
        assert!(
            error.to_ascii_lowercase().contains(expected),
            "{name}: {error}"
        );
    }
    for mode in [
        "relative",
        "bare",
        "type",
        "keyword",
        "missing",
        "extra",
        "construct",
    ] {
        fs::write(
            root.join("BUILD.bazel"),
            BUILD.replace(
                "producer(name = \"producer\",",
                &format!("producer(name = \"producer\", failure = \"{mode}\","),
            ),
        )
        .unwrap();
        let error = request(&dice, &root, "consumer").await.unwrap_err();
        assert!(
            error.contains("contains") || error.contains("PackageSpecificationInfo"),
            "{mode}: {error}"
        );
    }
    fs::write(root.join("BUILD.bazel"), BUILD).unwrap();
    assert_eq!(
        answer(&request(&dice, &root, "consumer").await.unwrap()),
        "True|True|True|True|True|True"
    );
    fs::remove_dir_all(root).unwrap();
}
