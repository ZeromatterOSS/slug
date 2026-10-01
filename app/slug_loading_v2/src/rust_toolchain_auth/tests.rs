//! Negative authentication and descriptor tracing. Positive original-source
//! construction and native capture ownership belong to Analysis integration.
use std::path::PathBuf;

use starlark::environment::GlobalsBuilder;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::tuple::TupleRef;

use super::*;
use crate::bzl_module::BzlModuleIdentity;
use crate::bzl_module::BzlModuleSourceProvenance;
use crate::bzl_module::rust_toolchain_helper::synthetic_test_binding;
use crate::package::loading_globals;
use crate::subrule_invocation::AnalysisEvaluationContext;

const FILE: &str = "authentication.bzl";
const BODY: &str = "def callback(label, actions, allocator_library, std): pass\nout = platform_common.ToolchainInfo(make_libstd_and_allocator_ccinfo=callback)\n";

#[starlark::starlark_module]
fn entry_globals(builder: &mut GlobalsBuilder) {
    fn entry_probe<'v>(
        callback: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let contextual = eval.native_call_context("make_ccinfo").is_some();
        let local_value = eval.native_call_local_value("make_ccinfo");
        let local = local_value.is_some_and(|local| local.ptr_eq(callback));
        let filename = eval
            .native_caller_function_filename()
            .unwrap_or_else(|| "<none>".to_owned());
        let stack = eval.call_stack();
        let caller = stack
            .frames
            .iter()
            .rev()
            .nth(1)
            .map_or("<none>", |frame| frame.name.as_str());
        Ok(eval.heap().alloc(AllocTuple([
            Value::new_bool(contextual),
            Value::new_bool(local),
            eval.heap().alloc(filename),
            eval.heap().alloc(caller),
            Value::new_bool(initializer_frame(eval, FILE)),
            Value::new_bool(initializer_frame(eval, FILE) && local),
            Value::new_bool(local_value.is_some()),
        ])))
    }
}

#[test]
fn native_entry_uses_direct_def_and_local_without_requiring_a_caller_span() {
    let module = Module::new();
    let globals = GlobalsBuilder::standard().with(entry_globals).build();
    Evaluator::new(&module).eval_module(AstModule::parse(FILE, "def module_callback(): pass\nmodule_result=entry_probe(module_callback)\ndef _rust_toolchain_impl(substitute=False):\n    def make_ccinfo(): pass\n    def replacement(): pass\n    return entry_probe(replacement if substitute else make_ccinfo)\ndef another_frame(substitute=False):\n    def make_ccinfo(): pass\n    return entry_probe(make_ccinfo)\n".to_owned(), &Dialect::Bazel).unwrap(), &globals).unwrap();
    let module_values = TupleRef::from_value(module.get("module_result").unwrap()).unwrap();
    assert_eq!(module_values.content()[0].unpack_bool(), Some(false));
    assert_eq!(module_values.content()[1].unpack_bool(), Some(false));
    assert_eq!(module_values.content()[2].unpack_str(), Some("<none>"));
    assert_eq!(module_values.content()[4].unpack_bool(), Some(false));
    assert_eq!(module_values.content()[5].unpack_bool(), Some(false));
    assert_eq!(module_values.content()[6].unpack_bool(), Some(false));
    let frozen = module.freeze().unwrap();
    let consumer = Module::new();
    let mut eval = Evaluator::new(&consumer);
    for (name, substitute, same_local, expected_frame) in [
        ("_rust_toolchain_impl", false, true, true),
        ("_rust_toolchain_impl", true, false, true),
        ("another_frame", false, true, false),
    ] {
        let function = frozen.get_any_visibility(name).unwrap().0;
        let result = eval
            .eval_function(
                function.owned_value(consumer.frozen_heap()),
                &[Value::new_bool(substitute)],
                &[],
            )
            .unwrap();
        let result = TupleRef::from_value(result).unwrap();
        let values = result.content();
        // Rust eval_function creates no Starlark caller location, but the
        // immediate FrozenDef and its unwrapped local remain available.
        assert_eq!(values[0].unpack_bool(), Some(false));
        assert_eq!(values[1].unpack_bool(), Some(same_local));
        assert_eq!(values[2].unpack_str(), Some(FILE));
        assert_eq!(values[3].unpack_str(), Some(name));
        assert_eq!(values[4].unpack_bool(), Some(expected_frame));
        assert_eq!(values[5].unpack_bool(), Some(expected_frame && same_local));
        assert_eq!(values[6].unpack_bool(), Some(true));
    }
}

#[test]
fn pinned_multiline_constructor_authenticates_its_begin_line() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../slug_analysis_v2/tests/fixtures/rustc-map-each/rules_rust/rust/private/toolchain.bzl"
    ));
    assert_eq!(<[u8; 32]>::from(Sha256::digest(source)), CALLER_SHA);
    let ast = AstModule::parse(CALLER_LABEL, source.to_owned(), &Dialect::Bazel).unwrap();
    let mut calls = Vec::new();
    ast.statement().visit_expr(|expr| {
        if let starlark::syntax::ast::Expr::Call(function, _) = &expr.node {
            if ast.file_span(function.span).source_span() == "platform_common.ToolchainInfo" {
                calls.push(ast.file_span(expr.span).resolve_span());
            }
        }
    });
    let [span] = calls.as_slice() else {
        panic!("exactly one ToolchainInfo constructor");
    };
    assert_eq!(span.begin.line, 589);
    assert!(
        span.end.line > span.begin.line,
        "the original constructor spans many lines"
    );
    assert!(constructor_callsite(span.begin.line));
    assert!(!constructor_callsite(span.begin.line - 1));
    assert!(!constructor_callsite(span.begin.line + 1));
}

fn source(label: &str, digest: [u8; 32]) -> (CompactString, BzlModuleSourceProvenance) {
    (
        FILE.into(),
        BzlModuleSourceProvenance::new(
            BzlModuleIdentity {
                label: slug_identity_v2::CanonicalLabel::parse(label).unwrap(),
                workspace_path: PathBuf::from(FILE),
                repository_mapping: Arc::from([]),
            },
            digest,
        ),
    )
}

#[test]
fn fixed_canonical_caller_refuses_forged_observed_evaluated_and_duplicate_sources() {
    for (digest, duplicate) in [
        (Sha256::digest(BODY).into(), false),
        (CALLER_SHA, false),
        (CALLER_SHA, true),
    ] {
        let module = Module::new();
        let entry = source(CALLER_LABEL, digest);
        let mut entries = vec![entry.clone()];
        if duplicate {
            entries.push(entry.clone());
        }
        let context = AnalysisEvaluationContext::new(
            Arc::from([]),
            [],
            slug_identity_v2::CanonicalLabel::parse("@@//:subject").unwrap(),
            Arc::new(crate::cc_common::solib_tests::RecordingSink::default()),
            FrozenValue::new_none(),
            entries.into(),
        );
        let mut eval = Evaluator::new(&module);
        eval.extra = Some(&context);
        let error = eval
            .eval_module(
                AstModule::parse(FILE, BODY.to_owned(), &Dialect::Bazel).unwrap(),
                &loading_globals(),
            )
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("caller/source/frame/callsite authority mismatch"),
            "{error}"
        );
        assert!(rust_callable_registrations(&module).is_empty());
    }
    let module = Module::new();
    let entry = source(
        "@@other+//rust/private:toolchain.bzl",
        Sha256::digest(BODY).into(),
    );
    let context = AnalysisEvaluationContext::new(
        Arc::from([]),
        [],
        slug_identity_v2::CanonicalLabel::parse("@@//:subject").unwrap(),
        Arc::new(crate::cc_common::solib_tests::RecordingSink::default()),
        FrozenValue::new_none(),
        Arc::from([entry]),
    );
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    eval.eval_module(
        AstModule::parse(FILE, BODY.to_owned(), &Dialect::Bazel).unwrap(),
        &loading_globals(),
    )
    .unwrap();
    assert!(
        rust_callable_registrations(&module).is_empty(),
        "unrelated global functions remain outside the finite registry"
    );
    assert_eq!(module.get("out").unwrap().get_type(), "ToolchainInfo");
}

#[test]
fn registered_original_function_and_capture_aliases_are_real_gc_and_freeze_roots() {
    let binding = synthetic_test_binding(
        "def make_libstd_and_allocator_ccinfo(*args, **kwargs): fail('guard fixture helper must not execute')\n",
    );
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    eval.eval_module(
        AstModule::parse(
            FILE,
            "def original(): pass\nshared = ['capture']\n".to_owned(),
            &Dialect::Bazel,
        )
        .unwrap(),
        &loading_globals(),
    )
    .unwrap();
    let registration = {
        let original = module.get("original").unwrap();
        let identity = original
            .request_value::<&dyn StarlarkFunctionIdentity>()
            .unwrap();
        RustCallableRegistration {
            source: binding.authority().clone(),
            identity: identity.issue_identity(),
            name_hash: identity.name_hash().get(),
        }
    };
    {
        let original = module.get("original").unwrap();
        let shared = module.get("shared").unwrap();
        let captures =
            module
                .heap()
                .alloc(AllocTuple([shared, shared, Value::new_bool(false), shared]));
        DictMut::from_value(crate::evaluator_roots::rust_callable_registry(&module))
            .unwrap()
            .aref
            .insert_hashed(
                original.get_hashed().unwrap(),
                module.heap().alloc_complex(RegisteredRustCallable {
                    registration: registration.clone(),
                    captures,
                }),
            );
    }
    drop(binding);
    eval.eval_module(AstModule::parse(FILE, "original=None\nshared=None\ndef churn():\n    for i in range(1000):\n        temporary=[str(i)] * 256\nchurn()\ncollected=True\n".to_owned(), &Dialect::Bazel).unwrap(), &loading_globals()).unwrap();
    assert!(
        module.heap().allocated_bytes() < module.heap().peak_allocated_bytes(),
        "must collect while the registry alone roots original and captures"
    );
    {
        let entries = rust_callable_registrations(&module);
        let [(function, found, captures)] = entries.as_slice() else {
            panic!("one retained original");
        };
        assert!(Arc::ptr_eq(&found.identity, &registration.identity));
        assert!(Arc::ptr_eq(
            function
                .request_value::<&dyn StarlarkFunctionIdentity>()
                .unwrap()
                .issued_identity()
                .unwrap(),
            &registration.identity
        ));
        let values = TupleRef::from_value(*captures).unwrap().content();
        assert!(values[0].ptr_eq(values[1]) && values[1].ptr_eq(values[3]));
        assert_eq!(values[0].to_repr(), "[\"capture\"]");
        assert_eq!(values[2].unpack_bool(), Some(false));
    }
    drop(eval);
    let frozen = module.freeze().unwrap();
    let consumer = Module::new();
    consumer.frozen_heap().add_reference(frozen.frozen_heap());
    consumer.set_extra_value(frozen.extra_value().unwrap().to_value());
    drop(frozen);
    let entries = rust_callable_registrations(&consumer);
    let [(function, found, captures)] = entries.as_slice() else {
        panic!("one frozen original");
    };
    assert_eq!(found.source, registration.source);
    assert!(Arc::ptr_eq(&found.identity, &registration.identity));
    let values = TupleRef::from_value(*captures).unwrap().content();
    assert!(values[0].ptr_eq(values[1]) && values[1].ptr_eq(values[3]));
    assert_eq!(values[0].to_repr(), "[\"capture\"]");
    assert_eq!(
        Evaluator::new(&consumer)
            .eval_function(*function, &[], &[])
            .unwrap()
            .to_repr(),
        "None"
    );
}
