//! Only the pending Module.extra_value slot retains these moving Values.
use slug_identity_v2::CanonicalLabel;
use starlark::values::FrozenValue;
use starlark::values::list::AllocList;

use super::*;
use crate::provider::StarlarkDepset;
use crate::provider::starlark_user_provider_fields;
use crate::subrule_invocation::AnalysisArtifactValue;
use crate::subrule_invocation::AnalysisEvaluationContext;

#[path = "../../../../slug_build_api_v2/tests/cpp_compile_support/mod.rs"]
mod fixture;

fn check_retained(module: &Module) {
    let rows = pending(module);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0.snapshot.normal_action_count, 0);
    assert_eq!(rows[1].0.snapshot.normal_action_count, 1);
    assert!(rows[0].1.ptr_eq(rows[1].1));
    assert!(rows[0].2.ptr_eq(rows[1].2));
    let (prepared, variables, context) = &rows[0];
    let vars = crate::cc_variables::cc_variables_value(*variables).unwrap();
    let local = |name: &str| vars.locals.iter().find(|(key, _)| key == name).unwrap().1;
    assert_eq!(
        local("unused").to_repr(),
        "[\"before\", \"after-registration\"]"
    );
    let (_, fields) = starlark_user_provider_fields(*context).unwrap();
    let field = |name: &str| fields.iter().find(|(key, _)| key == name).unwrap().1;
    assert!(field("variables").ptr_eq(*variables));
    assert!(field("mutable").ptr_eq(local("unused")));
    let shared_depset = local("files");
    assert!(field("files").ptr_eq(shared_depset));
    for (prepared, _, _) in &rows {
        for role in prepared.input_depsets {
            assert!(role.ptr_eq(shared_depset));
        }
    }
    let direct = StarlarkDepset::direct_from_value(shared_depset).unwrap();
    assert_eq!(direct.len(), 1);
    assert_eq!(
        AnalysisArtifactValue::from_starlark(direct[0])
            .unwrap()
            .artifact(),
        &AnalysisArtifact::Source(CanonicalLabel::parse("@@//:rooted.h").unwrap())
    );
    assert_eq!(prepared.snapshot.construction.action_name, "c++-compile");
}

fn collect(module: &Module, context: &AnalysisEvaluationContext) {
    let mut eval = Evaluator::new(module);
    eval.extra = Some(context);
    eval.eval_module(
        AstModule::parse(
            "pending_pressure.bzl",
            // PossibleGc is emitted BEFORE top-level statements, never inside
            // this function. The final assignment supplies the safe collection
            // point after churn returns (matching context_lifecycle_tests).
            "def churn():\n    for i in range(1000):\n        temporary = [str(i)] * 256\nchurn()\ncollected = True\n"
                .to_owned(),
            &Dialect::Bazel,
        )
        .unwrap(),
        &loading_globals(),
    )
    .unwrap();
    assert!(
        module.heap().allocated_bytes() < module.heap().peak_allocated_bytes(),
        "the safe top-level GC point must reclaim unreachable churn allocations"
    );
}

#[test]
fn pending_recipes_keep_live_aliases_through_gc_and_root_context_cleanup() {
    let module = Module::new();
    module.set(
        "FILE",
        module
            .heap()
            .alloc(AnalysisArtifactValue::new(AnalysisArtifact::Source(
                CanonicalLabel::parse("@@//:rooted.h").unwrap(),
            ))),
    );
    {
        let context = BzlEvaluationContext::new("@@rules_cc+//cc/private:pending_gc.bzl");
        let mut eval = Evaluator::new(&module);
        eval.extra = Some(&context);
        eval.eval_module(
            AstModule::parse(
                "pending_gc.bzl",
                r#"
i = cc_common.internal_DO_NOT_USE()
P = provider(fields = ["variables", "files", "mutable"])
mutable = ["before"]
files = depset([FILE])
variables = i.cc_toolchain_variables(vars = {"unused": mutable, "files": files})
context = P(variables = variables, files = files, mutable = mutable)
"#
                .to_owned(),
                &Dialect::Bazel,
            )
            .unwrap(),
            &loading_globals(),
        )
        .unwrap();
    }
    let action = fixture::action(crate::artifact_root::test_root());
    for normal_action_count in [0, 1] {
        record(
            &module,
            PreparedCcCompile {
                snapshot: PendingCppCompile {
                    construction: action
                        .cpp_compile_spec()
                        .unwrap()
                        .data()
                        .construction
                        .clone(),
                    normal_action_count,
                    exec_group: None,
                    additional_files: Box::new([]),
                    mandatory_direct: Box::new([]),
                },
                input_depsets: [module.get("files").unwrap(); 5],
            },
            module.get("variables").unwrap(),
            module.get("context").unwrap(),
        );
    }
    // Mutate after recording, then remove every ordinary module binding to the
    // Variables, context, list, File and depset. No borrowed Value crosses GC.
    Evaluator::new(&module)
        .eval_module(
            AstModule::parse(
                "clear.bzl",
                r#"
mutable.append("after-registration")
mutable = None
variables = None
context = None
files = None
FILE = None
"#
                .to_owned(),
                &Dialect::Bazel,
            )
            .unwrap(),
            &loading_globals(),
        )
        .unwrap();
    let context = AnalysisEvaluationContext::new(
        Arc::from([]),
        [],
        CanonicalLabel::parse("@@//:subject").unwrap(),
        Arc::new(crate::cc_common::solib_tests::RecordingSink::default()),
        FrozenValue::new_none(),
        Arc::from([]),
    );
    let guard = context
        .enter_root_context(&module, module.heap().alloc(AllocList([7])))
        .unwrap();
    collect(&module, &context);
    check_retained(&module);
    drop(guard);
    assert!(crate::evaluator_roots::analysis_context(&module).is_none());
    collect(&module, &context);
    check_retained(&module);
}
