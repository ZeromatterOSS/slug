use std::convert::Infallible;

use starlark::environment::FrozenModule;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;

fn evaluate(source: &str) -> anyhow::Result<FrozenModule> {
    let globals = GlobalsBuilder::standard()
        .with(dotted_version_globals)
        .build();
    let module = Module::new();
    let ast = AstModule::parse(
        "dotted_version_test.bzl",
        source.to_owned(),
        &Dialect::Standard,
    )
    .map_err(starlark::Error::into_anyhow)?;
    Evaluator::new(&module)
        .eval_module(ast, &globals)
        .map_err(starlark::Error::into_anyhow)?;
    Ok(module.freeze()?)
}

#[test]
fn native_constructor_preserves_general_spelling_and_exact_reflection() {
    evaluate(
        r#"
def check():
    v = dotted_version('00012ab34cd56.00.internal..☃')
    if type(v) != 'DottedVersion' or not v:
        fail('type/truth')
    if str(v) != '00012ab34cd56.00.internal..☃' or repr(v) != str(v) or v.to_string != str(v):
        fail('spelling')
    if dir(v) != ['compare_to', 'to_string']:
        fail('reflection: %s' % dir(v))
    if not dotted_version('0') or hasattr(v, 'numComponents') or hasattr(v, 'components'):
        fail('extra Java-only surface')
check()
"#,
    )
    .unwrap();
    assert!(evaluate("X = dotted_version('1').to_string()").is_err());
    for source in [
        "X = len(dotted_version('1'))",
        "X = dotted_version('1')[0]",
        "X = dotted_version('1') + dotted_version('2')",
    ] {
        assert!(evaluate(source).is_err(), "{source}");
    }
}

#[test]
fn normalized_dictionary_equality_keeps_first_key_spelling() {
    evaluate(
        r#"
def check():
    a = dotted_version('01.00')
    b = dotted_version('1.internal.☃')
    c = dotted_version('1')
    lookup = {a: 'first'}
    lookup[b] = 'second'
    if len(lookup) != 1 or lookup[c] != 'second' or lookup.keys()[0].to_string != '01.00':
        fail('equal-key replacement or original spelling')
    if a != b or b != c or a == '1' or a == 1 or a == None:
        fail('language equality')
    if (a,) != (c,):
        fail('nested equality')
    return lookup
FROZEN = check()
"#,
    )
    .unwrap();
}

#[test]
fn native_comparison_matches_normalized_order_and_sign() {
    evaluate(
        r#"
spellings = ['0', '0.1', '1a', '1a1', '1a9', '1a10', '1b', '1', '1.1', '2']
values = [dotted_version(v) for v in spellings]
def check():
    for i in range(len(values)):
        for j in range(len(values)):
            expected = -1 if i < j else (1 if i > j else 0)
            if values[i].compare_to(values[j]) != expected:
                fail('compare_to')
            if (values[i] < values[j]) != (i < j) or (values[i] <= values[j]) != (i <= j):
                fail('ordered comparison')
            if (values[i] > values[j]) != (i > j) or (values[i] >= values[j]) != (i >= j):
                fail('reverse ordered comparison')
    if dotted_version('1A').compare_to(dotted_version('1a')) != -1:
        fail('case-sensitive sign')
    if dotted_version('1ab').compare_to(dotted_version('1abcd')) != -1:
        fail('string magnitude escaped')
    if dotted_version('1').compare_to(dotted_version('01.00')) != 0:
        fail('normalized zero')
check()
"#,
    )
    .unwrap();
    assert!(evaluate("X = dotted_version('1') < '2'").is_err());
    assert!(evaluate("X = '2' < dotted_version('1')").is_err());
}

#[test]
fn genuine_constructor_binder_and_invalid_input_are_distinct() {
    for source in [
        "X = dotted_version()",
        "X = dotted_version('1', '2')",
        "X = dotted_version(version = '1')",
        "X = dotted_version(other = '1')",
        "X = dotted_version(None)",
        "X = dotted_version(1)",
    ] {
        assert!(evaluate(source).is_err(), "{source}");
    }
    for input in ["", "1.", "1..2", "1.α", "2147483648", "1a2147483648"] {
        let source = format!("X = dotted_version({input:?})");
        let error = evaluate(&source).err().unwrap().to_string();
        assert!(error.contains(&format!(
            "Dotted version components must all start with the form \\d+([a-z0-9]*?)?(\\d+)? but got '{input}'"
        )), "{error}");
        assert!(!error.contains("budget exceeded"));
    }
    evaluate("X = dotted_version('1a2147483648b')\nY = dotted_version('1.internal..☃')").unwrap();
}

#[test]
fn genuine_compare_method_has_positional_only_typed_binding() {
    for suffix in [
        "()",
        "('1')",
        "(None)",
        "(1)",
        "(other = dotted_version('1'))",
        "(dotted_version('1'), dotted_version('2'))",
    ] {
        let source = format!("X = dotted_version('1').compare_to{suffix}");
        assert!(evaluate(&source).is_err(), "{source}");
    }
    evaluate("method = dotted_version('01.00').compare_to\nX = method(dotted_version('1'))\ndef check():\n    if X != 0: fail('bound receiver')\ncheck()").unwrap();
}

#[test]
fn frozen_allocation_and_borrowed_payload_preserve_full_native_state() {
    let module = evaluate("V = dotted_version('01.00.internal.☃')").unwrap();
    let value = module.get("V").unwrap();
    let model = dotted_version_ref(value.value()).unwrap();
    assert_eq!(model.spelling(), "01.00.internal.☃");
    assert_eq!(model.original_component_count(), 2);
    assert!(dotted_version_ref(Value::new_none()).is_none());
    let heap = FrozenHeap::new();
    let copy = alloc_frozen_dotted_version(&heap, model.clone());
    drop(value);
    drop(module);
    let retained = dotted_version_ref(copy.to_value()).unwrap();
    assert_eq!(retained.spelling(), "01.00.internal.☃");
    let ordinary = DottedVersion::prepare("1", || Ok::<_, Infallible>(()))
        .unwrap()
        .into_owned();
    assert_ne!(
        retained, &ordinary,
        "publication payload preserves original spelling"
    );
    assert_eq!(retained.normalized_cmp(&ordinary), Ordering::Equal);
    assert_eq!(copy.to_value().to_repr(), "01.00.internal.☃");
}

#[test]
fn construction_caps_admit_exact_peak_and_refuse_before_owned_copy() {
    let input = "1.2a3.00.internal.☃";
    let plan = DottedVersion::prepare(input, || Ok::<_, Infallible>(())).unwrap();
    let peak = plan.storage().construction_peak_bytes()
        + size_of::<DottedVersionValue>()
        + 4 * size_of::<usize>();
    let mut exact = ConstructionBudget {
        records: 0,
        bytes: CALLABLE_GRAPH_BYTE_LIMIT - input.len() - peak,
    };
    assert_eq!(construct(input, &mut exact).unwrap().0.spelling(), input);
    assert_eq!(exact.bytes, CALLABLE_GRAPH_BYTE_LIMIT);
    let mut short = ConstructionBudget {
        records: 0,
        bytes: CALLABLE_GRAPH_BYTE_LIMIT - input.len() - peak + 1,
    };
    let before = short.bytes;
    assert!(
        construct(input, &mut short)
            .unwrap_err()
            .to_string()
            .contains("byte budget exceeded")
    );
    assert_eq!(
        short.bytes,
        before + input.len(),
        "failed peak admission did not change paid bytes"
    );
    let mut no_input = ConstructionBudget {
        records: 0,
        bytes: CALLABLE_GRAPH_BYTE_LIMIT,
    };
    assert!(
        construct("☃", &mut no_input)
            .unwrap_err()
            .to_string()
            .contains("byte budget exceeded")
    );
    assert_eq!(
        no_input.records, 0,
        "input is admitted before parser inspection"
    );
    let mut no_work = ConstructionBudget {
        records: CALLABLE_GRAPH_NODE_LIMIT,
        bytes: 0,
    };
    assert!(
        construct("☃", &mut no_work)
            .unwrap_err()
            .to_string()
            .contains("node budget exceeded")
    );
    assert_eq!(no_work.records, CALLABLE_GRAPH_NODE_LIMIT);
    assert_eq!(no_work.bytes, "☃".len());
    assert!(ConstructionBudget::default().bytes(usize::MAX).is_err());
    assert!(ConstructionBudget::default().records(usize::MAX).is_err());
}
