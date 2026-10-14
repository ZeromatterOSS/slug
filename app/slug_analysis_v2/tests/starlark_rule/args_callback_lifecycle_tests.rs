//! Tracked imported callbacks retain their actual executable and source owners.
use super::*;

const CALLBACKS: &str = r#"
def render(value): return 'kept:' + value
saved = render
render = 0
render = saved
def directory(value, expander):
    return [file.short_path for file in expander.list(value)]
"#;
const RULE: &str = r#"
load(':callbacks.bzl', _mapper='saved', _directory='directory')
def _impl(ctx):
    output = ctx.actions.declare_file('arguments')
    args = ctx.actions.args()
    args.add_all(['one'], map_each=_mapper)
    args.add_all([17], map_each=str)
    args.add_all([ctx.file.src], map_each=_directory)
    args.set_param_file_format('multiline')
    ctx.actions.write(output=output, content=args)
    return [DefaultInfo(files=depset([output]))]
subject = rule(implementation=_impl, attrs={'src':attr.label(allow_single_file=True)})
"#;
const LEAK: &str = r#"
def _impl(ctx):
    output = ctx.actions.declare_file('leak')
    captured = struct(tag='safe', buried=[ctx.actions])
    def mapper(value): return captured.tag + value
    args = ctx.actions.args()
    args.add_all(['x'], map_each=mapper, allow_closure=True)
    ctx.actions.write(output=output, content=args)
    return [DefaultInfo(files=depset([output]))]
leak = rule(implementation=_impl)
"#;
fn leaf_module(selected: &str) -> String {
    format!(
        "module(name='leaf',version='1.0.0')\nbazel_dep(name='{selected}',version='1.0.0',repo_name='alias')\n"
    )
}
fn workspace() -> PathBuf {
    let root = scratch();
    for path in ["leaf", "a", "b"] {
        fs::create_dir_all(root.join(path)).unwrap();
    }
    fs::write(root.join("MODULE.bazel"), "module(name='root')\nbazel_dep(name='leaf',version='1.0.0')\nlocal_path_override(module_name='leaf',path='leaf')\nbazel_dep(name='a',version='1.0.0')\nlocal_path_override(module_name='a',path='a')\nbazel_dep(name='b',version='1.0.0')\nlocal_path_override(module_name='b',path='b')\n").unwrap();
    for name in ["a", "b"] {
        fs::write(
            root.join(name).join("MODULE.bazel"),
            format!("module(name='{name}',version='1.0.0')\n"),
        )
        .unwrap();
    }
    for name in ["leaf", "a", "b"] {
        fs::write(root.join(name).join("REPO.bazel"), "").unwrap();
        fs::write(root.join(name).join(".bazelignore"), "").unwrap();
    }
    fs::write(root.join("leaf/MODULE.bazel"), leaf_module("a")).unwrap();
    fs::write(root.join("leaf/BUILD.bazel"), "load(':defs.bzl','subject')\nload(':leak.bzl','leak')\nsubject(name='subject',src='input.txt')\nleak(name='leak')\n").unwrap();
    for file in ["callbacks.bzl", "relocated_callbacks.bzl"] {
        fs::write(root.join("leaf").join(file), CALLBACKS).unwrap();
    }
    fs::write(root.join("leaf/defs.bzl"), RULE).unwrap();
    fs::write(root.join("leaf/leak.bzl"), LEAK).unwrap();
    fs::write(root.join("leaf/input.txt"), "input").unwrap();
    fs::write(root.join("BUILD.bazel"), "alias(name='subject',actual='@leaf//:subject')\nalias(name='leak',actual='@leaf//:leak')\n").unwrap();
    root
}
async fn request(
    dice: &Arc<Dice>,
    root: &PathBuf,
    target: &str,
) -> Result<Arc<ConfiguredNodeResult>, String> {
    let repositories = [("leaf+", "leaf"), ("a+", "a"), ("b+", "b")];
    let alias = observed_root_target_request_with_inputs(
        dice,
        root,
        &format!("@@//:{target}"),
        typed_action_test_configuration(),
        Arc::new(RootActivationTracker::default()),
        root_epoch(root),
        &repositories,
    )
    .await?;
    // Alias providers forward the actual target; actions remain on its own
    // configured result. Demand that canonical selected-package owner as well.
    observed_root_target_request_with_inputs(
        dice,
        root,
        &alias.actual_target().label().to_string(),
        typed_action_test_configuration(),
        Arc::new(RootActivationTracker::default()),
        root_epoch(root),
        &repositories,
    )
    .await
}
fn content(result: &ConfiguredNodeResult) -> String {
    result.actions()[0]
        .args_write_spec()
        .unwrap()
        .render_content()
        .unwrap()
}

#[tokio::test]
async fn configured_imported_callback_args_write_and_detached_recipe_survive_all_producer_owners() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let result = request(&dice, &root, "subject").await.unwrap();
    assert_eq!(content(&result), "kept:one\n17\n../leaf+/input.txt\n");
    let write = result.actions()[0].args_write_spec().unwrap().clone();
    let recipe = write.recipe().clone();
    drop(result);
    dice.wait_for_idle().await;
    drop(dice);
    fs::remove_dir_all(root).unwrap();
    assert_eq!(
        write.render_content().unwrap(),
        "kept:one\n17\n../leaf+/input.txt\n"
    );
    drop(write);
    assert_eq!(
        recipe.render().unwrap(),
        ["kept:one", "17", "../leaf+/input.txt"]
    );
}

#[tokio::test]
async fn configured_imported_callback_globality_and_buried_capability_are_checked_before_freeze() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let first = request(&dice, &root, "subject").await.unwrap();
    fs::write(
        root.join("leaf/callbacks.bzl"),
        CALLBACKS.replace("render = saved\n", ""),
    )
    .unwrap();
    let error = request(&dice, &root, "subject").await.unwrap_err();
    assert!(error.contains("top-level def"), "{error}");
    fs::write(root.join("leaf/callbacks.bzl"), CALLBACKS).unwrap();
    assert_eq!(request(&dice, &root, "subject").await.unwrap(), first);
    let error = request(&dice, &root, "leak").await.unwrap_err();
    assert!(
        error.contains("unsupported") || error.contains("capability"),
        "{error}"
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn configured_imported_callback_source_relocation_and_repository_mapping_restore_publication()
{
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let first = request(&dice, &root, "subject").await.unwrap();
    fs::write(
        root.join("leaf/callbacks.bzl"),
        format!("{CALLBACKS}\n# complete tracked helper source\n"),
    )
    .unwrap();
    let source = request(&dice, &root, "subject").await.unwrap();
    assert_ne!(first, source);
    assert_eq!(content(&first), content(&source));
    fs::write(root.join("leaf/callbacks.bzl"), CALLBACKS).unwrap();
    assert_eq!(request(&dice, &root, "subject").await.unwrap(), first);
    fs::write(
        root.join("leaf/defs.bzl"),
        RULE.replace(":callbacks.bzl", ":relocated_callbacks.bzl"),
    )
    .unwrap();
    let relocated = request(&dice, &root, "subject").await.unwrap();
    assert_ne!(first, relocated);
    assert_eq!(content(&first), content(&relocated));
    fs::write(root.join("leaf/defs.bzl"), RULE).unwrap();
    assert_eq!(request(&dice, &root, "subject").await.unwrap(), first);
    fs::write(root.join("leaf/MODULE.bazel"), leaf_module("b")).unwrap();
    let mapping = request(&dice, &root, "subject").await.unwrap();
    assert_ne!(first, mapping);
    assert_eq!(content(&first), content(&mapping));
    fs::write(root.join("leaf/MODULE.bazel"), leaf_module("a")).unwrap();
    assert_eq!(request(&dice, &root, "subject").await.unwrap(), first);
    dice.wait_for_idle().await;
    drop(dice);
    fs::remove_dir_all(root).unwrap();
}
