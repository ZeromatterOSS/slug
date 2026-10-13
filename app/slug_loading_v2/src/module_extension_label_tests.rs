use std::path::PathBuf;

use dupe::Dupe;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use starlark::environment::FrozenModule;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::eval::FileLoader;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::Value;

use crate::bzl_module::BzlLoadManifest;
use crate::bzl_module::BzlModuleIdentity;
use crate::bzl_module::BzlModuleSourceProvenance;
use crate::module_extension_repository_rule::RepositoryRuleCallRecord;
use crate::module_extension_repository_rule::RepositoryRuleCallValue;
use crate::module_extension_repository_rule::RepositoryRuleInvocationState;
use crate::package::loading_globals;
use crate::provider::BzlEvaluationContext;
use crate::starlark_label::StarlarkLabel;

const ENTRY_PATH: &str = "/workspace/ext/ext.bzl";
const HELPER_PATH: &str = "/registry-producer/owner/defs.bzl";
const HELPER_LOAD: &str = "@producer//owner:defs.bzl";

const HELPER: &str = r#"def make_labels():
    local = Label(':local')
    return (local, Label('//third_party:overlay'), Label('@alias//mapped:target'), Label('@@direct+//canon:target'), Label(local))

def bad_mapping():
    return Label('@missing//pkg:target')
"#;

const ENTRY: &str = r#"load('@producer//owner:defs.bzl', 'make_labels', 'bad_mapping')
def _repo_impl(ctx): pass
repo = repository_rule(_repo_impl, attrs = {
    'relative': attr.label(), 'absolute': attr.label(), 'apparent': attr.label(),
    'canonical': attr.label(), 'same': attr.label(), 'caller': attr.label(),
})
def invoke(ctx):
    local, absolute, apparent, canonical, same = make_labels()
    repo(name = 'first', relative = local, absolute = absolute, apparent = apparent,
         canonical = canonical, same = same, caller = Label(':caller'))
    repo(name = 'second', relative = local)
def invalid(ctx):
    return bad_mapping()
def direct(ctx):
    return Label(':caller')
def unrelated_builtin(ctx):
    return provider()
"#;

struct HelperLoader(FrozenModule);

impl FileLoader for HelperLoader {
    fn load(&self, path: &str) -> starlark::Result<FrozenModule> {
        (path == HELPER_LOAD)
            .then(|| self.0.dupe())
            .ok_or_else(|| starlark::Error::new_other(anyhow::anyhow!("unexpected load {path}")))
    }
}

fn identity(label: &str, path: &str, mapping: &[(&str, &str)]) -> BzlModuleIdentity {
    BzlModuleIdentity {
        label: CanonicalLabel::parse(label).unwrap(),
        workspace_path: PathBuf::from(path),
        repository_mapping: mapping
            .iter()
            .map(|(apparent, canonical)| {
                (
                    ApparentRepoName::new(*apparent).unwrap(),
                    CanonicalRepoName::new(*canonical).unwrap(),
                )
            })
            .collect::<Vec<_>>()
            .into(),
    }
}

fn manifest_for(root: BzlModuleIdentity, children: &[BzlModuleIdentity]) -> BzlLoadManifest {
    BzlLoadManifest {
        root: root.clone(),
        direct_children: children.to_vec().into(),
        reachable: std::iter::once(root)
            .chain(children.iter().cloned())
            .map(|identity| BzlModuleSourceProvenance::new(identity, [0; 32]))
            .collect::<Vec<_>>()
            .into(),
        fingerprint: [0; 32],
    }
}

fn freeze(
    source: &str,
    context: &BzlLoadManifest,
    loader: Option<&dyn FileLoader>,
) -> FrozenModule {
    let path = context.root.workspace_path.to_str().unwrap();
    let ast = AstModule::parse(path, source.to_owned(), &Dialect::Bazel).unwrap();
    let module = Module::new();
    crate::starlark_label::attach_defining_module_context(&module, &context.root).unwrap();
    let bzl = BzlEvaluationContext::from_manifest(context);
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&bzl);
    if let Some(loader) = loader {
        eval.set_loader(loader);
    }
    eval.eval_module(ast, &loading_globals()).unwrap();
    drop(eval);
    module.freeze().unwrap()
}

fn loaded() -> (FrozenModule, BzlLoadManifest) {
    let helper = identity(
        "@@producer+//owner:defs.bzl",
        HELPER_PATH,
        &[("alias", "dep+")],
    );
    let entry = identity("@@//ext:ext.bzl", ENTRY_PATH, &[("producer", "producer+")]);
    let helper_module = freeze(HELPER, &manifest_for(helper.clone(), &[]), None);
    let entry_manifest = manifest_for(entry, &[helper]);
    let entry_module = freeze(ENTRY, &entry_manifest, Some(&HelperLoader(helper_module)));
    (entry_module, entry_manifest)
}

fn invoke(
    loaded: &FrozenModule,
    function: &str,
    state: &RepositoryRuleInvocationState,
) -> Result<String, String> {
    let callable = loaded.get(function).unwrap();
    let module = Module::new();
    let callable = callable.owned_value(module.frozen_heap());
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(state);
    let result = eval
        .eval_function(callable, &[Value::new_none()], &[])
        .map(|value| value.to_repr())
        .map_err(|error| error.to_string());
    drop(eval);
    result
}

fn label_kwarg(record: &RepositoryRuleCallRecord, name: &str) -> String {
    let value = record
        .kwargs
        .iter()
        .find_map(|(key, value)| (key.as_str() == name).then_some(value))
        .unwrap();
    let RepositoryRuleCallValue::Label(label) = value else {
        panic!("{name} did not retain a typed Label: {value:?}");
    };
    label.to_string()
}

#[test]
fn extension_label_uses_imported_function_source_mapping_and_records_typed_calls() {
    let (loaded, manifest) = loaded();
    let state = RepositoryRuleInvocationState::new().with_bzl_manifest(&manifest);
    assert_eq!(invoke(&loaded, "invoke", &state).unwrap(), "None");
    let records = state.records();
    assert_eq!(records.len(), 2);
    assert_eq!(
        [records[0].name.as_str(), records[1].name.as_str()],
        ["first", "second"]
    );
    assert_eq!(records[0].caller.file, ENTRY_PATH);
    for (name, expected) in [
        ("relative", "@@producer+//owner:local"),
        ("absolute", "@@producer+//third_party:overlay"),
        ("apparent", "@@dep+//mapped:target"),
        ("canonical", "@@direct+//canon:target"),
        ("same", "@@producer+//owner:local"),
        ("caller", "@@//ext:caller"),
    ] {
        assert_eq!(label_kwarg(&records[0], name), expected, "{name}");
    }
    assert_eq!(
        label_kwarg(&records[1], "relative"),
        "@@producer+//owner:local"
    );
}

#[test]
fn extension_label_uses_original_module_and_refuses_missing_mapping_or_context() {
    let (loaded, manifest) = loaded();
    let state = RepositoryRuleInvocationState::new().with_bzl_manifest(&manifest);
    let error = invoke(&loaded, "invalid", &state).unwrap_err();
    assert!(error.contains("not visible"), "{error}");
    assert!(state.records().is_empty());

    let unrelated = manifest_for(
        identity("@@//other:other.bzl", "/other/other.bzl", &[]),
        &[],
    );
    let state = RepositoryRuleInvocationState::new().with_bzl_manifest(&unrelated);
    assert_eq!(
        invoke(&loaded, "direct", &state).unwrap(),
        "Label(\"@@//ext:caller\")"
    );
    assert!(state.records().is_empty());

    let state = RepositoryRuleInvocationState::new().with_bzl_manifest(&manifest);
    let error = invoke(&loaded, "unrelated_builtin", &state).unwrap_err();
    assert!(
        error.contains("provider() may only be called in a .bzl module"),
        "{error}"
    );
    assert!(state.records().is_empty());

    let globals = loading_globals();
    let label = globals
        .iter()
        .find_map(|(name, value)| (name == "Label").then_some(value))
        .unwrap();
    let module = Module::new();
    let state = RepositoryRuleInvocationState::new();
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&state);
    let raw = eval.heap().alloc_str("//:no_frame").to_value();
    let error = eval
        .eval_function(label.to_value(), &[raw], &[])
        .unwrap_err();
    assert!(error.to_string().contains("Label() can only be used"));
    let existing = eval.heap().alloc_simple(StarlarkLabel::new(
        CanonicalLabel::parse("@@producer+//owner:existing").unwrap(),
    ));
    let passthrough = eval
        .eval_function(label.to_value(), &[existing], &[])
        .unwrap();
    assert!(existing.ptr_eq(passthrough));
    assert!(state.records().is_empty());
}
