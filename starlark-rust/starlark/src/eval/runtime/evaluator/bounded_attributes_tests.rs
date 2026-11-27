use std::cell::Cell;
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use starlark_derive::starlark_value;

use crate as starlark;
use crate::collections::Hashed;
use crate::collections::symbol::symbol::Symbol;
use crate::environment::Globals;
use crate::environment::Methods;
use crate::environment::MethodsBuilder;
use crate::environment::MethodsStatic;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::eval::bc::call::BcCallArgsFull;
use crate::eval::bc::frame::alloca_frame;
use crate::eval::bc::instr_impl::InstrCallMaybeKnownMethod;
use crate::eval::bc::instr_impl::InstrConst;
use crate::eval::bc::instr_impl::InstrReturn;
use crate::eval::bc::stack_ptr::BcSlotInRange;
use crate::eval::bc::writer::BcWriter;
use crate::eval::compiler::def::FrozenDef;
use crate::eval::runtime::frame_span::FrameSpan;
use crate::starlark_module;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::FrozenRef;
use crate::values::Heap;
use crate::values::NoSerialize;
use crate::values::ProvidesStaticType;
use crate::values::StarlarkValue;
use crate::values::Value;
use crate::values::ValueLike;
use crate::values::types::known_methods::get_known_method;

#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    derive_more::Display
)]
#[display("modeled_fields")]
struct ModeledFields {
    reject_legacy: bool,
}

starlark::starlark_simple_value!(ModeledFields);

#[starlark_value(type = "modeled_fields")]
impl<'v> StarlarkValue<'v> for ModeledFields {
    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        assert!(
            !self.reject_legacy,
            "called a modeled field's legacy getter"
        );
        (attribute == "present").then(|| Value::testing_new_int(17))
    }

    fn get_attr_bounded(
        &self,
        attribute: &str,
        _heap: Heap<'v>,
        budget: &ExecutionBudget,
    ) -> crate::Result<Option<Value<'v>>> {
        budget.charge(1, 0)?;
        Ok((attribute == "present").then(|| Value::testing_new_int(17)))
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        assert!(!self.reject_legacy, "called a modeled field's legacy query");
        attribute == "present"
    }

    fn dir_attr(&self) -> Vec<String> {
        assert!(!self.reject_legacy, "enumerated modeled fields");
        vec!["present".to_owned()]
    }
}

#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    derive_more::Display
)]
#[display("legacy_field")]
struct LegacyField {
    reject_legacy: bool,
}

starlark::starlark_simple_value!(LegacyField);

#[starlark_value(type = "legacy_field")]
impl<'v> StarlarkValue<'v> for LegacyField {
    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        assert!(!self.reject_legacy, "called an unadmitted plain getter");
        (attribute == "present").then(|| Value::testing_new_int(19))
    }
}

#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    derive_more::Display
)]
#[display("legacy_hashed")]
struct LegacyHashed;

starlark::starlark_simple_value!(LegacyHashed);

#[starlark_value(type = "legacy_hashed")]
impl<'v> StarlarkValue<'v> for LegacyHashed {
    fn get_attr_hashed(&self, _attribute: Hashed<&str>, _heap: Heap<'v>) -> Option<Value<'v>> {
        panic!("called an unadmitted hashed getter");
    }
}

#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    derive_more::Display
)]
#[display("legacy_query")]
struct LegacyQuery;

starlark::starlark_simple_value!(LegacyQuery);

#[starlark_value(type = "legacy_query")]
impl<'v> StarlarkValue<'v> for LegacyQuery {
    fn has_attr(&self, _attribute: &str, _heap: Heap<'v>) -> bool {
        panic!("called an unadmitted attribute query");
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct ReprCanary;

starlark::starlark_simple_value!(ReprCanary);

impl fmt::Display for ReprCanary {
    fn fmt(&self, _writer: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic!("displayed a malformed attribute name");
    }
}

#[starlark_value(type = "repr_canary")]
impl<'v> StarlarkValue<'v> for ReprCanary {
    fn collect_repr(&self, _output: &mut String) {
        panic!("rendered a malformed attribute name");
    }
}

#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    derive_more::Display
)]
#[display("legacy_registry")]
struct LegacyRegistry;

starlark::starlark_simple_value!(LegacyRegistry);

thread_local! {
    static REGISTRY_LOOKUPS: Cell<usize> = const { Cell::new(0) };
}

#[starlark_value(type = "legacy_registry")]
impl<'v> StarlarkValue<'v> for LegacyRegistry {
    fn get_methods() -> Option<&'static Methods> {
        REGISTRY_LOOKUPS.with(|lookups| lookups.set(lookups.get() + 1));
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(legacy_members)
    }

    fn get_attr_bounded(
        &self,
        _name: &str,
        _heap: Heap<'v>,
        _budget: &ExecutionBudget,
    ) -> crate::Result<Option<Value<'v>>> {
        panic!("field projection ran before the native registry was admitted");
    }
}

#[starlark_module]
fn legacy_members(builder: &mut MethodsBuilder) {
    #[starlark(attribute)]
    fn property(this: Value) -> anyhow::Result<i32> {
        let _ = this;
        panic!("called an unadmitted NativeAttribute");
    }

    fn append(this: Value, _value: i32) -> anyhow::Result<i32> {
        let _ = this;
        panic!("called an unadmitted native method");
    }
}

fn eval<'v>(module: &'v Module, source: &str) -> crate::Result<Value<'v>> {
    let ast = AstModule::parse("attributes.star", source.to_owned(), &Dialect::Standard).unwrap();
    Evaluator::new(module).eval_module(ast, &Globals::standard())
}

fn strict_budget() -> Arc<ExecutionBudget> {
    Arc::new(ExecutionBudget::new(50_000, 1 << 20, 16).with_bounded_operations())
}

fn assert_refusal(
    error: crate::Error,
    budget: &ExecutionBudget,
    operation: &'static str,
    value_type: &'static str,
) {
    let expected = ExecutionLimit::UnsupportedOperation {
        operation,
        value_type,
    };
    assert!(
        matches!(
            error.kind(),
            crate::ErrorKind::Other(cause) if cause.downcast_ref::<ExecutionLimit>() == Some(&expected)
        ),
        "{error}"
    );
    assert_eq!(budget.check(), Err(expected));
}

#[test]
fn strict_attributes_project_modeled_fields_and_distinguish_missing() {
    let budget = strict_budget();
    let module = Module::new_with_execution_budget(budget.clone());
    let subject = module.heap().alloc(ModeledFields {
        reject_legacy: true,
    });
    module.set("subject", subject);
    let present = subject.get_attr("present", module.heap()).unwrap().unwrap();
    assert_eq!(present.unpack_i32(), Some(17));
    assert!(subject.get_attr("presen", module.heap()).unwrap().is_none());
    assert_eq!(
        eval(&module, "subject.present").unwrap().unpack_i32(),
        Some(17)
    );
    assert_eq!(
        eval(&module, "getattr(subject, 'present', 23)")
            .unwrap()
            .unpack_i32(),
        Some(17)
    );
    assert_eq!(
        eval(&module, "getattr(subject, 'presen', 23)")
            .unwrap()
            .unpack_i32(),
        Some(23)
    );
    assert_eq!(
        eval(&module, "hasattr(subject, 'present')")
            .unwrap()
            .unpack_bool(),
        Some(true)
    );
    assert_eq!(
        eval(&module, "hasattr(subject, 'presen')")
            .unwrap()
            .unpack_bool(),
        Some(false)
    );

    let ordinary = Module::new();
    ordinary.set(
        "subject",
        ordinary.heap().alloc(ModeledFields {
            reject_legacy: false,
        }),
    );
    let ordinary_dot = eval(&ordinary, "subject.presen").unwrap_err();
    let limited_dot = eval(&module, "subject.presen").unwrap_err();
    assert!(matches!(limited_dot.kind(), crate::ErrorKind::Value(_)));
    assert_eq!(
        limited_dot.without_diagnostic().to_string(),
        "Object of type `modeled_fields` has no attribute `presen`"
    );
    assert!(
        ordinary_dot
            .without_diagnostic()
            .to_string()
            .contains("did you mean `present`?")
    );

    let source = "getattr(subject, 'presen')";
    let ordinary_getattr = eval(&ordinary, source).unwrap_err();
    let limited_getattr = eval(&module, source).unwrap_err();
    assert!(matches!(limited_getattr.kind(), crate::ErrorKind::Value(_)));
    assert_eq!(
        limited_getattr.without_diagnostic().to_string(),
        ordinary_getattr.without_diagnostic().to_string()
    );
    assert_eq!(budget.check(), Ok(()));
    assert_refusal(
        eval(&module, "dir(subject)").unwrap_err(),
        &budget,
        "attribute enumeration",
        "modeled_fields",
    );
}

#[test]
fn strict_attributes_reject_unmodeled_getters_and_query_without_calling_them() {
    for (source, value_type) in [
        ("getattr(subject, 'unknown', 23)", "legacy_field"),
        ("subject.present", "legacy_hashed"),
        ("hasattr(subject, 'present')", "legacy_query"),
    ] {
        let budget = strict_budget();
        let module = Module::new_with_execution_budget(budget.clone());
        let heap = module.heap();
        let subject = match value_type {
            "legacy_field" => heap.alloc(LegacyField {
                reject_legacy: true,
            }),
            "legacy_hashed" => heap.alloc(LegacyHashed),
            _ => heap.alloc(LegacyQuery),
        };
        module.set("subject", subject);
        assert_refusal(
            eval(&module, source).unwrap_err(),
            &budget,
            "attribute access",
            value_type,
        );
    }

    let budget = strict_budget();
    let module = Module::new_with_execution_budget(budget.clone());
    let heap = module.heap();
    let subject = heap.alloc(LegacyHashed);
    assert_refusal(
        subject.get_attr("present", heap).unwrap_err(),
        &budget,
        "attribute access",
        "legacy_hashed",
    );

    let budget = Arc::new(ExecutionBudget::new(50_000, 1 << 20, 16));
    let module = Module::new_with_execution_budget(budget.clone());
    let subject = module.heap().alloc(LegacyField {
        reject_legacy: false,
    });
    module.set("subject", subject);
    assert_eq!(
        subject
            .get_attr("present", module.heap())
            .unwrap()
            .unwrap()
            .unpack_i32(),
        Some(19)
    );
    assert_eq!(
        eval(&module, "subject.present").unwrap().unpack_i32(),
        Some(19)
    );
    assert_eq!(
        eval(&module, "getattr(subject, 'missing', 23)")
            .unwrap()
            .unpack_i32(),
        Some(23)
    );
    assert_eq!(budget.check(), Ok(()));
}

#[test]
fn strict_attributes_reject_legacy_members_through_live_and_frozen_dot() {
    let ordinary = Module::new();
    eval(
        &ordinary,
        "def bind(subject): return subject.property\ndef invoke(subject): return subject.append(1)",
    )
    .unwrap();
    let ordinary = ordinary.freeze().unwrap();
    for (function, source) in [
        ("bind", "subject.property"),
        ("invoke", "subject.append(1)"),
    ] {
        let program = ordinary.get(function).unwrap();
        let bytecode = program
            .value()
            .downcast_ref::<FrozenDef>()
            .unwrap()
            .dump_debug();
        assert!(bytecode.contains("ObjectField"), "{bytecode}");
        for use_frozen in [false, true] {
            let budget = strict_budget();
            let module = Module::new_with_execution_budget(budget.clone());
            let subject = module.heap().alloc(LegacyRegistry);
            let before = REGISTRY_LOOKUPS.with(Cell::get);
            let result = if use_frozen {
                let program = program.owned_value(module.frozen_heap());
                Evaluator::new(&module).eval_function(program, &[subject], &[])
            } else {
                module.set("subject", subject);
                eval(&module, source)
            };
            assert_refusal(
                result.unwrap_err(),
                &budget,
                "attribute access",
                "legacy_registry",
            );
            assert_eq!(REGISTRY_LOOKUPS.with(Cell::get), before);
        }
    }
}

#[test]
fn strict_attributes_check_malformed_names_without_rendering_values() {
    let budget = strict_budget();
    let module = Module::new_with_execution_budget(budget.clone());
    module.set("malformed", module.heap().alloc(ReprCanary));
    for source in ["getattr(None, malformed)", "hasattr(None, malformed)"] {
        let error = eval(&module, source).unwrap_err();
        assert!(
            matches!(error.kind(), crate::ErrorKind::Value(_)),
            "{error}"
        );
        assert!(
            error
                .without_diagnostic()
                .to_string()
                .contains("expected `string`, actual `repr_canary`")
        );
        assert_eq!(budget.check(), Ok(()));
    }
}

#[test]
fn strict_attributes_reject_unknown_registry_in_maybe_known_method_opcode() {
    let budget = strict_budget();
    let module = Module::new_with_execution_budget(budget.clone());
    let frozen = module.frozen_heap();
    let receiver = frozen.alloc(LegacyRegistry);
    let known = get_known_method("append").unwrap();
    let span = FrameSpan::default();
    let mut writer = BcWriter::new(FrozenRef::default(), 0, frozen);
    writer.alloc_slots_c::<3, _>(|slots, writer| {
        let this = slots.get::<0>();
        let argument = slots.get::<1>();
        let target = slots.get::<2>();
        writer.write_instr::<InstrConst>(span, (receiver, this.to_out()));
        writer.write_instr::<InstrConst>(span, (frozen.alloc(1i32), argument.to_out()));
        // Ordinary source often folds a constant's method into a bound value.
        // Emit the real interpreter opcode to exercise its registry fallback.
        let args = BcCallArgsFull {
            pos_named: BcSlotInRange {
                start: argument.to_in(),
                end: target.to_in(),
            },
            names: Box::new([]),
            args: None,
            kwargs: None,
        };
        let call_site = writer.alloc_file_span(span);
        writer.write_instr::<InstrCallMaybeKnownMethod>(
            span,
            (
                this.to_in(),
                Symbol::new("append"),
                known,
                args,
                call_site,
                target.to_out(),
            ),
        );
        writer.write_instr::<InstrReturn>(span, target.to_in());
    });
    let bytecode = writer.finish();
    let mut evaluator = Evaluator::new(&module);
    evaluator
        .call_stack
        .alloc_if_needed(4, Some(&budget))
        .unwrap();
    evaluator.call_stack.push(Value::new_none(), None).unwrap();
    let before = REGISTRY_LOOKUPS.with(Cell::get);
    let result = alloca_frame(
        &mut evaluator,
        bytecode.local_count,
        bytecode.max_stack_size,
        bytecode.max_loop_depth,
        |eval| eval.eval_bc(Value::new_none(), &bytecode),
    )
    .unwrap();
    evaluator.call_stack.pop();
    assert_refusal(
        result.unwrap_err().into_error(),
        &budget,
        "attribute access",
        "legacy_registry",
    );
    assert_eq!(REGISTRY_LOOKUPS.with(Cell::get), before);
}
