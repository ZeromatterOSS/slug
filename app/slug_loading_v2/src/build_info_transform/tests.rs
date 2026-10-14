use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use compact_str::CompactString;
use slug_identity_v2::CanonicalLabel;
use starlark::environment::FrozenModule;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::FrozenValue;
use starlark::values::list::ListRef;

use super::*;
use crate::BzlModuleIdentity;
use crate::BzlModuleSourceProvenance;
use crate::package::loading_globals;
use crate::provider::BzlEvaluationContext;
use crate::subrule_invocation::AnalysisActionCallScope;
use crate::subrule_invocation::AnalysisActionSink;
use crate::subrule_invocation::AnalysisActions;
use crate::subrule_invocation::AnalysisEvaluationContext;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../slug_bzlmod_v2/builtin/bazel_tools/tools/build_defs/build_info/bazel_cc_build_info.bzl"
));
const FILE: &str = "build_info.bzl";

#[test]
fn builtin_mapping_promotion_is_limited_to_the_zero_import_pinned_cpp_module() {
    assert!(needs_builtin_mapping(
        &CanonicalLabel::parse(LABEL).unwrap(),
        &[]
    ));
    for label in [
        "@@bazel_tools//tools/build_defs/build_info:bazel_java_build_info.bzl",
        "@@bazel_tools//tools/build_defs:unrelated.bzl",
        "@@copied//tools/build_defs/build_info:bazel_cc_build_info.bzl",
        "@@bazel_tools//copied:bazel_cc_build_info.bzl",
    ] {
        assert!(!needs_builtin_mapping(
            &CanonicalLabel::parse(label).unwrap(),
            &[]
        ));
    }
    for loads in [
        vec![":relative.bzl".to_owned()],
        vec!["@other//:defs.bzl".to_owned()],
    ] {
        assert!(!needs_builtin_mapping(
            &CanonicalLabel::parse(LABEL).unwrap(),
            &loads
        ));
    }
}

#[derive(Debug, Default)]
struct Sink(
    Mutex<
        Vec<(
            BuildInfoCallbackRecipe,
            AnalysisArtifact,
            String,
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
        let output = match request.callback.kind() {
            BuildInfoTransformKind::Info => slug_build_api_v2::ActionOutput::new(
                root,
                request.output_file_name,
                slug_build_api_v2::ActionOutputKind::File,
            ),
            BuildInfoTransformKind::Version => {
                slug_build_api_v2::ActionOutput::new_constant(root, request.output_file_name)
            }
        };
        let artifact = AnalysisArtifact::Derived {
            owner: slug_build_api_v2::AnalysisConfiguredTargetKey::new(
                CanonicalLabel::parse("@@//:subject").unwrap(),
                &b"test-owner"[..],
            ),
            output,
        };
        self.0.lock().unwrap().push((
            request.callback,
            request.template.clone(),
            request.output_file_name.to_owned(),
            artifact.clone(),
        ));
        // Actual sink ownership/registry validation has separate owner tests.
        // These typed Files exercise callback result lifetime and metadata.
        Ok(AnalysisArtifactValue::new(artifact))
    }
    fn declare_file(
        &self,
        _: &str,
        _: Option<&AnalysisArtifact>,
    ) -> anyhow::Result<AnalysisArtifactValue> {
        unreachable!()
    }
    fn write<'v>(
        &self,
        _: &'v starlark::environment::Module,
        _: Value<'v>,
        _: Value<'v>,
        _: bool,
    ) -> anyhow::Result<()> {
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
                workspace_path: PathBuf::from(FILE),
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

fn definition(source: &str) -> FrozenModule {
    definition_at(source, LABEL)
}

fn definition_at(source: &str, label: &str) -> FrozenModule {
    let module = Module::new();
    let p = provenance(label, SHA);
    let context = BzlEvaluationContext::macro_runtime_context(p.1.identity.clone(), Arc::from([p]));
    let ast = AstModule::parse(FILE, source.to_owned(), &Dialect::Bazel).unwrap();
    let prepared = crate::bzl_module::test_source::prepare(
        &module,
        &ast,
        context.source_identity().clone(),
        &[],
    )
    .unwrap();
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(prepared.context());
    eval.eval_module(ast, &loading_globals()).unwrap();
    drop(eval);
    module.freeze().unwrap()
}

fn context(
    sink: Arc<Sink>,
    label: &str,
    digest: [u8; 32],
    duplicate: bool,
) -> AnalysisEvaluationContext {
    let p = provenance(label, digest);
    let sources = if duplicate {
        vec![p.clone(), p]
    } else {
        vec![p]
    };
    AnalysisEvaluationContext::new(
        Arc::from([]),
        [],
        CanonicalLabel::parse("@@//:subject").unwrap(),
        sink,
        FrozenValue::new_none(),
        sources.into(),
    )
}

fn file<'v>(module: &'v Module) -> Value<'v> {
    module
        .heap()
        .alloc_simple(AnalysisArtifactValue::new(AnalysisArtifact::Source(
            CanonicalLabel::parse("@@//:template").unwrap(),
        )))
}

fn actions<'v>(
    module: &'v Module,
    context: &AnalysisEvaluationContext,
    sink: Arc<Sink>,
) -> Value<'v> {
    module.heap().alloc_simple(AnalysisActions::new(
        sink,
        context.root_token(),
        "rule context",
        AnalysisActionCallScope::Root,
    ))
}

fn invoke(
    source: &str,
    label: &str,
    observed: [u8; 32],
    duplicate: bool,
) -> (Result<(), String>, Arc<Sink>) {
    let definition = definition_at(source, label);
    let module = Module::new();
    let sink = Arc::new(Sink::default());
    let context = context(sink.clone(), label, observed, duplicate);
    let function = definition
        .get_any_visibility("_get_build_info_files")
        .unwrap()
        .0;
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    let result = eval
        .eval_function(
            function.owned_value(module.frozen_heap()),
            &[
                actions(&module, &context, sink.clone()),
                file(&module),
                file(&module),
                file(&module),
                Value::new_bool(true),
            ],
            &[],
        )
        .map(|value| module.set("outputs", value))
        .map_err(|e| e.to_string());
    if result.is_ok() {
        // No moving Value survives in Rust across collection. Only the module
        // outputs slot retains the returned File list; the sink owns recipes.
        eval.eval_module(AstModule::parse("pressure.bzl", "def churn():\n    for i in range(1000):\n        temporary = [str(i)] * 256\nchurn()\ncollected = True\n".to_owned(), &Dialect::Bazel).unwrap(), &loading_globals()).unwrap();
        assert!(
            module.heap().allocated_bytes() < module.heap().peak_allocated_bytes(),
            "must actually collect at supported evaluation safe points"
        );
        drop(eval);
        let frozen = module.freeze().unwrap();
        let outputs = frozen.get("outputs").unwrap();
        let files = ListRef::from_value(outputs.value()).unwrap();
        let rows = sink.0.lock().unwrap();
        // The unchanged helper returns stable first despite registering volatile first.
        assert_eq!(
            AnalysisArtifactValue::from_starlark(files[0])
                .unwrap()
                .artifact(),
            &rows[1].3
        );
        assert_eq!(
            AnalysisArtifactValue::from_starlark(files[1])
                .unwrap()
                .artifact(),
            &rows[0].3
        );
        for row in rows.iter() {
            let AnalysisArtifact::Derived { output, .. } = &row.3 else {
                unreachable!()
            };
            assert_eq!(
                output.metadata(),
                if row.0.kind() == BuildInfoTransformKind::Version {
                    slug_build_api_v2::ActionOutputMetadata::Constant
                } else {
                    slug_build_api_v2::ActionOutputMetadata::Ordinary
                }
            );
        }
    }
    (result, sink)
}

#[starlark::starlark_module]
fn wrong_role_globals(builder: &mut starlark::environment::GlobalsBuilder) {
    fn wrong_role<'v>(
        #[starlark(require = named)] transform_func: Value<'v>,
        #[starlark(require = named)] template: Value<'v>,
        #[starlark(require = named)] output_file_name: &'v str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<starlark::values::none::NoneType> {
        // Only the requested semantic role is wrong. The caller executes the
        // exact unchanged bytes and genuine callback at the version callsite.
        prepare(
            BuildInfoTransformKind::Info,
            transform_func,
            template,
            output_file_name,
            eval,
        )?;
        Ok(starlark::values::none::NoneType)
    }
}

#[test]
fn unchanged_source_wrong_role_is_rejected_by_callsite_guard_alone() {
    let definition = definition(SOURCE);
    let module = Module::new();
    let sink = Arc::new(Sink::default());
    let context = context(sink, LABEL, SHA, false);
    let globals = starlark::environment::GlobalsBuilder::new()
        .with(wrong_role_globals)
        .build();
    let wrong = globals
        .iter()
        .find(|(name, _)| *name == "wrong_role")
        .unwrap()
        .1;
    let actions = module
        .heap()
        .alloc(starlark::values::structs::AllocStruct([(
            "transform_version_file",
            wrong.to_value(),
        )]));
    let function = definition
        .get_any_visibility("_get_build_info_files")
        .unwrap()
        .0;
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    let error = eval
        .eval_function(
            function.owned_value(module.frozen_heap()),
            &[
                actions,
                file(&module),
                file(&module),
                file(&module),
                Value::new_bool(true),
            ],
            &[],
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unchanged pinned source and transform callsite"),
        "{error}"
    );
}

#[test]
fn pinned_callbacks_register_both_recipes_without_evaluation_or_heap_retention() {
    assert_eq!(<[u8; 32]>::from(Sha256::digest(SOURCE.as_bytes())), SHA);
    let (result, sink) = invoke(SOURCE, LABEL, SHA, false);
    result.unwrap();
    // All producing modules/heaps have already been dropped by invoke().
    // Calling the version callback with no timestamp would fail; registration
    // succeeds without any status dictionary or callback evaluation.
    let rows = sink.0.lock().unwrap();
    assert_eq!(rows.len(), 2);
    for (row, kind, name) in [
        (&rows[0], BuildInfoTransformKind::Version, "volatile_file.h"),
        (
            &rows[1],
            BuildInfoTransformKind::Info,
            "non_volatile_file.h",
        ),
    ] {
        assert_eq!(row.0.kind(), kind);
        assert_eq!(row.0.source().to_string(), LABEL);
        assert_eq!(row.0.digest(), &SHA);
        assert_eq!(row.1.path(), "template");
        assert_eq!(row.2, name);
    }
}

#[test]
fn source_authentication_rejects_changed_copied_unobserved_and_ambiguous_sources() {
    let changed = format!("{SOURCE}\n# changed\n");
    for (case, (source, label, observed, duplicate, expected)) in [
        (
            changed.as_str(),
            LABEL,
            SHA,
            false,
            "conflicting original module provenance",
        ),
        (
            SOURCE,
            "@@//tools/build_defs/build_info:copied.bzl",
            SHA,
            false,
            "unchanged pinned source",
        ),
        // The current source row does not authenticate this actual original.
        (
            SOURCE,
            LABEL,
            [0; 32],
            false,
            "conflicting original module provenance",
        ),
        (
            SOURCE,
            LABEL,
            SHA,
            true,
            "build-info callback requires the unchanged pinned source and transform callsite",
        ),
        (
            SOURCE,
            "@@//ordinary:copied.bzl",
            SHA,
            false,
            "cannot use private API",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        if duplicate {
            // Only uniqueness is invalid: the exact original source, label,
            // digest and both complete admitted rows are otherwise identical.
            let admitted = context(Arc::new(Sink::default()), label, observed, duplicate);
            let rows = admitted.source_identities_by_filename();
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0], rows[1]);
            assert_eq!(rows[0].0.as_str(), FILE);
            assert_eq!(rows[0].1.identity.label.to_string(), LABEL);
            assert_eq!(rows[0].1.source_digest, SHA);
            let actual: [u8; 32] = Sha256::digest(source.as_bytes()).into();
            assert_eq!(actual, SHA);
            assert_eq!(observed, SHA);
        }
        let (result, sink) = invoke(source, label, observed, duplicate);
        let error = result.unwrap_err();
        assert!(
            error.contains(expected),
            "case={case} label={label} duplicate={duplicate} observed_prefix={:02x?} expected={expected:?}: {error}",
            &observed[..4]
        );
        assert!(sink.0.lock().unwrap().is_empty());
    }
}

fn call_text(body: &str) -> (Result<(), String>, Arc<Sink>) {
    let module = Module::new();
    let sink = Arc::new(Sink::default());
    let source = format!(
        "def callback(values):\n    fail('must not invoke')\ndef call():\n    {body}\ncall()\n"
    );
    let ast = AstModule::parse(FILE, source, &Dialect::Bazel).unwrap();
    let identity = provenance(LABEL, SHA).1.identity;
    let prepared = crate::bzl_module::test_source::prepare(&module, &ast, identity, &[]).unwrap();
    let context = AnalysisEvaluationContext::new(
        Arc::from([]),
        [],
        CanonicalLabel::parse("@@//:subject").unwrap(),
        sink.clone(),
        FrozenValue::new_none(),
        prepared.sources(),
    );
    module.set("actions", actions(&module, &context, sink.clone()));
    module.set("template", file(&module));
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    let result = eval.eval_module(ast, &loading_globals());
    (result.map(|_| ()).map_err(|e| e.to_string()), sink)
}

#[test]
fn transform_binders_require_three_named_arguments_and_native_value_types() {
    for method in ["transform_info_file", "transform_version_file"] {
        for (arguments, expected) in [
            ("callback, template, 'out'", "named-only parameter"),
            (
                "template = template, output_file_name = 'out'",
                "transform_func",
            ),
            (
                "transform_func = callback, output_file_name = 'out'",
                "template",
            ),
            (
                "transform_func = callback, template = template",
                "output_file_name",
            ),
            (
                "transform_func = callback, template = template, output_file_name = 'out', unknown = 1",
                "unknown",
            ),
            (
                "transform_func = None, template = template, output_file_name = 'out'",
                "Starlark function",
            ),
            (
                "transform_func = str, template = template, output_file_name = 'out'",
                "Starlark function",
            ),
            (
                "transform_func = callback, template = 'file', output_file_name = 'out'",
                "template must be a File",
            ),
            (
                "transform_func = callback, template = template, output_file_name = 1",
                "str",
            ),
            (
                "transform_func = callback, template = template, output_file_name = 'out'",
                "unchanged pinned source",
            ),
        ] {
            let (result, sink) = call_text(&format!("actions.{method}({arguments})"));
            let error = result.unwrap_err();
            assert!(error.contains(expected), "{method}({arguments}): {error}");
            assert!(sink.0.lock().unwrap().is_empty());
        }
    }
}

#[test]
fn escaped_transform_actions_reject_after_context_expiry() {
    let definition = definition(SOURCE);
    let module = Module::new();
    let sink = Arc::new(Sink::default());
    let context = context(sink.clone(), LABEL, SHA, false);
    let action_value = actions(&module, &context, sink.clone());
    let guard = context
        .enter_root_context(&module, Value::new_none())
        .unwrap();
    drop(guard);
    let function = definition
        .get_any_visibility("_get_build_info_files")
        .unwrap()
        .0;
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    let error = eval
        .eval_function(
            function.owned_value(module.frozen_heap()),
            &[
                action_value,
                file(&module),
                file(&module),
                file(&module),
                Value::new_bool(true),
            ],
            &[],
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("outside of its own implementation"),
        "{error}"
    );
    assert!(sink.0.lock().unwrap().is_empty());
}
