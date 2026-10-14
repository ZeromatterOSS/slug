use super::*;
use crate::docs::DocFunction;
use crate::docs::DocItem;
use crate::docs::DocMember;
use crate::docs::DocString;
use crate::environment::Module;
use crate::eval::Arguments;
use crate::eval::Evaluator;
use crate::eval::ParametersSpec;
use crate::eval::ParametersSpecParam;
use crate::typing::Ty;
use crate::values::FrozenHeap;
use crate::values::FrozenValue;
use crate::values::function::FrozenBoundMethod;
use crate::values::function::NativeMeth;
use crate::values::list::AllocList;

fn copy_method(method: &NativeMethod) -> NativeMethod {
    NativeMethod {
        function: NativeMeth(method.function.0, method.function.1.clone()),
        name: method.name.clone(),
        ty: method.ty.clone(),
        speculative_exec_safe: method.speculative_exec_safe,
        docs: method.docs.clone(),
    }
}
fn bind(heap: &FrozenHeap, receiver: FrozenValue, method: NativeMethod) -> FrozenValue {
    heap.alloc(FrozenBoundMethod::new(
        receiver,
        heap.alloc_simple_typed(method),
    ))
}
fn counterfeit<'v>(
    _: &mut Evaluator<'v, '_, '_>,
    _: Value<'v>,
    _: &ParametersSpec<FrozenValue>,
    _: &Arguments<'v, '_>,
) -> crate::Result<Value<'v>> {
    Ok(Value::new_bool(false))
}

#[test]
fn bound_native_method_inspection_checks_every_fixed_wrapper_field() {
    let module = Module::new();
    let heap = FrozenHeap::new();
    let receiver = heap.alloc("-");
    let actual = receiver
        .to_value()
        .get_attr("join", module.heap())
        .unwrap()
        .unwrap();
    let inspected = inspect_native_bound_method(actual).unwrap();
    assert!(inspected.matches_receiver_wrapper());
    assert_eq!(inspected.receiver().unpack_str(), Some("-"));
    assert_eq!(inspected.name(), "join");
    assert_eq!(inspected.parameter_count(), inspected.parameters().len());
    let arguments = [module.heap().alloc(AllocList(["x", "y"]))];
    assert_eq!(
        Evaluator::new(&module)
            .eval_function(inspected.value(), &arguments, &[])
            .unwrap()
            .unpack_str(),
        Some("x-y")
    );
    let canonical = inspected.method;
    for change in 0..6 {
        let mut changed = copy_method(canonical);
        match change {
            0 => changed.function.0 = counterfeit,
            1 => changed.name = "not_a_registered_method".into(),
            2 => changed.ty = Ty::int(),
            3 => changed.speculative_exec_safe = !changed.speculative_exec_safe,
            4 => {
                changed.docs = DocItem::Member(DocMember::Function(DocFunction {
                    docs: Some(DocString {
                        summary: "changed".into(),
                        ..DocString::default()
                    }),
                    ..DocFunction::default()
                }))
            }
            5 => {
                changed.function.1 = ParametersSpec::new_parts(
                    "changed diagnostic",
                    [("value", ParametersSpecParam::Required)],
                    [],
                    false,
                    [],
                    false,
                )
            }
            _ => unreachable!(),
        }
        let wrapper = bind(&heap, receiver, changed);
        assert!(
            !inspect_native_bound_method(wrapper.to_value())
                .unwrap()
                .matches_receiver_wrapper(),
            "metadata field {change}"
        );
    }
    // A real wrapper attached to the wrong receiver's table is not certified.
    let wrong_receiver = heap.alloc(42i32);
    let wrong = bind(&heap, wrong_receiver, copy_method(canonical));
    assert!(
        !inspect_native_bound_method(wrong.to_value())
            .unwrap()
            .matches_receiver_wrapper()
    );
    assert!(inspect_native_bound_method(receiver.to_value()).is_none());
}

#[test]
fn bound_native_method_inspection_retains_actual_defaults_layout_and_frozen_receiver() {
    let module = Module::new();
    let heap = FrozenHeap::new();
    let receiver = heap.alloc("-");
    let value = receiver
        .to_value()
        .get_attr("join", module.heap())
        .unwrap()
        .unwrap();
    let actual = inspect_native_bound_method(value).unwrap();
    let mut changed = copy_method(actual.method);
    let default = heap.alloc("actual default");
    changed.function.1 = ParametersSpec::new_parts(
        actual.method.function.1.inspection_function_name(),
        [("first", ParametersSpecParam::Required)],
        [("defaulted", ParametersSpecParam::Defaulted(default))],
        true,
        [("named", ParametersSpecParam::Optional)],
        true,
    );
    let bound = bind(&heap, receiver, changed);
    let inspected = inspect_native_bound_method(bound.to_value()).unwrap();
    assert!(
        inspected.matches_receiver_wrapper(),
        "actual layout/defaults are independent semantic graph inputs"
    );
    let parameters = inspected.parameters().collect::<Vec<_>>();
    assert_eq!(parameters.len(), 5);
    assert_eq!(
        parameters[0].style,
        super::super::ParameterStyle::PositionalOnly
    );
    assert_eq!(parameters[1].name, "defaulted");
    let super::super::ParameterRequirement::Defaulted(value) = parameters[1].requirement else {
        panic!("actual default")
    };
    assert_eq!(value.unpack_str(), Some("actual default"));
    assert_eq!(parameters[3].style, super::super::ParameterStyle::NamedOnly);
    assert_eq!(inspected.receiver().unpack_str(), Some("-"));
    module.frozen_heap().add_reference(&heap.into_ref());
    module.set("bound", bound.to_value());
    let frozen = module.freeze().unwrap();
    let owned = frozen.get("bound").unwrap();
    let after = inspect_native_bound_method(owned.value()).unwrap();
    assert_eq!(after.receiver().unpack_str(), Some("-"));
    assert_eq!(after.parameters().len(), parameters.len());
    assert!(after.matches_receiver_wrapper());
}
