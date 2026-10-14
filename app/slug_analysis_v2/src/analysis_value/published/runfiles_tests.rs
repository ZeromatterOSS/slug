//! Pinned Runfiles.java isImmutable admits dictionary keys through originals.
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::RetainedRunfiles;
use slug_identity_v2::CanonicalLabel;
use starlark::environment::Globals;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::ValueLike;

use crate::analysis_value::materialize_runfiles;

#[test]
fn original_runfiles_keys_preserve_wrapper_identity_and_shared_empty_hashing() {
    let module = Module::new();
    let empty = RetainedRunfiles::empty();
    let populated = empty
        .with_artifact(AnalysisArtifact::Source(
            CanonicalLabel::parse("@@//pkg:data").unwrap(),
        ))
        .unwrap();
    for (name, value) in [
        ("empty_a", &empty),
        ("empty_b", &empty),
        ("first", &populated),
        ("second", &populated),
    ] {
        module.set(
            name,
            materialize_runfiles(value, module.frozen_heap())
                .unwrap()
                .to_value(),
        );
    }
    let result = Evaluator::new(&module).eval_module(
        AstModule::parse("original_keys.bzl", "keys = {empty_a: 'empty', first: 'first', second: 'second'}\nanswer = (len(keys), keys[empty_b], keys[first], keys[second], first == second)\n".to_owned(), &Dialect::Bazel).unwrap(),
        &Globals::standard(),
    ).unwrap();
    let _ = result;
    assert_eq!(
        module.get("answer").unwrap().to_repr(),
        r#"(3, "empty", "first", "second", False)"#
    );
}
