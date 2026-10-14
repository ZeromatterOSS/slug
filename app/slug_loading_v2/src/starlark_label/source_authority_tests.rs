//! Source authority follows the actual function module, never a filename lookup.
use std::path::PathBuf;
use std::sync::Arc;

use dupe::Dupe;
use sha2::Digest;
use sha2::Sha256;
use starlark::environment::FrozenModule;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::eval::FileLoader;
use starlark::eval::inspect_def;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::Value;

use super::*;
use crate::BzlLoadManifest;
use crate::BzlModuleSourceProvenance;

fn manifest(
    label: &str,
    mapped: &str,
    source: &str,
    child: Option<&BzlLoadManifest>,
) -> BzlLoadManifest {
    let root = BzlModuleIdentity {
        label: CanonicalLabel::parse(label).unwrap(),
        // All evaluators deliberately share this path and CodeMap filename.
        workspace_path: PathBuf::from("/same/defs.bzl"),
        repository_mapping: Arc::from([(
            ApparentRepoName::new("alias").unwrap(),
            CanonicalRepoName::new(mapped).unwrap(),
        )]),
    };
    let digest: [u8; 32] = Sha256::digest(source.as_bytes()).into();
    let mut reachable = vec![BzlModuleSourceProvenance::new(root.clone(), digest)];
    if let Some(child) = child {
        reachable.extend(child.reachable.iter().cloned());
    }
    BzlLoadManifest {
        root,
        direct_children: child
            .map(|child| Arc::from([child.root.clone()]))
            .unwrap_or_else(|| Arc::from([])),
        reachable: reachable.into(),
        fingerprint: digest,
    }
}

struct Loader(FrozenModule);
impl FileLoader for Loader {
    fn load(&self, path: &str) -> starlark::Result<FrozenModule> {
        assert_eq!(path, "child.bzl");
        Ok(self.0.dupe())
    }
}

fn freeze(source: &str, manifest: &BzlLoadManifest, loader: Option<&Loader>) -> FrozenModule {
    let module = Module::new();
    attach_defining_module_manifest(&module, manifest).unwrap();
    let globals = GlobalsBuilder::standard().with(label_globals).build();
    let mut evaluator = Evaluator::new(&module);
    if let Some(loader) = loader {
        evaluator.set_loader(loader);
    }
    evaluator
        .eval_module(
            AstModule::parse("identical.bzl", source.to_owned(), &Dialect::Bazel).unwrap(),
            &globals,
        )
        .unwrap();
    drop(evaluator);
    module.freeze().unwrap()
}

fn authority(function: Value<'_>) -> &BzlLoadManifest {
    defining_module_source_manifest(
        inspect_def(function, None)
            .unwrap()
            .original_module_client_data()
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn source_authority_follows_frozen_imported_and_nested_original_modules_after_drop() {
    let leaf_source = r#"
def resolve(): return Label('@alias//pkg:item')
def factory():
    captured = "kept"
    def nested(): return [Label(':nested'), captured]
    return nested
"#;
    let leaf_manifest = manifest("@@leaf+//leaf:defs.bzl", "selected+", leaf_source, None);
    let leaf = freeze(leaf_source, &leaf_manifest, None);
    let middle_source =
        "load('child.bzl', 'resolve', 'factory')\ndef wrapper(): return resolve()\n";
    let middle_manifest = manifest(
        "@@middle+//middle:defs.bzl",
        "wrong+",
        middle_source,
        Some(&leaf_manifest),
    );
    let loader = Loader(leaf);
    let middle = freeze(middle_source, &middle_manifest, Some(&loader));
    let imported = middle.get("resolve").unwrap();
    let factory = middle.get("factory").unwrap();
    let wrapper = middle.get("wrapper").unwrap();
    assert_eq!(authority(imported.value()), &leaf_manifest);
    assert_eq!(authority(wrapper.value()), &middle_manifest);
    let scratch = Module::new();
    let caller_manifest = manifest("@@caller+//caller:defs.bzl", "caller+", "caller", None);
    attach_defining_module_manifest(&scratch, &caller_manifest).unwrap();
    let factory_value = factory.owned_value(scratch.frozen_heap());
    let nested = Evaluator::new(&scratch)
        .eval_function(factory_value, &[], &[])
        .unwrap();
    scratch.set("nested", nested);
    let nested_view = inspect_def(nested, Some(&scratch)).unwrap();
    let borrowed =
        defining_module_source_manifest(nested_view.original_module_client_data().unwrap())
            .unwrap();
    assert_eq!(borrowed, &leaf_manifest);
    assert!(Arc::ptr_eq(&borrowed.reachable, &leaf_manifest.reachable));
    assert!(Arc::ptr_eq(
        &borrowed.direct_children,
        &leaf_manifest.direct_children
    ));
    assert!(Arc::ptr_eq(
        &borrowed.root.repository_mapping,
        &leaf_manifest.root.repository_mapping
    ));
    assert!(std::ptr::eq(
        borrowed,
        defining_module_source_manifest(nested_view.original_module_client_data().unwrap())
            .unwrap()
    ));
    drop(loader);
    drop(middle);
    drop(factory);
    let frozen_nested = scratch.freeze().unwrap().get("nested").unwrap();
    assert_eq!(authority(frozen_nested.value()), &leaf_manifest);
    drop(leaf_manifest);
    drop(middle_manifest);
    drop(caller_manifest);
    let consumer = Module::new();
    let nested_value = frozen_nested.owned_value(consumer.frozen_heap());
    drop(frozen_nested);
    let authority = authority(nested_value);
    assert_eq!(authority.root.label.to_string(), "@@leaf+//leaf:defs.bzl");
    assert_eq!(authority.reachable.len(), 1);
    assert_eq!(
        Evaluator::new(&consumer)
            .eval_function(nested_value, &[], &[])
            .unwrap()
            .to_repr(),
        "[Label(\"@@leaf+//leaf:nested\"), \"kept\"]"
    );
    assert_eq!(
        Evaluator::new(&consumer)
            .eval_function(imported.value(), &[], &[])
            .unwrap()
            .to_string(),
        "@@selected+//pkg:item"
    );
    assert_eq!(
        Evaluator::new(&consumer)
            .eval_function(wrapper.value(), &[], &[])
            .unwrap()
            .to_string(),
        "@@selected+//pkg:item"
    );
}

#[test]
fn borrowed_source_and_repository_mapping_authority_changes_and_restores() {
    let first_source = "def resolve(): return ['first', Label('@alias//pkg:item')]\n";
    let changed_source = "def resolve(): return ['changed', Label('@alias//pkg:item')]\n";
    let first = manifest("@@owner+//pkg:defs.bzl", "mapped_a+", first_source, None);
    let source_changed = manifest("@@owner+//pkg:defs.bzl", "mapped_a+", changed_source, None);
    let mapping_changed = manifest("@@owner+//pkg:defs.bzl", "mapped_b+", first_source, None);
    let restored = manifest("@@owner+//pkg:defs.bzl", "mapped_a+", first_source, None);
    let functions = [
        freeze(first_source, &first, None).get("resolve").unwrap(),
        freeze(changed_source, &source_changed, None)
            .get("resolve")
            .unwrap(),
        freeze(first_source, &mapping_changed, None)
            .get("resolve")
            .unwrap(),
        freeze(first_source, &restored, None)
            .get("resolve")
            .unwrap(),
    ];
    assert_eq!(authority(functions[0].value()), &first);
    assert_ne!(
        authority(functions[0].value()),
        authority(functions[1].value())
    );
    assert_ne!(
        authority(functions[0].value()).reachable[0].source_digest,
        authority(functions[1].value()).reachable[0].source_digest
    );
    assert_ne!(
        authority(functions[0].value()),
        authority(functions[2].value())
    );
    assert_eq!(
        authority(functions[0].value()).reachable[0].source_digest,
        authority(functions[2].value()).reachable[0].source_digest
    );
    assert_eq!(
        authority(functions[0].value()),
        authority(functions[3].value())
    );
    let consumer = Module::new();
    for (function, expected) in functions.iter().zip([
        "[\"first\", Label(\"@@mapped_a+//pkg:item\")]",
        "[\"changed\", Label(\"@@mapped_a+//pkg:item\")]",
        "[\"first\", Label(\"@@mapped_b+//pkg:item\")]",
        "[\"first\", Label(\"@@mapped_a+//pkg:item\")]",
    ]) {
        assert_eq!(
            Evaluator::new(&consumer)
                .eval_function(function.value(), &[], &[])
                .unwrap()
                .to_repr(),
            expected
        );
    }
}

#[test]
fn identity_only_and_missing_manifest_remain_label_valid_without_authority_fallback() {
    let identity = manifest(
        "@@identity+//pkg:defs.bzl",
        "mapped+",
        "identity only",
        None,
    )
    .root;
    let module = Module::new();
    attach_defining_module_context(&module, &identity).unwrap();
    assert!(
        attach_defining_module_manifest(
            &module,
            &manifest("@@other+//pkg:defs.bzl", "wrong+", "other", None)
        )
        .is_err()
    );
    Evaluator::new(&module)
        .eval_module(
            AstModule::parse(
                "identical.bzl",
                "def resolve(): return Label('@alias//pkg:item')\n".to_owned(),
                &Dialect::Bazel,
            )
            .unwrap(),
            &GlobalsBuilder::standard().with(label_globals).build(),
        )
        .unwrap();
    let function = module.freeze().unwrap().get("resolve").unwrap();
    let scratch = Module::new();
    let misleading = manifest("@@caller+//wrong:defs.bzl", "wrong+", "misleading", None);
    attach_defining_module_manifest(&scratch, &misleading).unwrap();
    let original_metadata = inspect_def(function.value(), Some(&scratch))
        .unwrap()
        .original_module_client_data()
        .unwrap();
    assert!(defining_module_source_manifest(original_metadata).is_none());
    assert!(defining_module_source_manifest(Value::new_none()).is_none());
    assert_eq!(
        Evaluator::new(&scratch)
            .eval_function(function.value(), &[], &[])
            .unwrap()
            .to_string(),
        "@@mapped+//pkg:item"
    );
}
