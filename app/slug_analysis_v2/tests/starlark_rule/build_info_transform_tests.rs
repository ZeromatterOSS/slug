//! Genuine unrelated DEFAULT-allowed originals; no rendering recipe fixture.
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::BuildInfoEvaluationContext;
use slug_build_api_v2::BuildInfoTransformError;

use super::*;
#[derive(Default)]
struct Consumer {
    printed: Mutex<Vec<String>>,
}
impl BuildInfoEvaluationContext for Consumer {
    fn is_cancelled(&self) -> bool {
        false
    }
    fn print(&self, text: &str) -> Result<(), BuildInfoTransformError> {
        self.printed.lock().unwrap().push(text.to_owned());
        Ok(())
    }
    fn mnemonic(&self) -> &str {
        "TranslateBuildInfo"
    }
}
const DEFS: &str = r#"def common(status, count=None):
    print('build-info-original')
    return {'{A}': status.get('NAME', 'missing'), '{B}': 'tail'}
def make():
    captured = ['captured']
    def callback(status):
        print(captured[0])
        return {'{A}': captured[0], '{B}': status.get('NAME', 'missing')}
    return callback
closure = make()
Info = provider(fields=['callback'])
def _impl(ctx):
    callback = closure if ctx.attr.capture else common
    template = ctx.file.template
    if ctx.attr.derived:
        template = ctx.actions.declare_file('derived.template')
        ctx.actions.write(output=template, content='{A}|{B}')
    info = ctx.actions.transform_info_file(transform_func=callback, template=template, output_file_name='info.h')
    version = ctx.actions.transform_version_file(transform_func=callback, template=template, output_file_name='version.h')
    args = ctx.actions.args()
    args.add_all([], map_each=callback, allow_closure=True, omit_if_empty=False)
    out = ctx.actions.declare_file('args')
    ctx.actions.write(output=out, content=args)
    return [DefaultInfo(files=depset([info,version,out])),Info(callback=callback)]
subject = rule(implementation=_impl, attrs={'capture':attr.bool(),'derived':attr.bool(),'template':attr.label(allow_single_file=True,default=Label('//:template'))})
"#;
fn workspace() -> PathBuf {
    let root = scratch();
    fs::write(
        root.join("MODULE.bazel"),
        "module(name='build_info_root')\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("test")).unwrap();
    fs::write(
        root.join("test/BUILD.bazel"),
        "exports_files(['defs.bzl'])\n",
    )
    .unwrap();
    fs::write(root.join("test/defs.bzl"), DEFS).unwrap();
    fs::write(root.join("template"), "{A}|{B}").unwrap();
    fs::write(root.join("BUILD.bazel"), "load('//test:defs.bzl','subject')\nexports_files(['template'])\nsubject(name='plain')\nsubject(name='captured',capture=True)\nsubject(name='derived',derived=True)\n").unwrap();
    root
}
async fn request(dice: &Arc<Dice>, root: &PathBuf, name: &str) -> Arc<ConfiguredNodeResult> {
    root_target_request_with_explicit_inputs_and_prefix(
        dice,
        root,
        &format!("@@//:{name}"),
        typed_action_test_configuration(),
        None,
        Arc::new(RootActivationTracker::default()),
        builtin_fixture::FixtureObservations::Automatic,
        &[],
        true,
    )
    .await
    .unwrap()
}
#[tokio::test]
async fn unrelated_build_info_originals_capture_both_methods_and_outlive_producer() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut held = Vec::new();
    for name in ["plain", "captured", "derived"] {
        let value = request(&dice, &root, name).await;
        let specs: Vec<_> = value
            .actions()
            .iter()
            .filter_map(|a| a.build_info_transform_spec().cloned())
            .collect();
        assert_eq!(specs.len(), 2);
        assert!(!specs[0].is_volatile());
        assert!(specs[1].is_volatile());
        assert_eq!(specs[0].callback(), specs[1].callback());
        assert!(
            specs[0]
                .callback()
                .published_callable()
                .same_original(specs[1].callback().published_callable())
        );
        let writes: Vec<_> = value
            .actions()
            .iter()
            .filter_map(|a| a.args_write_spec())
            .collect();
        let [write] = writes.as_slice() else {
            panic!("one actual Args write")
        };
        let [slug_build_api_v2::RetainedArgCall::AddAll(vector)] = write.recipe().calls() else {
            panic!("the empty vector retains its registered mapper")
        };
        let slug_build_api_v2::RetainedVectorSource::Mapped { mapper, .. } = vector.source() else {
            panic!("actual mapped vector source")
        };
        let original = specs[0].callback().published_callable();
        let mapped = mapper.published_callable();
        assert!(original.same_original(mapped));
        assert_eq!(original.node_id(), mapped.node_id());
        assert_eq!(original.arena().identity(), mapped.arena().identity());
        assert_eq!(original.original_slot(), mapped.original_slot());
        // The provider field, Args mapper and both build-info registrations
        // must share the SAME publication/original, not merely equal functions.
        let info = value
            .providers()
            .user(&ProviderId::new("//test:defs.bzl", "Info").unwrap())
            .unwrap();
        let provided = info.field("callback").unwrap().original_origin().unwrap();
        assert!(original.same_original(provided));
        assert_eq!(original.node_id(), provided.node_id());
        assert_eq!(original.arena().identity(), provided.arena().identity());
        for spec in &specs {
            assert!(
                spec.callback()
                    .published_callable()
                    .original_slot()
                    .is_some()
            );
            assert_eq!(
                matches!(spec.template(), AnalysisArtifact::Derived { .. }),
                name == "derived"
            );
        }
        held.push((name, specs));
    }
    dice.wait_for_idle().await;
    drop(dice);
    fs::remove_dir_all(&root).unwrap();
    let consumer = Consumer::default();
    assert!(
        consumer.printed.lock().unwrap().is_empty(),
        "metadata has not evaluated callbacks"
    );
    for (name, specs) in held {
        for spec in specs {
            assert_eq!(
                spec.callback()
                    .render(b"NAME first\nNAME supplied\n", b"{A}|{B}", &consumer)
                    .unwrap(),
                if name == "captured" {
                    b"captured|supplied".as_slice()
                } else {
                    b"supplied|tail".as_slice()
                }
            );
        }
    }
    assert_eq!(
        *consumer.printed.lock().unwrap(),
        vec![
            "build-info-original",
            "build-info-original",
            "captured",
            "captured",
            "build-info-original",
            "build-info-original"
        ]
    );
}
#[tokio::test]
async fn equal_foreign_build_info_originals_remain_structural_after_disposal() {
    let root = workspace();
    let first = Dice::builder().build(DetectCycles::Enabled);
    let second = Dice::builder().build(DetectCycles::Enabled);
    let a = request(&first, &root, "plain").await;
    let b = request(&second, &root, "plain").await;
    let get = |v: &ConfiguredNodeResult| {
        v.actions()
            .iter()
            .find_map(|a| a.build_info_transform_spec())
            .unwrap()
            .callback()
            .clone()
    };
    let a = get(&a);
    let b = get(&b);
    assert_eq!(a, b);
    assert!(!a.published_callable().same_original(b.published_callable()));
    first.wait_for_idle().await;
    second.wait_for_idle().await;
    drop(first);
    drop(second);
    fs::remove_dir_all(root).unwrap();
    let sink = Consumer::default();
    assert_eq!(
        a.render(b"NAME x", b"{A}|{B}", &sink).unwrap(),
        b.render(b"NAME x", b"{A}|{B}", &sink).unwrap()
    );
    assert_eq!(sink.printed.lock().unwrap().len(), 2);
}
