//! Full pinned module and load-only inert dependency exports; no source aliases.
use std::path::PathBuf;
use std::sync::Arc;

use compact_str::CompactString;
use starlark::environment::FrozenModule;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Module;
use starlark::eval::FileLoader;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::none::NoneType;
use starlark_map::small_map::SmallMap;

use super::*;
use crate::BzlModuleIdentity;
use crate::BzlModuleSourceProvenance;
use crate::package::loading_globals;
use crate::provider::BzlEvaluationContext;

const SOURCE: &str = include_str!(
    "../../../slug_analysis_v2/tests/fixtures/rustc-map-each/rules_cc/cc/private/link/finalize_link_action.bzl"
);
const FILE: &str = "finalize_link_action.bzl";
const STUB: &str = r#"
def noop(*args, **kwargs): return None
def empty(*args, **kwargs): return []
def environment(*args, **kwargs): return {}
def disabled(*args, **kwargs): return False
def mode(): return 'fastbuild'
def root(*args, **kwargs): return struct(exec_groups = {}, toolchains = {})
def memlimit(mode, info): return info
semantics = struct(get_cc_link_memlimit = memlimit)
cc_internal = struct(cc_toolchain_variables = noop, get_link_args = empty, actions2ctx_cheat = root)
native_cc_common = struct(get_execution_requirements = empty, get_environment_variables = environment)
register_linkstamp_compile_action = noop
collect_solib_dirs = noop
add_libraries_to_link = noop
add_object_files_to_link = noop
process_objects_for_lto = noop
setup_common_linking_variables = noop
LINKING_MODE = None
LINK_TARGET_TYPE = None
USE_ARCHIVER = 'archiver'
USE_LINKER = 'linker'
is_dynamic_library = disabled
configuration = struct(is_enabled = disabled)
toolchain = struct(_supports_param_files = False, _cpp_configuration = struct(compilation_mode = mode))
"#;
struct Loader(FrozenModule);
impl FileLoader for Loader {
    fn load(&self, _: &str) -> starlark::Result<FrozenModule> {
        Ok(self.0.clone())
    }
}
fn freeze(source: &str, loader: Option<&dyn FileLoader>) -> FrozenModule {
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    if let Some(loader) = loader {
        eval.set_loader(loader);
    }
    eval.eval_module(
        AstModule::parse(FILE, source.to_owned(), &Dialect::Bazel).unwrap(),
        &loading_globals(),
    )
    .unwrap();
    drop(eval);
    module.freeze().unwrap()
}
fn provenance(label: &str, digest: [u8; 32]) -> (CompactString, BzlModuleSourceProvenance) {
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
#[starlark::starlark_module]
fn probe(builder: &mut GlobalsBuilder) {
    fn capture<'v>(
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recipe = authenticate(*kwargs.get("resource_set").unwrap(), eval);
        eval.module()
            .set("admitted", Value::new_bool(recipe.is_some()));
        if let Some(recipe) = recipe {
            assert_eq!(recipe.source(), RetainedSpawnResourceSet::SOURCE_LABEL);
            assert_eq!(recipe.digest(), &RetainedSpawnResourceSet::SOURCE_DIGEST);
        }
        Ok(NoneType)
    }
}
fn invoke(source: &str, label: &str, digest: [u8; 32], copies: usize) -> bool {
    let loader = Loader(freeze(STUB, None));
    let definition = freeze(source, Some(&loader));
    let module = Module::new();
    let provenance = provenance(label, digest);
    let context = BzlEvaluationContext::macro_runtime_context(
        provenance.1.identity.clone(),
        vec![provenance; copies].into(),
    );
    let globals = GlobalsBuilder::new().with(probe).build();
    let capture = globals
        .iter()
        .find(|(name, _)| *name == "capture")
        .unwrap()
        .1;
    let actions = module
        .heap()
        .alloc(starlark::values::structs::AllocStruct([(
            "run",
            capture.to_value(),
        )]));
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    eval.eval_function(
        definition
            .get_any_visibility("_create_action")
            .unwrap()
            .0
            .owned_value(module.frozen_heap()),
        &[
            actions,
            module.heap().alloc_str("archive").to_value(),
            loader
                .0
                .get("configuration")
                .unwrap()
                .owned_value(module.frozen_heap()),
            loader
                .0
                .get("toolchain")
                .unwrap()
                .owned_value(module.frozen_heap()),
            module
                .heap()
                .alloc(starlark::values::dict::AllocDict(std::iter::empty::<(
                    Value,
                    Value,
                )>())),
            module.heap().alloc_str("CppArchive").to_value(),
            module.heap().alloc_str("runner").to_value(),
            module
                .heap()
                .alloc(starlark::values::list::AllocList([] as [Value; 0])),
            module
                .heap()
                .alloc(starlark::values::list::AllocList([] as [Value; 0])),
            Value::new_none(),
            Value::new_none(),
            Value::new_none(),
        ],
        &[],
    )
    .unwrap();
    module.get("admitted").unwrap().unpack_bool().unwrap()
}
#[test]
fn unchanged_global_resource_callback_authenticates_only_complete_unique_pinned_source() {
    let label = RetainedSpawnResourceSet::SOURCE_LABEL;
    let digest = RetainedSpawnResourceSet::SOURCE_DIGEST;
    assert_eq!(<[u8; 32]>::from(Sha256::digest(SOURCE.as_bytes())), digest);
    assert!(invoke(SOURCE, label, digest, 1));
    assert!(!invoke(
        SOURCE,
        "@@copied//cc/private/link:finalize_link_action.bzl",
        digest,
        1
    ));
    assert!(!invoke(SOURCE, label, [0; 32], 1));
    assert!(!invoke(SOURCE, label, digest, 0));
    assert!(!invoke(SOURCE, label, digest, 2));
    assert!(!invoke(&format!("{SOURCE}\n# changed\n"), label, digest, 1));
}
#[test]
fn resource_callback_wrong_callsite_and_native_callable_never_gain_authority() {
    let loader = Loader(freeze(STUB, None));
    let definition = freeze(SOURCE, Some(&loader));
    let module = Module::new();
    let p = provenance(
        RetainedSpawnResourceSet::SOURCE_LABEL,
        RetainedSpawnResourceSet::SOURCE_DIGEST,
    );
    let context = BzlEvaluationContext::macro_runtime_context(p.1.identity.clone(), Arc::from([p]));
    let globals = GlobalsBuilder::new().with(probe).build();
    module.set(
        "capture",
        globals
            .iter()
            .find(|(name, _)| *name == "capture")
            .unwrap()
            .1
            .to_value(),
    );
    module.set(
        "callback",
        definition
            .get_any_visibility("_resource_set")
            .unwrap()
            .0
            .owned_value(module.frozen_heap()),
    );
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    for callback in ["callback", "len"] {
        eval.eval_module(
            AstModule::parse(
                "ordinary.bzl",
                format!("capture(resource_set={callback})"),
                &Dialect::Bazel,
            )
            .unwrap(),
            &loading_globals(),
        )
        .unwrap();
        assert_eq!(module.get("admitted").unwrap().unpack_bool(), Some(false));
    }
}
