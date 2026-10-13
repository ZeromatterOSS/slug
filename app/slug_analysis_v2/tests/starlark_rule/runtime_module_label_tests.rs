use super::*;

const LEAF: &str = "def resolve(ctor):\n    return '|'.join([str(ctor(':local')), str(ctor('//pkg:item')), str(ctor('@alias//mapped:item'))])\n";

fn leaf_module(selected: &str) -> String {
    format!(
        "module(name='leaf', version='1.0.0')\nbazel_dep(name='{selected}', version='1.0.0', repo_name='alias')\n"
    )
}

#[tokio::test]
async fn configured_runtime_labels_use_nested_original_module_and_restore_source_and_mapping() {
    let workspace = scratch();
    for directory in ["leaf/middle", "leaf/inner", "a", "b"] {
        fs::create_dir_all(workspace.join(directory)).unwrap();
    }
    fs::write(workspace.join("MODULE.bazel"), "module(name='root')\nbazel_dep(name='leaf',version='1.0.0')\nlocal_path_override(module_name='leaf',path='leaf')\nbazel_dep(name='a',version='1.0.0')\nlocal_path_override(module_name='a',path='a')\nbazel_dep(name='b',version='1.0.0')\nlocal_path_override(module_name='b',path='b')\n").unwrap();
    for name in ["a", "b"] {
        fs::write(
            workspace.join(name).join("MODULE.bazel"),
            format!("module(name='{name}',version='1.0.0')\n"),
        )
        .unwrap();
    }
    for repository in ["leaf", "a", "b"] {
        fs::write(workspace.join(repository).join("REPO.bazel"), "").unwrap();
        fs::write(workspace.join(repository).join(".bazelignore"), "").unwrap();
    }
    fs::write(workspace.join("leaf/MODULE.bazel"), leaf_module("a")).unwrap();
    fs::write(workspace.join("leaf/inner/BUILD.bazel"), "").unwrap();
    fs::write(workspace.join("leaf/inner/defs.bzl"), LEAF).unwrap();
    fs::write(workspace.join("leaf/middle/BUILD.bazel"), "").unwrap();
    fs::write(
        workspace.join("leaf/middle/defs.bzl"),
        "load('//inner:defs.bzl', 'resolve')\ndef invoke(ctor): return resolve(ctor)\n",
    )
    .unwrap();
    fs::write(workspace.join("leaf/defs.bzl"), "load('//middle:defs.bzl', 'invoke')\nObserved = provider(fields={'value':''})\nconstructor = Label\ndef _impl(ctx): return [Observed(value=invoke(constructor))]\nconsumer = rule(implementation=_impl)\n").unwrap();
    fs::write(
        workspace.join("leaf/BUILD.bazel"),
        "load(':defs.bzl','consumer')\nconsumer(name='consumer')\n",
    )
    .unwrap();
    fs::write(
        workspace.join("BUILD.bazel"),
        "alias(name='consumer',actual='@leaf//:consumer')\n",
    )
    .unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let repositories = [("leaf+", "leaf"), ("a+", "a"), ("b+", "b")];
    let provider = ProviderId::new("@@leaf+//:defs.bzl", "Observed").unwrap();
    let request = || {
        observed_root_target_request_with_inputs(
            &dice,
            &workspace,
            "@@//:consumer",
            test_configuration(),
            Arc::new(RootActivationTracker::default()),
            root_epoch(&workspace),
            &repositories,
        )
    };
    let first = request().await.unwrap();
    assert_eq!(
        provider_value(&first, &provider),
        "@@leaf+//inner:local|@@leaf+//pkg:item|@@a+//mapped:item"
    );
    fs::write(
        workspace.join("leaf/inner/defs.bzl"),
        LEAF.replace(":local", ":changed"),
    )
    .unwrap();
    let changed = request().await.unwrap();
    assert_eq!(
        provider_value(&changed, &provider),
        "@@leaf+//inner:changed|@@leaf+//pkg:item|@@a+//mapped:item"
    );
    fs::write(workspace.join("leaf/inner/defs.bzl"), LEAF).unwrap();
    assert_eq!(request().await.unwrap(), first);
    fs::write(workspace.join("leaf/MODULE.bazel"), leaf_module("b")).unwrap();
    let mapped = request().await.unwrap();
    assert_eq!(
        provider_value(&mapped, &provider),
        "@@leaf+//inner:local|@@leaf+//pkg:item|@@b+//mapped:item"
    );
    fs::write(workspace.join("leaf/MODULE.bazel"), leaf_module("a")).unwrap();
    assert_eq!(request().await.unwrap(), first);
    dice.wait_for_idle().await;
    drop(dice);
    fs::remove_dir_all(workspace).unwrap();
}
