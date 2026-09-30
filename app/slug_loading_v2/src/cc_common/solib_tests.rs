//! Unchanged private rules_cc helper and production binder. Only unused imports
//! and feature/toolchain inputs are authored scaffolding; the recording sink
//! proves delegation. Analysis tests separately exercise the production sink.
use std::sync::Arc;
use std::sync::Mutex;

use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisConfiguredTargetKey;
use slug_identity_v2::CanonicalLabel;
use starlark::environment::FrozenModule;
use starlark::environment::Globals;
use starlark::environment::LibraryExtension;
use starlark::environment::Module;
use starlark::eval::FileLoader;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;
use crate::subrule_invocation::AnalysisActionCallScope;
use crate::subrule_invocation::AnalysisActionSink;
use crate::subrule_invocation::AnalysisArtifactValue;
use crate::subrule_invocation::AnalysisEvaluationContext;
use crate::subrule_invocation::AnalysisRunRequest;

const SOURCE: &str = include_str!(
    "../../../slug_analysis_v2/tests/fixtures/rustc-map-each/rules_cc/cc/private/link/cc_linking_helper.bzl"
);

#[derive(Debug, Default)]
pub(crate) struct RecordingSink(pub(crate) Mutex<Vec<(AnalysisArtifact, String)>>);
pub(crate) fn artifact() -> AnalysisArtifact {
    AnalysisArtifact::Derived {
        owner: AnalysisConfiguredTargetKey::new(
            CanonicalLabel::parse("@@rules_rust+//ffi/cc/allocator_library:producer").unwrap(),
            Arc::<[u8]>::from(&b"config"[..]),
        ),
        output: ActionOutput::new(
            crate::artifact_root::test_root(),
            "ffi/cc/allocator_library/liballocator.so",
            ActionOutputKind::File,
        ),
    }
}

fn returned_artifact() -> AnalysisArtifact {
    let AnalysisArtifact::Derived { owner, .. } = artifact() else {
        unreachable!()
    };
    AnalysisArtifact::Derived {
        owner,
        output: ActionOutput::new(
            crate::artifact_root::test_root(),
            "_solib_k8/from_sink.so",
            ActionOutputKind::File,
        ),
    }
}
impl AnalysisActionSink for RecordingSink {
    fn dynamic_library_symlink(
        &self,
        library: Value<'_>,
        directory: &str,
    ) -> anyhow::Result<AnalysisArtifactValue> {
        let library = AnalysisArtifactValue::from_starlark(library)
            .ok_or_else(|| anyhow::anyhow!("library must be a File"))?;
        self.0
            .lock()
            .unwrap()
            .push((library.artifact().clone(), directory.to_owned()));
        Ok(AnalysisArtifactValue::new(returned_artifact()))
    }
    fn declare_file(
        &self,
        _: &str,
        _: Option<&AnalysisArtifact>,
    ) -> anyhow::Result<AnalysisArtifactValue> {
        unreachable!()
    }
    fn write(&self, _: Value<'_>, _: Value<'_>, _: bool) -> anyhow::Result<()> {
        unreachable!()
    }
    fn run_shell(&self, _: Value<'_>, _: &str, _: Value<'_>) -> anyhow::Result<()> {
        unreachable!()
    }
    fn run(&self, _: AnalysisRunRequest<'_>) -> anyhow::Result<()> {
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
fn globals() -> Globals {
    GlobalsBuilder::extended_by(&[LibraryExtension::StructType]).build()
}
fn evaluate(module: &Module, source: &str) -> anyhow::Result<()> {
    Evaluator::new(module)
        .eval_module(
            AstModule::parse("solib_test.bzl", source.to_owned(), &Dialect::Bazel)
                .map_err(starlark::Error::into_anyhow)?,
            &globals(),
        )
        .map_err(starlark::Error::into_anyhow)?;
    Ok(())
}
struct Imports(FrozenModule);
impl FileLoader for Imports {
    fn load(&self, _: &str) -> starlark::Result<FrozenModule> {
        Ok(self.0.clone())
    }
}
fn helper() -> anyhow::Result<FrozenModule> {
    let module = Module::new();
    module.set("cc_internal", module.heap().alloc_simple(CcInternalModule));
    evaluate(
        &module,
        r#"
def inert(*args, **kwargs): fail('unused helper dependency executed')
depset = inert
paths = None
wrap_with_check_private_api = inert
use_pic_for_binaries = inert
use_pic_for_dynamic_libs = inert
artifact_category_names = None
EMPTY_COMPILATION_OUTPUTS = None
link_action = inert
make_library_to_link = inert
create_lto_artifacts_and_lto_indexing_action = inert
LINKING_MODE = None
LINK_TARGET_TYPE = struct(NODEPS_DYNAMIC_LIBRARY='nodeps')
USE_ARCHIVER = None
is_dynamic_library = inert
"#,
    )?;
    let loader = Imports(module.freeze()?);
    let module = Module::new();
    module.import_public_symbols(&loader.0);
    let mut eval = Evaluator::new(&module);
    eval.set_loader(&loader);
    eval.eval_module(
        AstModule::parse("cc_linking_helper.bzl", SOURCE.to_owned(), &Dialect::Bazel)
            .map_err(starlark::Error::into_anyhow)?,
        &globals(),
    )
    .map_err(starlark::Error::into_anyhow)?;
    drop(eval);
    Ok(module.freeze()?)
}
fn actions(sink: Arc<RecordingSink>) -> AnalysisActions {
    let context = AnalysisEvaluationContext::new(
        Arc::from([]),
        [],
        CanonicalLabel::parse("@@//:consumer").unwrap(),
        sink.clone(),
        starlark::values::FrozenValue::new_none(),
        Arc::from([]),
    );
    AnalysisActions::new(
        sink,
        context.root_token(),
        "rule context",
        AnalysisActionCallScope::Root,
    )
}

#[test]
fn unchanged_nodeps_helper_delegates_false_false_native_constructor() {
    use sha2::Digest;
    use sha2::Sha256;
    assert_eq!(
        format!("{:x}", Sha256::digest(SOURCE)),
        "c45dd243835bd70803a7bb2e0a11167c9ea5ac912b3f02d415841879873b2a03"
    );
    let helper = helper().unwrap();
    let module = Module::new();
    let sink = Arc::new(RecordingSink::default());
    module.set("actions", module.heap().alloc_simple(actions(sink.clone())));
    module.set(
        "library",
        module
            .heap()
            .alloc_simple(AnalysisArtifactValue::new(artifact())),
    );
    module.set(
        "returned",
        module
            .heap()
            .alloc_simple(AnalysisArtifactValue::new(returned_artifact())),
    );
    let construct = helper
        .get_assigned("_construct_dynamic_library_to_link")
        .unwrap()
        .0;
    module.set("construct", construct.value());
    evaluate(&module, r#"
def disabled(_): return False
result = construct(actions, 'nodeps', None, struct(file=library, library_identifier='allocator'), None, False, struct(is_enabled=disabled), struct(_solib_dir='_solib_k8'))
asserted = result['dynamic_library'] == returned and result['resolved_symlink_dynamic_library'] == library
"#).unwrap();
    assert_eq!(module.get("asserted").unwrap().unpack_bool(), Some(true));
    assert_eq!(
        *sink.0.lock().unwrap(),
        vec![(artifact(), "_solib_k8".to_owned())]
    );
}

#[test]
fn native_solib_binder_rejects_flags_and_types_before_delegation() {
    for call in [
        "native.dynamic_library_symlink(actions, library, '_solib_k8', True, False)",
        "native.dynamic_library_symlink(actions, library, '_solib_k8', False, True)",
        "native.dynamic_library_symlink(actions, library, '_solib_k8', 1, False)",
        "native.dynamic_library_symlink(None, library, '_solib_k8', False, False)",
        "native.dynamic_library_symlink(actions, library, None, False, False)",
        "native.dynamic_library_symlink(actions=actions, library=library, solib_directory='_solib_k8', preserve_name=False, prefix_consumer=False)",
        "native.dynamic_library_symlink(actions, None, '_solib_k8', False, False)",
    ] {
        let module = Module::new();
        let sink = Arc::new(RecordingSink::default());
        module.set("native", module.heap().alloc_simple(CcInternalModule));
        module.set("actions", module.heap().alloc_simple(actions(sink.clone())));
        module.set(
            "library",
            module
                .heap()
                .alloc_simple(AnalysisArtifactValue::new(artifact())),
        );
        assert!(evaluate(&module, call).is_err(), "{call}");
        assert!(sink.0.lock().unwrap().is_empty(), "{call}");
    }
}
