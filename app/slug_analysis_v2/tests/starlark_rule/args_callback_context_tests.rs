//! Real configured actions publish and later execute unrelated original callbacks.
use super::*;

const DEFS: &str = r#"
Proof = provider(fields=['shared'])
def _decorate(value):
    return value.upper()
def _global(value):
    if value == 'skip': return None
    return (_decorate(value), _decorate(value))
def _impl(ctx):
    output = ctx.actions.declare_file('out')
    args = ctx.actions.args()
    shared = [ctx.attr.prefix]
    def nested(value):
        return shared[0] + ':' + value
    args.add_all(['one','skip','two'], map_each=_global, uniquify=True, format_each='<%s>')
    args.add_joined(['a','b'], map_each=nested, allow_closure=True, join_with='|')
    args.add_all(['z'], map_each=lambda value: shared[0] + value, allow_closure=True)
    ctx.actions.run_shell(outputs=[output], command='true', arguments=[args])
    shared[0] = ctx.attr.prefix + '-late'
    return [DefaultInfo(files=depset([output])), Proof(shared=shared)]
sample = rule(implementation=_impl, attrs={'prefix':attr.string(default='A')})
def _failure(value):
    fail('actual deferred failure')
def _bad(ctx):
    output = ctx.actions.declare_file('bad')
    args = ctx.actions.args()
    args.add_all(['one'], map_each=_failure)
    ctx.actions.run_shell(outputs=[output], command='true', arguments=[args])
    return [DefaultInfo(files=depset([output]))]
bad = rule(implementation=_bad)
def _capability(ctx):
    output = ctx.actions.declare_file('capability')
    args = ctx.actions.args()
    args.add_all(['one'], map_each=lambda value: str(ctx.label), allow_closure=True)
    ctx.actions.run_shell(outputs=[output], command='true', arguments=[args])
    return [DefaultInfo(files=depset([output]))]
capability = rule(implementation=_capability)
"#;

fn workspace() -> PathBuf {
    let root = scratch();
    fs::write(root.join("MODULE.bazel"), "module(name='root')\n").unwrap();
    fs::write(root.join("defs.bzl"), DEFS).unwrap();
    fs::write(root.join("BUILD.bazel"), "load(':defs.bzl','sample','bad','capability')\nsample(name='sample')\nbad(name='bad')\ncapability(name='capability')\n").unwrap();
    root
}
async fn request(
    dice: &Arc<Dice>,
    root: &PathBuf,
    target: &str,
) -> Result<Arc<ConfiguredNodeResult>, String> {
    root_target_request_with_configuration(
        dice,
        root,
        &format!("@@//:{target}"),
        typed_action_test_configuration(),
        Arc::new(RootActivationTracker::default()),
    )
    .await
}
fn argv(
    result: &ConfiguredNodeResult,
) -> Result<Vec<String>, slug_build_api_v2::ArgsExpansionError> {
    result.actions()[0].spawn_spec().unwrap().render_argv()
}

#[tokio::test]
async fn configured_callbacks_execute_original_globals_closures_and_lambdas_after_producer_drop() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let result = request(&dice, &root, "sample").await.unwrap();
    drop(dice);
    fs::remove_dir_all(root).unwrap();
    assert!(argv(&result).unwrap().ends_with(&[
        "<ONE>".into(),
        "<TWO>".into(),
        "A-late:a|A-late:b".into(),
        "A-latez".into()
    ]));
}

#[tokio::test]
async fn configured_callbacks_defer_failure_and_reject_captured_analysis_capabilities() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let result = request(&dice, &root, "bad").await.unwrap();
    let error = argv(&result).unwrap_err().to_string();
    assert!(
        error.contains("actual deferred failure") && error.contains("defs.bzl"),
        "{error}"
    );
    let error = request(&dice, &root, "capability").await.unwrap_err();
    assert!(
        error.contains("unsupported") || error.contains("capability"),
        "{error}"
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn configured_callbacks_source_and_capture_mutations_restore_publication() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let a = request(&dice, &root, "sample").await.unwrap();
    fs::write(
        root.join("defs.bzl"),
        DEFS.replace("default='A'", "default='B'"),
    )
    .unwrap();
    let b = request(&dice, &root, "sample").await.unwrap();
    assert_ne!(a, b);
    assert_ne!(argv(&a).unwrap(), argv(&b).unwrap());
    fs::write(root.join("defs.bzl"), DEFS).unwrap();
    assert_eq!(a, request(&dice, &root, "sample").await.unwrap());
    fs::write(
        root.join("defs.bzl"),
        format!("{DEFS}\n# tracked source change\n"),
    )
    .unwrap();
    let source = request(&dice, &root, "sample").await.unwrap();
    assert_ne!(a, source);
    assert_eq!(argv(&a).unwrap(), argv(&source).unwrap());
    fs::write(root.join("defs.bzl"), DEFS).unwrap();
    assert_eq!(a, request(&dice, &root, "sample").await.unwrap());
    fs::remove_dir_all(root).unwrap();
}
