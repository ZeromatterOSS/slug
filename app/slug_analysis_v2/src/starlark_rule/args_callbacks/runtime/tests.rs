use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisConfiguredTargetKey;
use slug_build_api_v2::NoopArgsDirectoryContext;
use slug_build_api_v2::UnsupportedTreeArgsDirectoryContext;
use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::subrule_invocation::AnalysisArtifactValue;
use starlark::environment::Globals;
use starlark::environment::LibraryExtension;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;

fn mapper(source: &str) -> StarlarkRuntimeMapper {
    let module = Module::new();
    evaluate(&module, source);
    // The producer Module and FrozenModule both drop before invocation. The
    // executable's owned heap guard must preserve its original globals/cells.
    StarlarkRuntimeMapper::new(module.freeze().unwrap().get("mapper").unwrap())
}
pub(super) fn evaluate(module: &Module, source: &str) {
    Evaluator::new(module)
        .eval_module(
            AstModule::parse(
                "actual_original_module.bzl",
                source.to_owned(),
                &Dialect::Bazel,
            )
            .unwrap(),
            &Globals::extended_by(&[LibraryExtension::Print]),
        )
        .unwrap();
}
fn strings(values: &[&str]) -> Vec<AnalysisValue> {
    values.iter().map(|s| AnalysisValue::string(*s)).collect()
}

#[test]
fn args_runtime_invokes_original_frozen_globals_defaults_and_cells_after_producer_drop() {
    let runtime = mapper(
        r#"
prefix = "original/"
def make():
    captured = "cell:"
    def callback(value, unused = None):
        print("muted callback output")
        if value == "skip": return None
        if value == "tuple": return (prefix + captured + value, "tail")
        if value == "list": return [prefix + captured + value]
        return prefix + captured + value
    return callback
mapper = make()
"#,
    );
    for _ in 0..2 {
        assert_eq!(
            runtime
                .map_batch(
                    &strings(&["str", "skip", "tuple", "list"]),
                    &NoopArgsDirectoryContext
                )
                .unwrap(),
            [
                "original/cell:str",
                "original/cell:tuple",
                "tail",
                "original/cell:list"
            ]
        );
    }
    // Native functions use their actual one-argument convention.
    let native = mapper("mapper = str\n");
    assert_eq!(
        native
            .map_batch(&[AnalysisValue::integer(7)], &NoopArgsDirectoryContext)
            .unwrap(),
        ["7"]
    );
    let default =
        mapper("def mapper(value, unused = None, suffix = 'default'): return value + suffix\n");
    assert_eq!(
        default
            .map_batch(&strings(&["x"]), &NoopArgsDirectoryContext)
            .unwrap(),
        ["xdefault"]
    );
}

#[derive(Debug)]
struct Children(Vec<AnalysisArtifact>);
impl ArgsDirectoryContext for Children {
    fn list(&self, _: &AnalysisArtifact) -> Result<Vec<AnalysisArtifact>, ArgsExpansionError> {
        Ok(self.0.clone())
    }
}
fn source(name: &str) -> AnalysisArtifact {
    AnalysisArtifact::Source(CanonicalLabel::parse(&format!("@@//pkg:{name}")).unwrap())
}
fn tree() -> AnalysisArtifact {
    AnalysisArtifact::Derived {
        owner: AnalysisConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//pkg:producer").unwrap(),
            b"configuration".as_slice(),
        ),
        output: ActionOutput::new(
            crate::analysis_value::test_artifact_root(),
            "pkg/tree",
            ActionOutputKind::Directory,
        ),
    }
}

#[test]
fn args_runtime_directory_expander_uses_supplied_metadata_for_inputs_and_captures() {
    let tree = tree();
    let runtime = mapper(
        "def mapper(value, directory): return [f.short_path for f in directory.list(value)]\n",
    );
    assert_eq!(
        runtime
            .map_batch(
                &[AnalysisValue::artifact(tree.clone())],
                &NoopArgsDirectoryContext
            )
            .unwrap(),
        ["pkg/tree"]
    );
    assert!(matches!(
        runtime.map_batch(
            &[AnalysisValue::artifact(tree.clone())],
            &UnsupportedTreeArgsDirectoryContext
        ),
        Err(ArgsExpansionError::Unsupported(_))
    ));
    let children = Children(vec![source("first"), source("second")]);
    assert_eq!(
        runtime
            .map_batch(&[AnalysisValue::artifact(tree.clone())], &children)
            .unwrap(),
        ["pkg/first", "pkg/second"]
    );
    // Directory access is dynamic, including a file captured by the original
    // module and absent from the vector's native inputs.
    let module = Module::new();
    module.set(
        "captured",
        module
            .frozen_heap()
            .alloc(AnalysisArtifactValue::new(tree))
            .to_value(),
    );
    evaluate(
        &module,
        "def mapper(value, directory): return [f.short_path for f in directory.list(captured)]\n",
    );
    let captured = StarlarkRuntimeMapper::new(module.freeze().unwrap().get("mapper").unwrap());
    assert_eq!(
        captured
            .map_batch(&strings(&["unused"]), &children)
            .unwrap(),
        ["pkg/first", "pkg/second"]
    );
    let wrong = mapper("def mapper(value, directory): return directory.list(value)\n");
    assert!(matches!(
        wrong.map_batch(&strings(&["wrong"]), &children),
        Err(ArgsExpansionError::Callback(_))
    ));
}

#[test]
fn args_runtime_return_errors_and_resource_failures_are_fallible() {
    for source in [
        "def mapper(x): return 7\n",
        "def mapper(x): return ['ok', 7]\n",
        "def mapper(x): return ('ok', None)\n",
    ] {
        assert!(matches!(
            mapper(source).map_batch(&strings(&["x"]), &NoopArgsDirectoryContext),
            Err(ArgsExpansionError::InvalidReturn(_))
        ));
    }
    let error = mapper("def mapper(x): fail('original failure')\n")
        .map_batch(&strings(&["x"]), &NoopArgsDirectoryContext)
        .unwrap_err();
    assert!(
        matches!(error, ArgsExpansionError::Callback(ref text) if text.contains("original failure") && text.contains("actual_original_module.bzl"))
    );
    let cancelled = mapper("def mapper(x): return x\n").with_cancellation(Arc::new(|| true));
    assert_eq!(
        cancelled.map_batch(&strings(&["x"]), &NoopArgsDirectoryContext),
        Err(ArgsExpansionError::Cancelled)
    );
    for limits in [
        ArgsRuntimeLimits {
            input_count: 0,
            ..Default::default()
        },
        ArgsRuntimeLimits {
            output_count: 0,
            ..Default::default()
        },
        ArgsRuntimeLimits {
            output_bytes: 0,
            ..Default::default()
        },
        ArgsRuntimeLimits {
            heap_bytes: 0,
            ..Default::default()
        },
        ArgsRuntimeLimits {
            timeout: Duration::ZERO,
            ..Default::default()
        },
    ] {
        assert!(matches!(
            mapper("def mapper(x): return x\n")
                .with_limits(limits)
                .map_batch(&strings(&["x"]), &NoopArgsDirectoryContext),
            Err(ArgsExpansionError::Limit(_))
        ));
    }
    // Cancellation also reaches already-frozen bytecode loop checkpoints.
    let checks = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = checks.clone();
    let loop_mapper =
        mapper("def mapper(x):\n    for i in range(10000000):\n        x = str(i)\n    return x\n")
            .with_cancellation(Arc::new(move || {
                counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= 10
            }));
    assert_eq!(
        loop_mapper.map_batch(&strings(&["x"]), &NoopArgsDirectoryContext),
        Err(ArgsExpansionError::Cancelled)
    );
    assert!(checks.load(std::sync::atomic::Ordering::Relaxed) >= 11);
}

#[starlark::starlark_module]
fn cache_gc_helpers(builder: &mut starlark::environment::GlobalsBuilder) {
    fn collect(eval: &mut Evaluator<'_, '_, '_>) -> anyhow::Result<bool> {
        // Allocate inside the native call so no earlier bytecode checkpoint can
        // reclaim the test garbage before this initialized-frame collection.
        eval.heap().alloc(starlark::values::list::AllocList(vec![
            "unreachable-during-callback".to_owned(); 512
        ]));
        let before = eval.heap().allocated_bytes();
        unsafe { eval.garbage_collect() };
        Ok(eval.heap().allocated_bytes() < before)
    }
}

#[derive(Debug)]
struct BatchDirectoryMetadata {
    tree: AnalysisArtifact,
    children: Vec<AnalysisArtifact>,
    requested: std::sync::Mutex<Vec<AnalysisArtifact>>,
}
impl ArgsDirectoryContext for BatchDirectoryMetadata {
    fn list(
        &self,
        artifact: &AnalysisArtifact,
    ) -> Result<Vec<AnalysisArtifact>, ArgsExpansionError> {
        self.requested.lock().unwrap().push(artifact.clone());
        Ok(if artifact == &self.tree {
            self.children.clone()
        } else {
            vec![artifact.clone()]
        })
    }
}

#[test]
fn args_runtime_actual_depset_cache_gc_preserves_expander_captures_and_all_batch_inputs() {
    let producer = Module::new();
    producer.set(
        "captured_file",
        producer
            .frozen_heap()
            .alloc(AnalysisArtifactValue::new(source("captured")))
            .to_value(),
    );
    let depset = slug_loading_v2::alloc_frozen_analysis_builtin_callable(
        producer.frozen_heap(),
        slug_loading_v2::AnalysisBuiltinCallableApi::Depset,
    );
    producer.set("depset", depset.to_value());
    let globals = starlark::environment::GlobalsBuilder::standard()
        .with(cache_gc_helpers)
        .build();
    Evaluator::new(&producer).eval_module(
        AstModule::parse("cache_and_batch_liveness.bzl", r#"
def mapper(value, directory):
    empty = depset([])
    if empty.to_list():
        fail("actual empty depset must be empty")
    if not collect():
        fail("initialized-frame GC must reclaim test garbage")
    if empty.to_list() or depset([]).to_list():
        fail("actual depset cache must remain alive after GC")
    return [captured_file.short_path + ":" + value.short_path] + [child.short_path for child in directory.list(value)]
"#.to_owned(), &Dialect::Bazel).unwrap(), &globals,
    ).unwrap();
    // Only the runtime's actual original frozen callable owns the producer now.
    let runtime = StarlarkRuntimeMapper::new(producer.freeze().unwrap().get("mapper").unwrap())
        .with_limits(ArgsRuntimeLimits {
            input_count: 3,
            output_count: 8,
            output_bytes: 256,
            heap_bytes: 128 * 1024,
            timeout: Duration::from_secs(1),
            call_stack: 32,
        });
    let tree = tree();
    let plain = source("plain");
    let context = BatchDirectoryMetadata {
        tree: tree.clone(),
        children: vec![source("first"), source("second")],
        requested: std::sync::Mutex::new(Vec::new()),
    };
    let inputs = vec![
        AnalysisValue::artifact(tree.clone()),
        AnalysisValue::artifact(plain.clone()),
        AnalysisValue::artifact(tree.clone()),
    ];
    for _ in 0..2 {
        assert_eq!(
            runtime.map_batch(&inputs, &context).unwrap(),
            [
                "pkg/captured:pkg/tree",
                "pkg/first",
                "pkg/second",
                "pkg/captured:pkg/plain",
                "pkg/plain",
                "pkg/captured:pkg/tree",
                "pkg/first",
                "pkg/second",
            ]
        );
    }
    assert_eq!(
        *context.requested.lock().unwrap(),
        [
            tree.clone(),
            plain.clone(),
            tree.clone(),
            tree.clone(),
            plain,
            tree
        ]
    );
    let limits = runtime.limits;
    assert!(
        matches!(
            runtime
                .with_limits(ArgsRuntimeLimits {
                    output_count: 7,
                    ..limits
                })
                .map_batch(&inputs, &context),
            Err(ArgsExpansionError::Limit(_))
        ),
        "the same real callback must still obey its cumulative output quota"
    );
}
