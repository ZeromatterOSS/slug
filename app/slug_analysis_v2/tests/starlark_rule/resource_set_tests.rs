//! Actual unrelated original resource callbacks, independent of any source recipe.
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use slug_build_api_v2::actions::ResourceSetError;
use slug_build_api_v2::actions::ResourceSetEvaluationContext;
use slug_build_api_v2::actions::ResourceSetInputs;
use slug_build_api_v2::actions::ResourceSetOs;

use super::*;

#[derive(Debug, Default)]
struct Consumer {
    cancelled: AtomicBool,
    printed: Mutex<Vec<String>>,
}
impl ResourceSetEvaluationContext for Consumer {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
    fn print(&self, text: &str) -> Result<(), ResourceSetError> {
        self.printed.lock().unwrap().push(text.to_owned());
        Ok(())
    }
    fn mnemonic(&self) -> &str {
        "IndependentResource"
    }
}
const CALLBACKS: &str = r#"DATA = [300]
MAPPING = Label('@alias//:input')
def estimate(os, count):
    print('resource:' + str(MAPPING))
    return {'memory': DATA[0] + count, 'cpu': 2 if os == 'osx' else 1, 'local_test': 3.75}
def explode(os, count):
    fail('would-error-original-resource')
"#;
const DEFS: &str = r#"load(':callbacks.bzl', _estimate='estimate', _explode='explode')
def _empty(value): return str(value)
def _register(ctx, shell, empty, callback):
    out = ctx.actions.declare_file('out')
    arguments = []
    if empty:
        args = ctx.actions.args()
        if empty == 2: args.add_all([], map_each=_empty)
        arguments = [args]
    if shell:
        ctx.actions.run_shell(outputs=[out], command='true', arguments=arguments, resource_set=callback)
    else:
        ctx.actions.run(outputs=[out], executable='tool', arguments=arguments, resource_set=callback)
    return out

def _child(ctx, shell, empty, callback):
    return _register(ctx, shell, empty, callback)
child = subrule(implementation=_child)
def _impl(ctx):
    callback = _explode if ctx.attr.bad else _estimate
    out = child(ctx.attr.shell, ctx.attr.empty, callback) if ctx.attr.sub else _register(ctx, ctx.attr.shell, ctx.attr.empty, callback)
    return [DefaultInfo(files=depset([out]))]
subject = rule(implementation=_impl, attrs={'shell':attr.bool(),'sub':attr.bool(),'bad':attr.bool(),'empty':attr.int()}, subrules=[child])
"#;
fn module(selected: &str) -> String {
    format!(
        "module(name='resource_root')\nbazel_dep(name='{selected}',version='1.0.0',repo_name='alias')\nlocal_path_override(module_name='{selected}',path='{selected}')\n"
    )
}
fn workspace() -> PathBuf {
    let root = scratch();
    fs::write(root.join("MODULE.bazel"), module("a")).unwrap();
    for name in ["a", "b"] {
        fs::create_dir_all(root.join(name)).unwrap();
        fs::write(
            root.join(name).join("MODULE.bazel"),
            format!("module(name='{name}',version='1.0.0')\n"),
        )
        .unwrap();
        fs::write(
            root.join(name).join("BUILD.bazel"),
            "exports_files(['input'])\n",
        )
        .unwrap();
        fs::write(root.join(name).join("input"), "source").unwrap();
    }
    fs::write(root.join("callbacks.bzl"), CALLBACKS).unwrap();
    fs::write(root.join("defs.bzl"), DEFS).unwrap();
    let mut build = "load(':defs.bzl','subject')\n".to_owned();
    for sub in [false, true] {
        for shell in [false, true] {
            for empty in 0..3 {
                build.push_str(&format!(
                    "subject(name='r{}{}{}',sub={},shell={},empty={})\n",
                    usize::from(sub),
                    usize::from(shell),
                    empty,
                    if sub { "True" } else { "False" },
                    if shell { "True" } else { "False" },
                    empty
                ));
            }
        }
    }
    build.push_str("subject(name='bad',bad=True)\n");
    fs::write(root.join("BUILD.bazel"), build).unwrap();
    root
}
async fn request(
    dice: &Arc<Dice>,
    root: &PathBuf,
    name: &str,
) -> Result<Arc<ConfiguredNodeResult>, String> {
    root_target_request_with_explicit_inputs_and_prefix(
        dice,
        root,
        &format!("@@//:{name}"),
        typed_action_test_configuration(),
        None,
        Arc::new(RootActivationTracker::default()),
        builtin_fixture::FixtureObservations::Automatic,
        &[("a+", "a"), ("b+", "b")],
        true,
    )
    .await
}
fn descriptor(
    result: &ConfiguredNodeResult,
) -> slug_build_api_v2::actions::RetainedSpawnResourceSet {
    assert_eq!(result.actions().len(), 1);
    result.actions()[0]
        .spawn_spec()
        .unwrap()
        .resource_set()
        .unwrap()
        .clone()
}
#[tokio::test]
async fn original_resources_survive_all_four_registration_routes_and_empty_args_after_producer_drop()
 {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut retained = Vec::new();
    for sub in 0..2 {
        for shell in 0..2 {
            for empty in 0..3 {
                let value = request(&dice, &root, &format!("r{sub}{shell}{empty}"))
                    .await
                    .unwrap();
                let resource = descriptor(&value);
                assert!(resource.published_callable().original_slot().is_some());
                let command = value.actions()[0]
                    .spawn_spec()
                    .unwrap()
                    .render_argv()
                    .unwrap();
                assert_eq!(
                    command,
                    if shell == 0 {
                        vec!["tool".to_owned()]
                    } else {
                        {
                            let mut shell =
                                vec!["/bin/bash".to_owned(), "-c".to_owned(), "true".to_owned()];
                            if empty != 0 {
                                shell.push(String::new());
                            }
                            shell
                        }
                    }
                );
                retained.push(resource);
            }
        }
    }
    // Body failure is deferred too; registration and final freeze succeeded.
    let bad = descriptor(&request(&dice, &root, "bad").await.unwrap());
    dice.wait_for_idle().await;
    drop(dice);
    fs::remove_dir_all(&root).unwrap();
    let consumer = Consumer::default();
    for resource in retained {
        let value = resource
            .evaluate(
                ResourceSetInputs {
                    os: ResourceSetOs::Linux,
                    parent_input_count: 2,
                },
                &consumer,
            )
            .unwrap();
        assert_eq!(
            (value.memory_mb, value.cpu, value.local_test),
            (302.0, 1.0, 3)
        );
    }
    assert_eq!(consumer.printed.lock().unwrap().len(), 12);
    let err = bad
        .evaluate(
            ResourceSetInputs {
                os: ResourceSetOs::Linux,
                parent_input_count: 2,
            },
            &consumer,
        )
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("would-error-original-resource") && err.contains("IndependentResource"),
        "{err}"
    );
}
#[tokio::test]
async fn resource_source_mapping_and_equal_foreign_owners_restore_complete_semantics() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let a = request(&dice, &root, "r000").await.unwrap();
    let other = Dice::builder().build(DetectCycles::Enabled);
    let foreign = request(&other, &root, "r000").await.unwrap();
    assert_eq!(a, foreign);
    assert!(
        !descriptor(&a)
            .published_callable()
            .same_original(descriptor(&foreign).published_callable())
    );
    let consumer = Consumer::default();
    let inputs = ResourceSetInputs {
        os: ResourceSetOs::Osx,
        parent_input_count: 4,
    };
    let answer = descriptor(&a).evaluate(inputs, &consumer).unwrap();
    assert_eq!(
        (answer.memory_mb, answer.cpu, answer.local_test),
        (304.0, 2.0, 3)
    );
    fs::write(
        root.join("callbacks.bzl"),
        CALLBACKS.replace("[300]", "[500]"),
    )
    .unwrap();
    let b = request(&dice, &root, "r000").await.unwrap();
    assert_ne!(a, b);
    assert_eq!(
        descriptor(&b)
            .evaluate(inputs, &consumer)
            .unwrap()
            .memory_mb,
        504.0
    );
    fs::write(root.join("callbacks.bzl"), CALLBACKS).unwrap();
    assert_eq!(a, request(&dice, &root, "r000").await.unwrap());
    fs::write(root.join("MODULE.bazel"), module("b")).unwrap();
    let mapping = request(&dice, &root, "r000").await.unwrap();
    assert_ne!(a, mapping);
    assert_eq!(
        descriptor(&mapping)
            .evaluate(inputs, &consumer)
            .unwrap()
            .memory_mb,
        304.0
    );
    assert_eq!(
        a.actions()[0].spawn_spec().unwrap().render_argv().unwrap(),
        mapping.actions()[0]
            .spawn_spec()
            .unwrap()
            .render_argv()
            .unwrap()
    );
    fs::write(root.join("MODULE.bazel"), module("a")).unwrap();
    assert_eq!(a, request(&dice, &root, "r000").await.unwrap());
    fs::remove_dir_all(root).unwrap();
}
