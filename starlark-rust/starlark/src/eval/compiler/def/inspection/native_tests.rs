use super::super::ParameterRequirement;
use super::super::ParameterStyle;
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
use crate::values::function::NativeFunc;
use crate::values::function::NativeFuncFn;
use crate::values::function::NativeFunction;
use crate::values::function::SpecialBuiltinFunction;

fn first<'v>(
    _: &mut Evaluator<'v, '_, '_>,
    _: &ParametersSpec<FrozenValue>,
    _: &Arguments<'v, '_>,
) -> crate::Result<Value<'v>> {
    Ok(Value::new_bool(true))
}
fn second<'v>(
    _: &mut Evaluator<'v, '_, '_>,
    _: &ParametersSpec<FrozenValue>,
    _: &Arguments<'v, '_>,
) -> crate::Result<Value<'v>> {
    Ok(Value::new_bool(false))
}

fn parameters(function_name: &str, default: FrozenValue) -> ParametersSpec<FrozenValue> {
    ParametersSpec::new_parts(
        function_name,
        [("positional", ParametersSpecParam::Required)],
        [("defaulted", ParametersSpecParam::Defaulted(default))],
        true,
        [("named", ParametersSpecParam::Optional)],
        true,
    )
}

fn wrapper(implementation: NativeFuncFn, default: FrozenValue) -> NativeFunction {
    NativeFunction {
        function: NativeFunc(implementation, parameters("canonical", default)),
        name: "canonical".to_owned(),
        as_type: None,
        ty: Ty::any(),
        speculative_exec_safe: false,
        docs: DocItem::Member(DocMember::Function(DocFunction::default())),
        special_builtin_function: None,
    }
}

#[test]
fn canonical_native_matcher_rejects_same_name_spoof_and_every_changed_metadata_field() {
    let heap = FrozenHeap::new();
    let default = heap.alloc("default");
    let canonical =
        inspect_native_function(heap.alloc(wrapper(first, default)).to_value()).unwrap();
    let equivalent =
        inspect_native_function(heap.alloc(wrapper(first, default)).to_value()).unwrap();
    assert!(!canonical.value().ptr_eq(equivalent.value()));
    assert!(canonical.matches_canonical_wrapper(equivalent));
    assert!(equivalent.matches_canonical_wrapper(canonical));
    let spoof = inspect_native_function(heap.alloc(wrapper(second, default)).to_value()).unwrap();
    assert_eq!(canonical.name(), spoof.name());
    assert!(!canonical.matches_canonical_wrapper(spoof));
    let scratch = Module::new();
    assert_eq!(
        Evaluator::new(&scratch)
            .eval_function(canonical.value(), &[], &[])
            .unwrap()
            .unpack_bool(),
        Some(true)
    );
    assert_eq!(
        Evaluator::new(&scratch)
            .eval_function(spoof.value(), &[], &[])
            .unwrap()
            .unpack_bool(),
        Some(false)
    );
    for change in 0..7 {
        let mut changed = wrapper(first, default);
        match change {
            0 => changed.name = "different name".to_owned(),
            1 => changed.as_type = Some(Ty::string()),
            2 => changed.ty = Ty::int(),
            3 => changed.special_builtin_function = Some(SpecialBuiltinFunction::List),
            4 => changed.speculative_exec_safe = true,
            5 => {
                changed.docs = DocItem::Member(DocMember::Function(DocFunction {
                    docs: Some(DocString {
                        summary: "changed documentation".to_owned(),
                        ..DocString::default()
                    }),
                    ..DocFunction::default()
                }))
            }
            6 => changed.function.1 = parameters("different diagnostic name", default),
            _ => unreachable!(),
        }
        let changed = inspect_native_function(heap.alloc(changed).to_value()).unwrap();
        assert!(
            !canonical.matches_canonical_wrapper(changed),
            "metadata change {change}"
        );
        assert!(
            !changed.matches_canonical_wrapper(canonical),
            "metadata change {change}"
        );
    }
    let mut list = wrapper(first, default);
    list.special_builtin_function = Some(SpecialBuiltinFunction::List);
    let list = inspect_native_function(heap.alloc(list).to_value()).unwrap();
    let mut dict = wrapper(first, default);
    dict.special_builtin_function = Some(SpecialBuiltinFunction::Dict);
    let dict = inspect_native_function(heap.alloc(dict).to_value()).unwrap();
    assert!(!list.matches_canonical_wrapper(dict));
}

#[test]
fn native_parameter_inspection_preserves_actual_layout_and_borrowed_default_edges() {
    let heap = FrozenHeap::new();
    let canonical_default = heap.alloc("canonical default");
    let actual_default = heap.alloc("actual default");
    let canonical =
        inspect_native_function(heap.alloc(wrapper(first, canonical_default)).to_value()).unwrap();
    let actual =
        inspect_native_function(heap.alloc(wrapper(first, actual_default)).to_value()).unwrap();
    // Matching executable wrapper metadata never substitutes canonical default data.
    // The native publication graph must retain these distinct actual edges.
    assert!(actual.matches_canonical_wrapper(canonical));
    let actual_rows = actual.parameters().collect::<Vec<_>>();
    assert_eq!(actual.parameter_count(), 5);
    assert_eq!(
        actual_rows.iter().map(|row| row.style).collect::<Vec<_>>(),
        [
            ParameterStyle::PositionalOnly,
            ParameterStyle::PositionalOrNamed,
            ParameterStyle::Args,
            ParameterStyle::NamedOnly,
            ParameterStyle::Kwargs,
        ]
    );
    assert_eq!(actual_rows[0].name, "positional");
    assert_eq!(actual_rows[1].name, "defaulted");
    assert_eq!(actual_rows[3].name, "named");
    assert!(actual_rows.iter().all(|row| row.annotation.is_none()));
    assert!(matches!(
        actual_rows[0].requirement,
        ParameterRequirement::Required
    ));
    assert!(matches!(
        actual_rows[2].requirement,
        ParameterRequirement::Variadic
    ));
    assert!(matches!(
        actual_rows[3].requirement,
        ParameterRequirement::Optional
    ));
    assert!(matches!(
        actual_rows[4].requirement,
        ParameterRequirement::Variadic
    ));
    let ParameterRequirement::Defaulted(actual_edge) = actual_rows[1].requirement else {
        panic!("actual default")
    };
    let ParameterRequirement::Defaulted(canonical_edge) =
        canonical.parameters().nth(1).unwrap().requirement
    else {
        panic!("canonical default")
    };
    assert!(actual_edge.ptr_eq(actual_default.to_value()));
    assert!(canonical_edge.ptr_eq(canonical_default.to_value()));
    assert_eq!(actual_edge.unpack_str(), Some("actual default"));
    assert_eq!(canonical_edge.unpack_str(), Some("canonical default"));
    assert!(!actual_edge.ptr_eq(canonical_edge));
    let mut different_layout = wrapper(first, actual_default);
    different_layout.function.1 = ParametersSpec::new_named_only(
        "canonical",
        [("different", ParametersSpecParam::Defaulted(actual_default))],
    );
    let different_layout =
        inspect_native_function(heap.alloc(different_layout).to_value()).unwrap();
    assert!(different_layout.matches_canonical_wrapper(canonical));
    assert_eq!(different_layout.parameter_count(), 1);
    let row = different_layout.parameters().next().unwrap();
    assert_eq!(row.name, "different");
    assert_eq!(row.style, ParameterStyle::NamedOnly);
    let ParameterRequirement::Defaulted(edge) = row.requirement else {
        panic!("changed layout default")
    };
    assert!(edge.ptr_eq(actual_default.to_value()));
}
