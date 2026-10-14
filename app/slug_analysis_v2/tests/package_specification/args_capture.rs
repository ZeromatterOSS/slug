//! Original callbacks retain the complete native membership receiver.
use super::*;

const CALLBACKS: &str = r#"
def _capture_args(ctx):
    info = ctx.attr.group[PackageSpecificationInfo]
    first = info.contains
    second = info.contains
    def mapper(value):
        return str(info.contains(value)) + ":" + str(first(value)) + ":" + str(second(value))
    args = ctx.actions.args()
    args.add_all(["//switch:target", "//changed:target"],
        map_each = first if ctx.attr.bare else mapper, allow_closure = True)
    out = ctx.actions.declare_file(ctx.label.name + ".out")
    ctx.actions.run_shell(outputs = [out], command = "unused", arguments = [args])
    return [DefaultInfo(files = depset([out]))]
capture_args = rule(implementation = _capture_args, attrs = {
    "group": attr.label(default = ":group"), "bare": attr.bool(),
})
"#;

fn callback_workspace() -> PathBuf {
    let root = workspace();
    fs::write(root.join("defs.bzl"), format!("{DEFS}\n{CALLBACKS}")).unwrap();
    fs::write(root.join("BUILD.bazel"), format!("{BUILD}\nload(':defs.bzl', 'capture_args')\ncapture_args(name='capture')\ncapture_args(name='bare_capture', bare=True)\n")).unwrap();
    root
}

fn argv(result: &ConfiguredNodeResult) -> Vec<String> {
    result.actions()[0]
        .spawn_spec()
        .unwrap()
        .render_argv()
        .unwrap()
}

#[tokio::test]
async fn captured_package_membership_and_bound_methods_restore_full_publication() {
    let root = callback_workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let first = request(&dice, &root, "capture").await.unwrap();
    let original_argv = argv(&first);
    assert!(
        original_argv.ends_with(&["True:True:True".into(), "False:False:False".into()]),
        "{original_argv:?}"
    );
    let groups = root.join("groups/BUILD.bazel");
    fs::write(&groups, GROUPS.replace("'//switch'", "'//changed'")).unwrap();
    let changed = request(&dice, &root, "capture").await.unwrap();
    assert!(
        first != changed,
        "hidden native membership must change the published callback"
    );
    let changed_argv = argv(&changed);
    assert!(
        changed_argv.ends_with(&["False:False:False".into(), "True:True:True".into()]),
        "{changed_argv:?}"
    );
    fs::write(&groups, GROUPS).unwrap();
    let restored = request(&dice, &root, "capture").await.unwrap();
    assert!(
        first == restored,
        "source restoration must preserve the complete original alias graph"
    );
    assert_eq!(argv(&restored), original_argv);
    // Execute again after the producer result and its engine are gone.
    drop(first);
    drop(changed);
    drop(dice);
    assert_eq!(argv(&restored), original_argv);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn bare_native_contains_keeps_the_ordinary_invalid_bool_mapper_error() {
    let root = callback_workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let result = request(&dice, &root, "bare_capture").await.unwrap();
    let error = result.actions()[0]
        .spawn_spec()
        .unwrap()
        .render_argv()
        .unwrap_err();
    assert!(
        matches!(&error, slug_build_api_v2::ArgsExpansionError::Callback(message)
            if message.contains("Args callback return: expected None, string, list or tuple of strings, got bool")),
        "{error}"
    );
    fs::remove_dir_all(root).unwrap();
}
