//! General registration globality; no caller/source/callsite authorization.
use starlark::environment::FrozenModule;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::eval::FileLoader;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;
use crate::package::loading_globals;
fn fill(module: &Module, source: &str) {
    Evaluator::new(module)
        .eval_module(
            AstModule::parse("unrelated.bzl", source.to_owned(), &Dialect::Bazel).unwrap(),
            &loading_globals(),
        )
        .unwrap();
}
fn frozen(source: &str) -> FrozenModule {
    let m = Module::new();
    fill(&m, source);
    m.freeze().unwrap()
}
#[test]
fn resource_globality_uses_actual_mutable_original_own_name_and_never_source_profile() {
    let m = Module::new();
    fill(&m, "def ordinary(os, count): return {}\nsaved = ordinary\n");
    let cb = m.get("saved").unwrap();
    validate_resource_set(cb, Some(&m)).unwrap();
    assert!(validate_resource_set(cb, None).is_err());
    // Aliases do not replace the defining function's own-name requirement.
    fill(&m, "ordinary = 0\n");
    assert!(validate_resource_set(cb, Some(&m)).is_err());
    m.set("ordinary", cb);
    validate_resource_set(cb, Some(&m)).unwrap();
    assert!(validate_resource_set(m.heap().alloc(4), Some(&m)).is_err());
}
#[test]
fn resource_frozen_original_wins_over_foreign_consumer_and_keeps_loaded_capture() {
    struct Loader(FrozenModule);
    impl FileLoader for Loader {
        fn load(&self, _: &str) -> starlark::Result<FrozenModule> {
            Ok(self.0.clone())
        }
    }
    let input = frozen("DATA = [3]\n");
    let m = Module::new();
    let loader = Loader(input);
    {
        let mut e = Evaluator::new(&m);
        e.set_loader(&loader);
        e.eval_module(AstModule::parse("relocated.bzl","load('arbitrary', 'DATA')\ndef resources(os, count): return {'cpu': DATA[0]}\nalias = resources\n".to_owned(),&Dialect::Bazel).unwrap(),&loading_globals()).unwrap();
    }
    let f = m.freeze().unwrap();
    let consumer = Module::new();
    fill(&consumer, "resources = 99\n");
    let cb = f.get("alias").unwrap().owned_value(consumer.frozen_heap());
    validate_resource_set(cb, None).unwrap();
    validate_resource_set(cb, Some(&consumer)).unwrap();
}
#[test]
fn resource_nested_lambda_and_rebound_alias_fail_but_native_callable_is_not_def_filtered() {
    let m = Module::new();
    fill(
        &m,
        "def make():\n    def nested(os, count): return {}\n    return nested\nclosure = make()\nlam = lambda os, count: {}\ndef top(os, count): return {}\nalias = top\ntop = 0\nnative = len\n",
    );
    for name in ["closure", "lam", "alias"] {
        assert!(
            validate_resource_set(m.get(name).unwrap(), Some(&m)).is_err(),
            "{name}"
        );
    }
    // Validation is callable/general: publication eligibility and two-arg
    // numeric invocation remain subsequent distinct boundaries.
    validate_resource_set(m.get("native").unwrap(), Some(&m)).unwrap();
}
