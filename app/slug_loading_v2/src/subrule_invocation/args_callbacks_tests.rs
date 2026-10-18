use starlark::environment::Globals;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::list::AllocList;
use starlark::values::none::NoneType;
use starlark::values::tuple::TupleRef;

use super::*;

#[starlark::starlark_module]
fn gc_helpers(builder: &mut GlobalsBuilder) {
    fn collect(eval: &mut Evaluator<'_, '_, '_>) -> anyhow::Result<NoneType> {
        // The native call runs with an initialized bytecode frame and holds no
        // argument Values across collection; the module traces snapshot roots.
        unsafe {
            eval.garbage_collect()
                .map_err(|error| error.into_anyhow())?
        };
        Ok(NoneType)
    }
}

fn setup(module: &Module) {
    module.set(
        "args",
        StarlarkArgs::from_snapshot(
            EvaluatorArgsSnapshot {
                calls: Vec::new(),
                format: RetainedParamFileFormat::Shell,
                param_file: None,
            },
            module.heap(),
        ),
    );
}

fn evaluate<'v>(module: &'v Module, source: &str) -> starlark::Result<Value<'v>> {
    Evaluator::new(module).eval_module(
        AstModule::parse(
            "relocated_unrelated.bzl",
            source.to_owned(),
            &Dialect::Bazel,
        )
        .unwrap(),
        &Globals::standard(),
    )
}

#[test]
fn args_add_all_and_joined_record_actual_callables_without_eager_invocation() {
    let module = Module::new();
    setup(&module);
    module.set(
        "empty",
        crate::provider::alloc_starlark_depset(
            module.frozen_heap(),
            slug_build_api_v2::AnalysisDepset::empty(slug_build_api_v2::DepsetOrder::Default),
            Vec::new(),
        )
        .to_value(),
    );
    evaluate(
        &module,
        r#"
def mapper(value): fail("must be deferred")
args.add_all([0], map_each = mapper)
args.add_joined([False], map_each = mapper, join_with = ":")
args.add_all([], map_each = None, allow_closure = True)
args.add_all(empty, map_each = None, allow_closure = True)
"#,
    )
    .unwrap();
    let snapshot = StarlarkArgs::snapshot(module.get("args").unwrap()).unwrap();
    assert_eq!(snapshot.calls.len(), 2);
    for call in &snapshot.calls {
        let vector = match call {
            EvaluatorArgCallGen::AddAll(value) | EvaluatorArgCallGen::AddJoined(value) => value,
            _ => panic!("vector call"),
        };
        let Some(EvaluatorVectorMapEachGen::Callable(callback)) = vector.map_each else {
            panic!("actual callback")
        };
        assert!(callback.ptr_eq(module.get("mapper").unwrap()));
        assert!(
            vector
                .registration_location
                .as_ref()
                .is_some_and(|location| location.contains("relocated_unrelated.bzl"))
        );
    }
    let mut evaluator = Evaluator::new(&module);
    let snapshot_callback = match &snapshot.calls[0] {
        EvaluatorArgCallGen::AddAll(value) => match value.map_each {
            Some(EvaluatorVectorMapEachGen::Callable(value)) => value,
            _ => panic!("callback"),
        },
        _ => panic!("add_all"),
    };
    assert!(
        evaluator
            .eval_function(snapshot_callback, &[Value::new_none()], &[])
            .unwrap_err()
            .to_string()
            .contains("must be deferred")
    );
    assert!(!vector_source_is_empty(
        &EvaluatorVectorSourceGen::Sequence(vec![Value::new_bool(false)])
    ));
}

#[test]
fn args_callback_validation_uses_actual_global_binding_and_allow_closure_for_both_methods() {
    for operation in ["add_all([0]", "add_joined([0], join_with = ':'"] {
        for (definitions, allow, accepted) in [
            ("def mapper(x): return x\n", false, true),
            (
                "def mapper(x): return x\nsaved = mapper\nmapper = 0\nmapper = saved\n",
                false,
                true,
            ),
            (
                "def mapper(x): return x\nsaved = mapper\nmapper = 0\n",
                false,
                false,
            ),
            ("mapper = lambda x: x\n", false, false),
            ("mapper = lambda x: x\n", true, true),
            (
                "def make():\n    captured = 'kept'\n    def nested(x): return captured\n    return nested\nmapper = make()\n",
                false,
                false,
            ),
            (
                "def make():\n    captured = 'kept'\n    def nested(x): return captured\n    return nested\nmapper = make()\n",
                true,
                true,
            ),
            ("mapper = None\n", true, true),
            ("mapper = 7\n", true, false),
            ("mapper = str\n", false, true),
        ] {
            let module = Module::new();
            setup(&module);
            let callback = if definitions.contains("mapper = 0\n")
                && !definitions.contains("mapper = saved")
            {
                "saved"
            } else {
                "mapper"
            };
            let source = format!(
                "{definitions}args.{operation}, map_each = {callback}, allow_closure = {})\n",
                if allow { "True" } else { "False" }
            );
            let result = evaluate(&module, &source);
            assert_eq!(result.is_ok(), accepted, "{source}\n{result:?}");
            let count = StarlarkArgs::snapshot(module.get("args").unwrap())
                .unwrap()
                .calls
                .len();
            assert_eq!(count, usize::from(accepted));
            if let Err(error) = result {
                assert!(error.to_string().contains("map_each"), "{error}");
            }
        }
    }
}

#[test]
fn pending_args_snapshots_trace_member_references_check_membership_and_require_publication() {
    let module = Module::new();
    setup(&module);
    evaluate(
        &module,
        r#"
def mapper(item): return item["member"]
args.add_all([{"member": "retained"}], map_each = mapper)
"#,
    )
    .unwrap();
    let token = register_args_snapshot(
        &module,
        StarlarkArgs::snapshot(module.get("args").unwrap()).unwrap(),
    )
    .unwrap();
    module.set("args", Value::new_none());
    module.set("mapper", Value::new_none());
    module
        .heap()
        .alloc(AllocList(vec!["discarded".to_owned(); 8192]));
    // Collection may begin at the top-level statement checkpoint, before the
    // native collect() call. Measure the entire active evaluation interval.
    let before = module.heap().allocated_bytes();
    Evaluator::new(&module)
        .eval_module(
            AstModule::parse("snapshot_gc.bzl", "collect()".to_owned(), &Dialect::Bazel).unwrap(),
            &GlobalsBuilder::standard().with(gc_helpers).build(),
        )
        .unwrap();
    let after = module.heap().allocated_bytes();
    assert!(
        after < before,
        "GC must reclaim unreachable allocations: before={before}, after={after}"
    );
    let old_roots = TupleRef::from_value(module.extra_value().unwrap())
        .unwrap()
        .content()[..3]
        .to_vec();
    let snapshot = args_snapshot(&module, &token).unwrap();
    let EvaluatorArgCallGen::AddAll(vector) = &snapshot.calls[0] else {
        panic!("vector")
    };
    let EvaluatorVectorSourceGen::Sequence(values) = &vector.source else {
        panic!("sequence")
    };
    let Some(EvaluatorVectorMapEachGen::Callable(callback)) = vector.map_each else {
        panic!("callback")
    };
    assert_eq!(
        Evaluator::new(&module)
            .eval_function(callback, &[values[0]], &[])
            .unwrap()
            .unpack_str(),
        Some("retained")
    );
    let other = Module::new();
    let other_token = register_args_snapshot(
        &other,
        EvaluatorArgsSnapshot {
            calls: Vec::new(),
            format: RetainedParamFileFormat::Shell,
            param_file: None,
        },
    )
    .unwrap();
    assert!(args_snapshot(&other, &token).is_err());
    assert!(args_snapshot(&module, &other_token).is_err());
    clear_args_snapshots(&other).unwrap();
    assert!(other.freeze().is_ok());
    clear_args_snapshots(&module).unwrap();
    assert!(args_snapshot(&module, &token).is_err());
    let roots = TupleRef::from_value(module.extra_value().unwrap()).unwrap();
    assert!(
        old_roots
            .iter()
            .zip(&roots.content()[..3])
            .all(|(old, current)| old.ptr_eq(*current))
    );
    // Replace with a fresh collector: phase indexes alone must not authorize stale tokens.
    let replacement = register_args_snapshot(&module, snapshot).unwrap();
    assert!(args_snapshot(&module, &token).is_err());
    assert!(args_snapshot(&module, &replacement).is_ok());
    assert!(format!("{:?}", module.freeze().unwrap_err()).contains("pending Args snapshots"));
}
