//! Pinned binding/outer-tuple shape; element preservation is Slug-native.
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use crate::package::loading_globals;
use crate::provider::BzlEvaluationContext;

fn execute(eval: &mut Evaluator<'_, '_, '_>, source: &str) -> anyhow::Result<()> {
    eval.eval_module(
        AstModule::parse("ordinary.bzl", source.to_owned(), &Dialect::Bazel)
            .map_err(starlark::Error::into_anyhow)?,
        &loading_globals(),
    )
    .map_err(starlark::Error::into_anyhow)?;
    Ok(())
}

fn prepared() -> Module {
    let source = Module::new();
    let context = BzlEvaluationContext::new("@@rules_cc+//cc/private:sequence_test.bzl");
    let mut eval = Evaluator::new(&source);
    eval.extra = Some(&context);
    execute(&mut eval, "i = cc_common.internal_DO_NOT_USE()\nNamed = provider(fields = ['type', 'name', 'is_whole_archive'])\nHolder = provider(fields = ['value'])\n").unwrap();
    drop(eval);
    let imported = source.freeze().unwrap();
    let module = Module::new();
    module.import_public_symbols(&imported);
    module
}

#[test]
fn sequence_captures_list_tuple_and_range_order_length_and_duplicates() {
    // CcStarlarkInternal.java:397-405 returns a tuple; RangeList.java:54
    // also implements Sequence. Imported helper needs no new caller gate.
    let module = prepared();
    let mut eval = Evaluator::new(&module);
    for (input, expected) in [
        ("[]", "[\"tuple\", 0, []]"),
        ("[2, 1, 2]", "[\"tuple\", 3, [2, 1, 2]]"),
        ("(2,)", "[\"tuple\", 1, [2]]"),
        ("(2, 1, 2)", "[\"tuple\", 3, [2, 1, 2]]"),
        ("range(4, 0, -2)", "[\"tuple\", 2, [4, 2]]"),
    ] {
        execute(
            &mut eval,
            &format!("out = i.intern_seq({input})\nresult = [type(out), len(out), list(out)]"),
        )
        .unwrap();
        assert_eq!(module.get("result").unwrap().to_repr(), expected);
    }
    execute(&mut eval, "original = [2, 1, 2]\nout = i.intern_seq(original)\noriginal[0] = 9\noriginal.append(7)\nresult = [list(out), original]\n").unwrap();
    assert_eq!(
        module.get("result").unwrap().to_repr(),
        "[[2, 1, 2], [9, 1, 2, 7]]"
    );
}

#[test]
fn sequence_preserves_immutable_provider_fields_and_nested_values() {
    let module = prepared();
    let mut eval = Evaluator::new(&module);
    execute(&mut eval, r#"
p = Named(type = 'object_file', name = 'pkg/α.pic.o', is_whole_archive = False)
nested = Holder(value = (p, (None, True, 'text', 7)))
values = [p, nested, p]
out = i.intern_seq(values)
result = [type(out), len(out), out == tuple(values), out[0].type, out[0].name, out[0].is_whole_archive, out[1].value[0] == p, out[1].value[1], out[2] == p]
"#).unwrap();
    assert_eq!(
        module.get("result").unwrap().to_repr(),
        "[\"tuple\", 3, True, \"object_file\", \"pkg/α.pic.o\", False, True, (None, True, \"text\", 7), True]"
    );
    drop(eval);
    let published = module.freeze().unwrap();
    assert_eq!(published.get("out").unwrap().value().get_type(), "tuple");
}

#[test]
fn sequence_keeps_slug_native_mixed_numeric_types_and_signed_zero_reprs() {
    // Upstream weak interning can substitute equal numeric representatives.
    // This adapter preserves original values; no representative-sharing claim.
    let module = prepared();
    let mut eval = Evaluator::new(&module);
    execute(&mut eval, "original = [1, 1.0, -0.0, 0.0]\nout = i.intern_seq(original)\nresult = [[type(value) for value in out], [repr(value) for value in out]]\n").unwrap();
    assert_eq!(
        module.get("result").unwrap().to_repr(),
        "[[\"int\", \"float\", \"float\", \"float\"], [\"1\", \"1.0\", \"-0.0\", \"0.0\"]]"
    );
    assert_eq!(
        module.get("original").unwrap().to_repr(),
        "[1, 1.0, -0.0, 0.0]"
    );
}

#[test]
fn sequence_refuses_mutable_elements_without_changing_originals() {
    let module = prepared();
    let mut eval = Evaluator::new(&module);
    for expression in [
        "[[]]",
        "[{'key': 1}]",
        "[Holder(value = [1])]",
        "[Holder(value = {'key': 1})]",
        "[(1, [2])]",
    ] {
        execute(&mut eval, &format!("original = {expression}")).unwrap();
        let before = module.get("original").unwrap().to_repr();
        let error = execute(&mut eval, "out = i.intern_seq(original)").unwrap_err();
        assert!(
            error.to_string().contains("unsupported/deferred"),
            "{error}"
        );
        assert_eq!(module.get("original").unwrap().to_repr(), before);
    }
}

#[test]
fn sequence_requires_one_positional_genuine_sequence() {
    // Param.java:78,94 default to positional=true, named=false.
    let module = prepared();
    let mut eval = Evaluator::new(&module);
    for expression in [
        "i.intern_seq()",
        "i.intern_seq([], [])",
        "i.intern_seq(seq = [])",
        "i.intern_seq('text')",
        "i.intern_seq({})",
        "i.intern_seq(set([1]))",
        "i.intern_seq(None)",
    ] {
        assert!(execute(&mut eval, expression).is_err(), "{expression}");
    }
}
