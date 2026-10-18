use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use compact_str::CompactString;
use slug_identity_v2::CanonicalLabel;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::FrozenValue;
use starlark::values::Value;

use super::*;
use crate::BzlModuleIdentity;
use crate::BzlModuleSourceProvenance;
use crate::package::loading_globals;
use crate::subrule_invocation::AnalysisActionCallScope;
use crate::subrule_invocation::AnalysisActionSink;
use crate::subrule_invocation::AnalysisActions;
use crate::subrule_invocation::AnalysisEvaluationContext;
const FILE: &str = "unrelated_transform.bzl";
const LABEL: &str = "@@//test/arbitrary:unrelated.bzl";
#[derive(Debug, Default)]
struct Sink(
    Mutex<
        Vec<(
            BuildInfoTransformKind,
            BuildInfoCallbackRegistration,
            AnalysisArtifact,
        )>,
    >,
);
impl AnalysisActionSink for Sink {
    fn transform_build_info(
        &self,
        request: BuildInfoTransformRequest<'_>,
    ) -> anyhow::Result<AnalysisArtifactValue> {
        let root = crate::artifact_root::test_root();
        let output = if request.kind == BuildInfoTransformKind::Version {
            slug_build_api_v2::ActionOutput::new_constant(root, request.output_file_name)
        } else {
            slug_build_api_v2::ActionOutput::new(
                root,
                request.output_file_name,
                slug_build_api_v2::ActionOutputKind::File,
            )
        };
        let artifact = AnalysisArtifact::Derived {
            owner: slug_build_api_v2::AnalysisConfiguredTargetKey::new(
                CanonicalLabel::parse("@@//:subject").unwrap(),
                &b"test-owner"[..],
            ),
            output,
        };
        self.0
            .lock()
            .unwrap()
            .push((request.kind, request.callback, request.template));
        Ok(AnalysisArtifactValue::new(artifact))
    }
    fn declare_file(
        &self,
        _: &str,
        _: Option<&AnalysisArtifact>,
    ) -> anyhow::Result<AnalysisArtifactValue> {
        unreachable!()
    }
    fn write<'v>(&self, _: &'v Module, _: Value<'v>, _: Value<'v>, _: bool) -> anyhow::Result<()> {
        unreachable!()
    }
    fn run_shell(&self, _: Value<'_>, _: &str, _: Value<'_>) -> anyhow::Result<()> {
        unreachable!()
    }
    fn artifact_symlink(
        &self,
        _: Value<'_>,
        _: Value<'_>,
        _: bool,
        _: Option<&str>,
    ) -> anyhow::Result<()> {
        unreachable!()
    }
    fn absolute_symlink(&self, _: Value<'_>, _: &str, _: Option<&str>) -> anyhow::Result<()> {
        unreachable!()
    }
}
fn provenance(label: &str, digest: [u8; 32]) -> (CompactString, BzlModuleSourceProvenance) {
    (
        FILE.into(),
        BzlModuleSourceProvenance::new(
            BzlModuleIdentity {
                label: CanonicalLabel::parse(label).unwrap(),
                source: slug_build_api_v2::ModuleSourceOrigin::Workspace(PathBuf::from(FILE)),
                // Match the installed builtin route: the unchanged rule's
                // template defaults resolve its apparent self-repository.
                repository_mapping: Arc::from([(
                    slug_identity_v2::ApparentRepoName::new("bazel_tools").unwrap(),
                    slug_identity_v2::CanonicalRepoName::new("bazel_tools").unwrap(),
                )]),
            },
            digest,
        ),
    )
}

fn execute(source: &str, label: &str) -> (Module, Arc<Sink>, Result<(), String>) {
    let module = Module::new();
    let sink = Arc::new(Sink::default());
    let ast = AstModule::parse(FILE, source.to_owned(), &Dialect::Bazel).unwrap();
    let identity = provenance(label, [7; 32]).1.identity;
    let prepared = crate::bzl_module::test_source::prepare(&module, &ast, identity, &[]).unwrap();
    let context = AnalysisEvaluationContext::new(
        Arc::from([]),
        [],
        CanonicalLabel::parse("@@//:subject").unwrap(),
        sink.clone(),
        FrozenValue::new_none(),
        prepared.sources(),
    );
    module.set(
        "actions",
        module.heap().alloc_simple(AnalysisActions::new(
            sink.clone(),
            context.root_token(),
            "rule context",
            AnalysisActionCallScope::Root,
        )),
    );
    module.set(
        "template",
        module
            .heap()
            .alloc_simple(AnalysisArtifactValue::new(AnalysisArtifact::Source(
                CanonicalLabel::parse("@@//:unrelated.template").unwrap(),
            ))),
    );
    let result = {
        let mut eval = Evaluator::new(&module);
        eval.extra = Some(&context);
        eval.eval_module(ast, &loading_globals())
            .map(|_| ())
            .map_err(|e| e.to_string())
    };
    drop(prepared);
    drop(context);
    (module, sink, result)
}
#[test]
fn unrelated_original_closures_both_methods_share_traced_collector_and_transfer_once() {
    let source = "def make():\n    cell = 'original'\n    def f(values): return {'key':cell}\n    return f\ndef call():\n    actions.transform_info_file(transform_func=make(),template=template,output_file_name='info')\n    actions.transform_version_file(transform_func=make(),template=template,output_file_name='version')\ncall()\ndef churn():\n    for i in range(1000):\n        temporary = [str(i)] * 256\nchurn()\n";
    let (module, sink, result) = execute(source, LABEL);
    result.unwrap();
    let rows = sink.0.lock().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0, BuildInfoTransformKind::Info);
    assert_eq!(rows[1].0, BuildInfoTransformKind::Version);
    let mut charges = Vec::new();
    crate::resource_set::transfer_resource_sets(&module, |w, b| {
        charges.push((w, b));
        Ok(())
    })
    .unwrap();
    assert_eq!(charges.len(), 2);
    assert!(charges[1].0 > charges[0].0);
    let mut lookup = Vec::new();
    let callback = callback_value(&module, &rows[0].1, |w, b| {
        lookup.push((w, b));
        Ok(())
    })
    .unwrap();
    assert_eq!(lookup.len(), 1);
    let value = Evaluator::new(&module)
        .eval_function(callback, &[Value::new_none()], &[])
        .unwrap();
    assert_eq!(
        starlark::values::dict::DictRef::from_value(value)
            .unwrap()
            .get_str("key")
            .unwrap()
            .unpack_str(),
        Some("original")
    );
    assert!(crate::resource_set::transfer_resource_sets(&module, |_, _| Ok(())).is_err());
    assert!(crate::subrule_invocation::clear_args_snapshots(&module).is_err());
    drop(rows);
    drop(sink);
    assert!(
        module.freeze().is_err(),
        "unaudited collector must never freeze"
    );
}
#[test]
fn default_access_and_named_parameter_function_file_controls_are_preserved() {
    for method in ["transform_info_file", "transform_version_file"] {
        for (args, expected) in [
            ("callback,template,'out'", "named-only"),
            ("template=template,output_file_name='out'", "transform_func"),
            (
                "transform_func=None,template=template,output_file_name='out'",
                "Starlark function",
            ),
            (
                "transform_func=str,template=template,output_file_name='out'",
                "Starlark function",
            ),
            (
                "transform_func=callback,template='x',output_file_name='out'",
                "File",
            ),
            (
                "transform_func=callback,template=template,output_file_name=1",
                "str",
            ),
        ] {
            let source = format!(
                "def callback(s): return {{}}\ndef call(): actions.{method}({args})\ncall()\n"
            );
            let (_, sink, result) = execute(&source, LABEL);
            assert!(result.unwrap_err().contains(expected), "{method} {args}");
            assert!(sink.0.lock().unwrap().is_empty());
        }
        let source = format!(
            "def callback(s): return {{}}\ndef call(): actions.{method}(transform_func=callback,template=template,output_file_name='out')\ncall()\n"
        );
        let (_, sink, result) = execute(&source, "@@//user:arbitrary.bzl");
        assert!(result.unwrap_err().contains("private API"));
        assert!(sink.0.lock().unwrap().is_empty());
        let (_, sink, result) = execute(&source, LABEL);
        result.unwrap();
        assert_eq!(sink.0.lock().unwrap().len(), 1);
    }
}

#[test]
fn escaped_transform_actions_reject_after_context_expiry() {
    for method in ["transform_info_file", "transform_version_file"] {
        let module = Module::new();
        let sink = Arc::new(Sink::default());
        let context = AnalysisEvaluationContext::new(
            Arc::from([]),
            [],
            CanonicalLabel::parse("@@//:subject").unwrap(),
            sink.clone(),
            FrozenValue::new_none(),
            Arc::from([provenance(LABEL, [7; 32])]),
        );
        module.set(
            "actions",
            module.heap().alloc_simple(AnalysisActions::new(
                sink.clone(),
                context.root_token(),
                "rule context",
                AnalysisActionCallScope::Root,
            )),
        );
        module.set(
            "template",
            module
                .heap()
                .alloc_simple(AnalysisArtifactValue::new(AnalysisArtifact::Source(
                    CanonicalLabel::parse("@@//:template").unwrap(),
                ))),
        );
        let source = format!(
            "def callback(s): return {{}}\ndef escaped(): return actions.{method}(transform_func=callback,template=template,output_file_name='out')\n"
        );
        let guard = context
            .enter_root_context(&module, Value::new_none())
            .unwrap();
        let mut eval = Evaluator::new(&module);
        eval.extra = Some(&context);
        eval.eval_module(
            AstModule::parse(FILE, source, &Dialect::Bazel).unwrap(),
            &loading_globals(),
        )
        .unwrap();
        drop(guard);
        let error = eval
            .eval_function(module.get("escaped").unwrap(), &[], &[])
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("outside of its own implementation"),
            "{error}"
        );
        assert!(sink.0.lock().unwrap().is_empty());
    }
}

#[test]
fn actual_registration_typed_token_work_boundary_is_cumulative_and_poisoned() {
    use crate::bzl_module::semantic_evaluation::budget::RECORD_LIMIT;
    assert_eq!(
        std::mem::size_of::<BuildInfoCallbackRegistration>(),
        std::mem::size_of::<crate::resource_set::ResourceSetRegistration>()
    );
    // Collector creation 2+16; both subsequent collector entries 2 each;
    // function inspection 8, actual token/vector retention 8+1. No allowance reset.
    let work = 2 + 16 + 2 + 8 + 2 + 8 + 1;
    for (seed, ok) in [
        (RECORD_LIMIT - work, true),
        (RECORD_LIMIT - work + 1, false),
    ] {
        let module = Module::new();
        Evaluator::new(&module)
            .eval_module(
                AstModule::parse(
                    FILE,
                    "def callback(status): return {}\n".to_owned(),
                    &Dialect::Bazel,
                )
                .unwrap(),
                &loading_globals(),
            )
            .unwrap();
        crate::resource_set::prepay_action_storage(&module, seed, 0).unwrap();
        let result = register_callback(&module, module.get("callback").unwrap());
        assert_eq!(result.is_ok(), ok);
        if !ok {
            assert!(crate::resource_set::prepay_action_storage(&module, 0, 0).is_err());
        }
        assert!(crate::subrule_invocation::clear_args_snapshots(&module).is_err());
    }
    let module = Module::new();
    assert!(register_callback(&module, Value::new_none()).is_err());
}
