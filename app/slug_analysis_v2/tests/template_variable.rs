//! Pinned TemplateVariableInfo publication, including cross-provider aliases.
use super::*;

const DEFS: &str = r#"
Proof = provider(fields = ["nested", "loaded"])
Answers = provider(fields = ["value"])
Loaded = platform_common.TemplateVariableInfo({"G": "loaded"})

def _producer(ctx):
    variables = {"first": ctx.attr.value, "second": "stable"}
    if ctx.attr.reverse:
        variables = {"second": "stable", "first": ctx.attr.value}
    info = platform_common.TemplateVariableInfo(variables)
    sibling = platform_common.TemplateVariableInfo(vars = variables)
    nested = platform_common.TemplateVariableInfo(variables) if ctx.attr.split else info
    variables["first"] = "mutated source"
    if info.variables["first"] != ctx.attr.value:
        fail("constructor did not snapshot its input")
    extracted = info.variables
    extracted["first"] = "mutated copy"
    if info.variables["first"] != ctx.attr.value:
        fail("field read leaked mutable state")
    if info == sibling or info != info:
        fail("native equality is not occurrence identity")
    lookup = {info: "original", sibling: "sibling"}
    lookup[info] = "alias"
    if len(lookup) != 2 or lookup[info] != "alias":
        fail("native occurrence hash changed")
    return [info, Proof(loaded = Loaded, nested = struct(
        info = nested, values = [nested, sibling], keys = {nested: "nested", sibling: "sibling"},
        set = depset([nested, nested, sibling])))]

producer = rule(implementation = _producer, attrs = {
    "value": attr.string(default = "A"), "reverse": attr.bool(), "split": attr.bool(),
})

def _forward(ctx):
    return [ctx.attr.dep[platform_common.TemplateVariableInfo], ctx.attr.dep[Proof]]
forward = rule(implementation = _forward, attrs = {
    "dep": attr.label(providers = [platform_common.TemplateVariableInfo, Proof]),
})

def _consumer(ctx):
    dep = ctx.attr.dep
    if platform_common.TemplateVariableInfo not in dep:
        fail("native constructor did not remain a provider key")
    info = dep[platform_common.TemplateVariableInfo]
    proof = dep[Proof]
    nested = proof.nested
    if nested.info != nested.values[0] or nested.info != nested.keys.keys()[0] or nested.info != nested.set.to_list()[0]:
        fail("nested occurrence aliases changed during publication")
    if len(nested.set.to_list()) != 2 or nested.values[0] == nested.values[1]:
        fail("distinct equal-map providers were collapsed")
    if proof.loaded.variables != {"G": "loaded"}:
        fail("loading-time frozen provider lost its payload")
    variables = info.variables
    answer = "|".join([variables["first"], ",".join(variables.keys()), str(info == nested.info)])
    variables.clear()
    if not info.variables:
        fail("frozen provider returned shared mutable dict")
    return [Answers(value = answer)]
consumer = rule(implementation = _consumer, attrs = {"dep": attr.label()})
"#;

const BUILD: &str = r#"
load(":defs.bzl", "producer", "forward", "consumer")
producer(name = "producer", value = "A", reverse = False, split = False)
forward(name = "forward", dep = ":producer")
consumer(name = "consumer", dep = ":forward")
"#;

fn workspace() -> PathBuf {
    let root = scratch();
    fs::write(root.join("MODULE.bazel"), "module(name='root')\n").unwrap();
    fs::write(root.join("defs.bzl"), DEFS).unwrap();
    fs::write(root.join("BUILD.bazel"), BUILD).unwrap();
    root
}

async fn request(
    dice: &Arc<Dice>,
    root: &PathBuf,
    name: &str,
) -> Result<ConfiguredNodeResult, String> {
    root_target_request_with_configuration(
        dice,
        root,
        &format!("@@//:{name}"),
        typed_action_test_configuration(),
        Arc::new(RootActivationTracker::default()),
    )
    .await
    .map(|result| result.as_ref().clone())
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

#[tokio::test]
async fn template_provider_forwards_native_and_nested_occurrences() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let producer = request(&dice, &root, "producer").await.unwrap();
    let forwarded = request(&dice, &root, "forward").await.unwrap();
    let identity = slug_build_api_v2::ProviderIdentity::builtin("TemplateVariableInfo");
    assert!(producer.providers().contains(&identity));
    assert!(forwarded.providers().contains(&identity));
    assert_eq!(
        answer(&request(&dice, &root, "consumer").await.unwrap()),
        "A|first,second|True"
    );
    assert_eq!(forwarded, request(&dice, &root, "forward").await.unwrap());
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn template_payload_order_and_cross_provider_aliases_invalidate_and_restore() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let first = request(&dice, &root, "forward").await.unwrap();
    assert_eq!(
        answer(&request(&dice, &root, "consumer").await.unwrap()),
        "A|first,second|True"
    );
    for (changed, expected) in [
        (
            BUILD.replace("value = \"A\"", "value = \"B\""),
            "B|first,second|True",
        ),
        (
            BUILD.replace("reverse = False", "reverse = True"),
            "A|second,first|True",
        ),
        (
            BUILD.replace("split = False", "split = True"),
            "A|first,second|False",
        ),
    ] {
        fs::write(root.join("BUILD.bazel"), changed).unwrap();
        let next = request(&dice, &root, "forward").await.unwrap();
        assert_ne!(
            first, next,
            "payload/order/alias change must invalidate publication"
        );
        assert_eq!(
            answer(&request(&dice, &root, "consumer").await.unwrap()),
            expected
        );
        fs::write(root.join("BUILD.bazel"), BUILD).unwrap();
        assert_eq!(first, request(&dice, &root, "forward").await.unwrap());
        assert_eq!(
            answer(&request(&dice, &root, "consumer").await.unwrap()),
            "A|first,second|True"
        );
    }
    // Tracked module loss and recreation must not serve a stale native provider.
    fs::remove_file(root.join("defs.bzl")).unwrap();
    let error = request(&dice, &root, "consumer").await.unwrap_err();
    assert_eq!(error, "root request returned Needs");
    fs::write(root.join("defs.bzl"), DEFS).unwrap();
    assert_eq!(first, request(&dice, &root, "forward").await.unwrap());
    assert_eq!(
        answer(&request(&dice, &root, "consumer").await.unwrap()),
        "A|first,second|True"
    );
    fs::remove_dir_all(root).unwrap();
}
