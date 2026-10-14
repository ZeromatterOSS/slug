//! One original-value bijection across typed direct leaves and outward fields.
use slug_build_api_v2::AnalysisValueKind;

use super::*;

#[tokio::test]
async fn typed_direct_functions_and_providers_keep_joint_original_aliases() {
    let root = scratch();
    fs::write(root.join("MODULE.bazel"), "module(name='root')\n").unwrap();
    fs::write(
        root.join("defs.bzl"),
        r#"
Leaf = provider(fields=['callback'])
Info = provider(fields=['callback','leaf','fresh','functions','providers'])
def _callback(word): return 'actual:' + word
def _produce(ctx):
    leaf = Leaf(callback=_callback)
    fresh = Leaf(callback=_callback)
    return [Info(callback=_callback, leaf=leaf, fresh=fresh,
                 functions=depset([_callback]), providers=depset([leaf]))]
def _consume(ctx):
    p = ctx.attr.dep[Info]
    function = p.functions.to_list()[0]
    leaf = p.providers.to_list()[0]
    if function('x') != 'actual:x' or leaf.callback('y') != 'actual:y':
        fail('typed leaf lost actual callable execution')
    return [Info(callback=p.callback, leaf=p.leaf, fresh=p.fresh,
                 functions=p.functions, providers=p.providers)]
produce = rule(implementation=_produce)
consume = rule(implementation=_consume, attrs={'dep': attr.label(providers=[Info])})
"#,
    )
    .unwrap();
    fs::write(root.join("BUILD.bazel"), "load(':defs.bzl','produce','consume')\nproduce(name='producer')\nconsume(name='consumer',dep=':producer')\n").unwrap();
    let key = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//:consumer").unwrap(),
        test_configuration(),
    );
    let result = analyze_request(
        &Dice::builder().build(DetectCycles::Enabled),
        &root,
        &key,
        None,
        false,
    )
    .await
    .unwrap();
    let info = result
        .providers()
        .user(&ProviderId::new("//:defs.bzl", "Info").unwrap())
        .unwrap();
    let callback = info.field("callback").unwrap();
    let leaf = info.field("leaf").unwrap();
    let fresh = info.field("fresh").unwrap();
    let AnalysisValueKind::Depset(functions) = info.field("functions").unwrap().kind() else {
        panic!("typed function depset")
    };
    let AnalysisValueKind::Depset(providers) = info.field("providers").unwrap().kind() else {
        panic!("typed provider depset")
    };
    let functions = functions.to_list();
    let providers = providers.to_list();
    assert_eq!(functions.len(), 1);
    assert_eq!(providers.len(), 1);
    assert!(
        functions[0]
            .original_origin()
            .unwrap()
            .same_original(callback.original_origin().unwrap())
    );
    assert!(
        providers[0]
            .original_origin()
            .unwrap()
            .same_original(leaf.original_origin().unwrap())
    );
    assert!(
        !fresh
            .original_origin()
            .unwrap()
            .same_original(leaf.original_origin().unwrap())
    );
    let AnalysisValueKind::Provider(leaf) = leaf.kind() else {
        panic!("leaf provider")
    };
    assert!(
        leaf.field("callback")
            .unwrap()
            .original_origin()
            .unwrap()
            .same_original(callback.original_origin().unwrap())
    );
}
