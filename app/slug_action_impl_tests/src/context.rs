/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use dupe::Dupe;
use indoc::indoc;
use slug_build_api::analysis::registry::AnalysisRegistry;
use slug_build_api::interpreter::rule_defs::context::AnalysisContext;
use slug_build_api::interpreter::rule_defs::plugins::AnalysisPlugins;
use slug_build_api::interpreter::rule_defs::register_rule_defs;
use slug_core::configuration::data::ConfigurationData;
use slug_core::deferred::base_deferred_key::BaseDeferredKey;
use slug_core::execution_types::execution::ExecutionPlatformResolution;
use slug_core::target::label::label::TargetLabel;
use slug_execute::digest_config::DigestConfig;
use slug_interpreter::file_type::StarlarkFileType;
use slug_interpreter::from_freeze::from_freeze_error;
use maplit::hashmap;
use starlark::collections::SmallMap;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::eval::ReturnFileLoader;
use starlark::syntax::AstModule;
use starlark::values::UnpackValue;
use starlark::values::Value;
use starlark::values::structs::AllocStruct;

fn run_ctx_test(
    content: &str,
    result_handler: impl FnOnce(starlark::Result<Value>) -> slug_error::Result<()>,
) -> slug_error::Result<()> {
    let func_mod = Module::new();
    let globals = GlobalsBuilder::standard().with(register_rule_defs).build();
    let prelude = indoc!(
        r#"
         def assert_eq(a, b):
             if a != b:
                 fail("Expected {}, got {}".format(a, b))
         "#
    );
    let full_content = format!("{prelude}\n{content}");

    {
        let mut eval = Evaluator::new(&func_mod);
        let ast = AstModule::parse(
            "foo.bzl",
            full_content,
            &StarlarkFileType::Bzl.dialect(false),
        )
        .unwrap();
        eval.eval_module(ast, &globals).unwrap();
    };
    let frozen_func_mod = func_mod.freeze().map_err(from_freeze_error)?;
    let test_function = frozen_func_mod.get("test").unwrap();

    let modules = hashmap!["func_mod" => &frozen_func_mod];

    let env = Module::new();
    let file_loader = ReturnFileLoader { modules: &modules };
    let test_function = test_function.owned_value(env.frozen_heap());
    let mut eval = Evaluator::new(&env);
    eval.set_loader(&file_loader);
    let label = TargetLabel::testing_parse("root//foo/bar:some_name")
        .configure(ConfigurationData::testing_new());
    let registry = AnalysisRegistry::new_from_owner(
        BaseDeferredKey::TargetLabel(label.dupe()),
        ExecutionPlatformResolution::unspecified(),
    )?;
    let attributes = eval
        .heap()
        .alloc_typed_unchecked(AllocStruct([("name", "some_name")]))
        .cast();
    let plugins = eval
        .heap()
        .alloc_typed(AnalysisPlugins::new(SmallMap::new()))
        .into();

    let ctx = eval.heap().alloc(AnalysisContext::prepare(
        eval.heap(),
        Some(attributes),
        Some(label),
        Some(plugins),
        registry,
        DigestConfig::testing_default(),
        vec![],
    ));

    let returned = eval.eval_function(test_function, &[ctx], &[]);
    result_handler(returned)
}

#[test]
fn ctx_instantiates() -> slug_error::Result<()> {
    let content = indoc!(
        r#"
         def test(ctx):
             assert_eq("foo/bar", ctx.label.package)
             assert_eq("some_name", ctx.label.name)
             assert_eq(None, ctx.label.sub_target)
             return ctx.attrs.name
         "#
    );
    run_ctx_test(content, |ret| {
        assert_eq!("some_name", ret.unwrap().unpack_str().unwrap());
        Ok(())
    })
}

#[test]
fn declare_output_declares_outputs() -> slug_error::Result<()> {
    let content = indoc!(
        r#"
         def test(c):
             out = c.actions.declare_output("foo/bar.cpp")
             return (out.basename, out.short_path)
         "#
    );

    run_ctx_test(content, |ret| {
        let a = <(&str, &str)>::unpack_value(ret.unwrap()).unwrap().unwrap();
        assert_eq!("bar.cpp", a.0);
        assert_eq!("foo/bar.cpp", a.1);
        Ok(())
    })
}

#[test]
fn declare_output_with_prefix() -> slug_error::Result<()> {
    let content = indoc!(
        r#"
         def test(c):
             out = c.actions.declare_output("out/test", "foo/bar.cpp")
             return (out.basename, out.short_path)
         "#
    );

    run_ctx_test(content, |ret| {
        let a = <(&str, &str)>::unpack_value(ret.unwrap()).unwrap().unwrap();
        assert_eq!("bar.cpp", a.0);
        assert_eq!("foo/bar.cpp", a.1);
        Ok(())
    })
}

#[test]
fn declare_output_dot() -> slug_error::Result<()> {
    let content = indoc!(
        r#"
         def test(c):
             return c.actions.declare_output("magic", ".")
         "#
    );

    let expect = "expected a normalized path";
    run_ctx_test(content, |ret| match ret {
        Err(e) if e.to_string().contains(expect) => Ok(()),
        _ => panic!("Expected a specific failure containing `{expect}`, got {ret:?}"),
    })
}

#[test]
fn declare_output_dot_bad() -> slug_error::Result<()> {
    let content = indoc!(
        r#"
         def test(c):
             return c.actions.declare_output("..")
         "#
    );

    let expect = "expected a normalized path";
    run_ctx_test(content, |ret| match ret {
        Err(e) if e.to_string().contains(expect) => Ok(()),
        _ => panic!("Expected a specific failure containing `{expect}`, got {ret:?}"),
    })
}

#[test]
fn declare_output_dotdot() -> slug_error::Result<()> {
    let content = indoc!(
        r#"
         def test(c):
             return c.actions.declare_output("foo/..")
         "#
    );

    let expect = "expected a normalized path";
    run_ctx_test(content, |ret| match ret {
        Err(e) if e.to_string().contains(expect) => Ok(()),
        _ => panic!("Expected a specific failure containing `{expect}`, got {ret:?}"),
    })
}

#[test]
fn declare_output_require_bound() -> slug_error::Result<()> {
    // In Bazel semantics, passing a declared artifact as an input to an action is
    // valid at analysis time (the artifact would be produced by another action at
    // build time). Buck2 required "binding" at analysis time but Bazel does not.
    let content = indoc!(
        r#"
         def test(c):
             a = c.actions.declare_output("a")
             b = c.actions.declare_output("b")
             c.actions.run([a, b.as_output()], category = "test_category")
         "#
    );

    run_ctx_test(content, |ret| match ret {
        Ok(_) => Ok(()),
        Err(e) => {
            panic!("Expected success (Bazel allows unbound artifacts as inputs), got error: {e}")
        }
    })
}
