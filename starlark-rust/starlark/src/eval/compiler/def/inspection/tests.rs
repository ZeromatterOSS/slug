use super::*;
use crate as starlark;
use crate::environment::Globals;
use crate::environment::GlobalsBuilder;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::starlark_module;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::function::StarlarkFunctionIdentity;
use crate::values::tuple::TupleRef;

#[starlark_module]
fn helpers(builder: &mut GlobalsBuilder) {
    fn unset<'v>(function: Value<'v>, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<bool> {
        let function = inspect_def(function, Some(eval.module())).unwrap();
        Ok(function.captures().next().unwrap().contents.is_none())
    }
    fn collect(eval: &mut Evaluator<'_, '_, '_>) -> anyhow::Result<bool> {
        // Only Module-rooted values are live at this statement boundary.
        unsafe {
            eval.garbage_collect();
        }
        Ok(true)
    }
}

fn evaluate(module: &Module, filename: &str, source: &str, dialect: &Dialect, globals: &Globals) {
    Evaluator::new(module)
        .eval_module(
            AstModule::parse(filename, source.to_owned(), dialect).unwrap(),
            globals,
        )
        .unwrap();
}

fn budget() -> InspectionBudget {
    InspectionBudget {
        remaining_nodes: 10_000,
        maximum_depth: 100,
    }
}

#[test]
fn borrowed_function_metadata_defaults_cells_and_global_binding_survive_freeze_gc() {
    let module = Module::new();
    module
        .set_client_data_no_overwrite(module.heap().alloc("original metadata"))
        .unwrap();
    let globals = GlobalsBuilder::standard().with(helpers).build();
    evaluate(
        &module,
        "metadata.bzl",
        r#"
def original(value, default = ("", False), *, keyword = 3):
    return default
before = original
original = "rebound"
def factory():
    def nested(x = "default"):
        return later
    observed_unset = unset(nested)
    later = ["shared"]
    def other():
        return later
    return nested, other, observed_unset
pair = factory()
nested = pair[0]
"#,
        &Dialect::Bazel,
        &globals,
    );
    {
        let function = inspect_def(module.get("before").unwrap(), Some(&module)).unwrap();
        assert_eq!(function.name(), "original");
        assert_eq!(function.source().filename(), "metadata.bzl");
        assert_eq!(function.signature_span().file.filename(), "metadata.bzl");
        assert_eq!(
            function.original_module_client_data().unwrap().unpack_str(),
            Some("original metadata")
        );
        assert!(!function.is_global().unwrap());
        assert_eq!(
            function.global_binding().unwrap().unwrap().unpack_str(),
            Some("rebound")
        );
        let parameters = function.parameters().collect::<Vec<_>>();
        assert_eq!(function.parameter_count(), 3);
        assert_eq!(parameters[0].name, "value");
        assert!(matches!(
            parameters[0].requirement,
            ParameterRequirement::Required
        ));
        assert_eq!(parameters[2].style, ParameterStyle::NamedOnly);
        let ParameterRequirement::Defaulted(default) = parameters[1].requirement else {
            panic!("default")
        };
        assert_eq!(default.to_repr(), "(\"\", False)");
        assert!(function.return_annotation().is_none());
        let pair = TupleRef::from_value(module.get("pair").unwrap()).unwrap();
        assert_eq!(pair.content()[2].unpack_bool(), Some(true));
        let nested = inspect_def(pair.content()[0], Some(&module)).unwrap();
        let other = inspect_def(pair.content()[1], Some(&module)).unwrap();
        assert!(nested.is_global().unwrap());
        assert!(!other.is_global().unwrap());
        let first = nested.captures().next().unwrap();
        let second = other.captures().next().unwrap();
        assert_eq!(first.child_name, "later");
        assert_eq!(first.cell.identity(), second.cell.identity());
        assert!(first.contents.unwrap().ptr_eq(second.contents.unwrap()));
        assert_eq!(first.contents.unwrap().to_repr(), "[\"shared\"]");
        assert!(
            inspect_def(module.get("before").unwrap(), None)
                .unwrap()
                .is_global()
                .is_err()
        );
        assert!(
            function
                .value()
                .request_value::<&dyn StarlarkFunctionIdentity>()
                .unwrap()
                .issued_identity()
                .is_none()
        );
    }
    evaluate(
        &module,
        "gc.bzl",
        "collected = collect()",
        &Dialect::Bazel,
        &globals,
    );
    let frozen = module.freeze().unwrap();
    let pair = frozen.get("pair").unwrap();
    let values = TupleRef::from_value(pair.value()).unwrap();
    let first = inspect_def(values.content()[0], None).unwrap();
    let second = inspect_def(values.content()[1], None).unwrap();
    assert!(first.is_global().unwrap());
    assert!(!second.is_global().unwrap());
    assert_eq!(
        first.captures().next().unwrap().cell.identity(),
        second.captures().next().unwrap().cell.identity()
    );
    assert!(
        first
            .value()
            .request_value::<&dyn StarlarkFunctionIdentity>()
            .unwrap()
            .issued_identity()
            .is_none()
    );
    let consumer = Module::new();
    let result = Evaluator::new(&consumer)
        .eval_function(first.value(), &[], &[])
        .unwrap();
    assert_eq!(result.to_repr(), "[\"shared\"]");
}

#[test]
fn imported_original_module_and_inlined_helper_dependencies_survive_producer_drop() {
    let producer = Module::new();
    producer
        .set_client_data_no_overwrite(producer.heap().alloc("producer authority"))
        .unwrap();
    evaluate(
        &producer,
        "producer.bzl",
        r#"
VALUE = "initial"
VALUE = "original"
def helper(x):
    return x + "suffix"
def factory():
    captured = "cell"
    def nested(x = "default"):
        return VALUE + x + captured
    return nested
"#,
        &Dialect::Bazel,
        &Globals::standard(),
    );
    let producer = producer.freeze().unwrap();
    let consumer = Module::new();
    consumer
        .set_client_data_no_overwrite(consumer.heap().alloc("wrong consumer authority"))
        .unwrap();
    consumer.set("VALUE", consumer.heap().alloc("wrong consumer global"));
    let helper = producer
        .get("helper")
        .unwrap()
        .owned_value(consumer.frozen_heap());
    consumer.set("helper", helper);
    let factory = producer
        .get("factory")
        .unwrap()
        .owned_value(consumer.frozen_heap());
    let nested = Evaluator::new(&consumer)
        .eval_function(factory, &[], &[])
        .unwrap();
    consumer.set("nested", nested);
    drop(producer);
    {
        let inspected = inspect_def(nested, Some(&consumer)).unwrap();
        assert!(!inspected.is_global().unwrap());
        assert_eq!(inspected.source().filename(), "producer.bzl");
        assert_eq!(
            inspected
                .original_module_client_data()
                .unwrap()
                .unpack_str(),
            Some("producer authority")
        );
        let mut saw_original = false;
        inspected
            .code()
            .visit_dependencies(&mut budget(), &mut |dependency| {
                if let DefDependency::ModuleSymbol {
                    name: "VALUE",
                    value,
                    ..
                } = dependency
                {
                    saw_original = value.unwrap().unpack_str() == Some("original");
                }
                Ok(())
            })
            .unwrap();
        assert!(saw_original);
    }
    evaluate(
        &consumer,
        "consumer.bzl",
        "def callback(x):\n    return helper(x)\n",
        &Dialect::Bazel,
        &Globals::standard(),
    );
    let callback = inspect_def(consumer.get("callback").unwrap(), Some(&consumer)).unwrap();
    let mut saw_helper = false;
    callback
        .code()
        .visit_dependencies(&mut budget(), &mut |dependency| {
            if let DefDependency::Constant(value) = dependency {
                if let Some(function) = inspect_def(value, None) {
                    saw_helper |= function.name() == "helper"
                        && function.source().filename() == "producer.bzl";
                }
            }
            Ok(())
        })
        .unwrap();
    assert!(
        saw_helper,
        "compiler-inlined helper remains a real function dependency"
    );
    let frozen = consumer.freeze().unwrap();
    let nested = frozen.get("nested").unwrap();
    let inspected = inspect_def(nested.value(), None).unwrap();
    assert!(!inspected.is_global().unwrap());
    assert_eq!(
        inspected
            .original_module_client_data()
            .unwrap()
            .unpack_str(),
        Some("producer authority")
    );
    let scratch = Module::new();
    assert_eq!(
        Evaluator::new(&scratch)
            .eval_function(nested.value(), &[], &[])
            .unwrap()
            .unpack_str(),
        Some("originaldefaultcell")
    );
}

#[test]
fn exhaustive_ir_dependencies_exclude_unused_globals_and_fail_on_budgets_or_consumer_error() {
    let module = Module::new();
    evaluate(
        &module,
        "dependencies.bzl",
        r#"
USED = "first"
USED = "owned"
INDEX = 1
INDEX = 0
UNUSED = ["unvisited sentinel"]
def callback(arg: str) -> str:
    value: int = INDEX
    rows = [USED for x in [arg] if x == USED]
    mapping = {x: USED for x in [arg] if x}
    rows[INDEX] = USED
    rows += [USED]
    if arg:
        rows[INDEX] = "%s-end" % USED
    else:
        rows[INDEX] = "{}-end".format(USED)
    def child(flag: int = INDEX) -> str:
        return str(flag) + USED
    return child(flag = INDEX) + str(*rows, **mapping)
"#,
        &Dialect::Extended,
        &Globals::standard(),
    );
    let function = inspect_def(module.get("callback").unwrap(), Some(&module)).unwrap();
    assert!(function.parameters().next().unwrap().annotation.is_some());
    assert!(function.return_annotation().is_some());
    let mut symbols = Vec::new();
    let mut constants = Vec::new();
    let mut native = Vec::new();
    let mut types = 0;
    let mut nested = 0;
    function
        .code()
        .visit_dependencies(&mut budget(), &mut |dependency| {
            match dependency {
                DefDependency::ModuleSymbol {
                    name,
                    value,
                    access,
                } => {
                    assert_eq!(access, ModuleAccess::Read);
                    assert!(value.is_some());
                    symbols.push(name.to_owned());
                }
                DefDependency::Constant(value) => {
                    if let Some(value) = value.unpack_str() {
                        constants.push(value.to_owned());
                    }
                }
                DefDependency::NativeFunction(function) => native.push(function.name().to_owned()),
                DefDependency::Type(_) => types += 1,
                DefDependency::NestedCode(_) => nested += 1,
                DefDependency::Source(span) => assert_eq!(span.file.filename(), "dependencies.bzl"),
            }
            Ok(())
        })
        .unwrap();
    assert!(symbols.iter().any(|value| value == "USED"));
    assert!(symbols.iter().any(|value| value == "INDEX"));
    assert!(!symbols.iter().any(|value| value == "UNUSED"));
    assert!(!constants.iter().any(|value| value == "unvisited sentinel"));
    assert!(constants.iter().any(|value| value == "-end"));
    assert!(native.iter().any(|value| value == "str"));
    assert!(types >= 3);
    assert_eq!(nested, 1);
    for mut limit in [
        InspectionBudget {
            remaining_nodes: 0,
            maximum_depth: 100,
        },
        InspectionBudget {
            remaining_nodes: 10_000,
            maximum_depth: 0,
        },
    ] {
        let mut called = false;
        assert!(
            function
                .code()
                .visit_dependencies(&mut limit, &mut |_| {
                    called = true;
                    Ok(())
                })
                .is_err()
        );
        assert!(!called);
    }
    let mut visits = 0;
    let error = function
        .code()
        .visit_dependencies(&mut budget(), &mut |_| {
            visits += 1;
            Err(crate::Error::new_other(anyhow::anyhow!(
                "consumer refused dependency"
            )))
        })
        .unwrap_err();
    assert_eq!(visits, 1);
    assert!(error.to_string().contains("consumer refused dependency"));
    assert!(
        function
            .value()
            .request_value::<&dyn StarlarkFunctionIdentity>()
            .unwrap()
            .issued_identity()
            .is_none()
    );
}
