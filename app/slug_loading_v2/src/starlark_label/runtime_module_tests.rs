use std::path::PathBuf;
use std::sync::Arc;

use dupe::Dupe;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use starlark::environment::FrozenModule;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::eval::FileLoader;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::list::ListRef;

use super::*;

fn identity(label: &str, mapped: &str) -> BzlModuleIdentity {
    BzlModuleIdentity {
        label: CanonicalLabel::parse(label).unwrap(),
        // Deliberately identical filenames do not imply identical module identity.
        source: slug_build_api_v2::ModuleSourceOrigin::Workspace(PathBuf::from("/same/defs.bzl")),
        repository_mapping: Arc::from([(
            ApparentRepoName::new("alias").unwrap(),
            CanonicalRepoName::new(mapped).unwrap(),
        )]),
    }
}

struct Loader(FrozenModule);
impl FileLoader for Loader {
    fn load(&self, path: &str) -> starlark::Result<FrozenModule> {
        assert_eq!(path, "child.bzl");
        Ok(self.0.dupe())
    }
}

fn freeze(
    source: &str,
    identity: Option<&BzlModuleIdentity>,
    loader: Option<&Loader>,
) -> FrozenModule {
    let module = Module::new();
    module.set_extra_value(module.heap().alloc(17));
    if let Some(identity) = identity {
        attach_defining_module_context(&module, identity).unwrap();
    }
    let globals = GlobalsBuilder::standard().with(label_globals).build();
    let mut eval = Evaluator::new(&module);
    if let Some(loader) = loader {
        eval.set_loader(loader);
    }
    eval.eval_module(
        AstModule::parse("identical.bzl", source.to_owned(), &Dialect::Bazel).unwrap(),
        &globals,
    )
    .unwrap();
    drop(eval);
    let frozen = module.freeze().unwrap();
    assert_eq!(
        frozen.extra_value().unwrap().to_value().unpack_i32(),
        Some(17)
    );
    frozen
}

#[test]
fn runtime_labels_follow_original_module_with_identical_filenames_and_first_class_constructor() {
    let source = "constructor = Label\ndef resolve(ctor):\n    local = ctor(':local')\n    return [local, ctor('//pkg:item'), ctor('@alias//mapped:item'), ctor(local)]\nINIT = resolve(Label)\n";
    let a = identity("@@owner_a+//a:defs.bzl", "mapped_a+");
    let b = identity("@@owner_b+//b:defs.bzl", "mapped_b+");
    let producer = freeze(source, Some(&a), None);
    let constructor = producer.get("constructor").unwrap();
    let first = producer.get("resolve").unwrap();
    let other = freeze(source, Some(&b), None);
    let second = other.get("resolve").unwrap();
    assert_eq!(
        other.get("INIT").unwrap().value().to_repr(),
        "[Label(\"@@owner_b+//b:local\"), Label(\"@@owner_b+//pkg:item\"), Label(\"@@mapped_b+//mapped:item\"), Label(\"@@owner_b+//b:local\")]"
    );
    drop(producer);
    drop(other);
    drop(a);
    drop(b);
    let scratch = Module::new();
    // Current evaluator metadata must not override a frozen callee's original module.
    attach_defining_module_context(&scratch, &identity("@@caller+//wrong:defs.bzl", "wrong+"))
        .unwrap();
    let mut eval = Evaluator::new(&scratch);
    for (function, repo, package, mapped) in [
        (&first, "owner_a+", "a", "mapped_a+"),
        (&second, "owner_b+", "b", "mapped_b+"),
        (&first, "owner_a+", "a", "mapped_a+"),
    ] {
        let values = eval
            .eval_function(function.value(), &[constructor.value()], &[])
            .unwrap();
        let values = ListRef::from_value(values).unwrap();
        assert_eq!(
            values.get(0).unwrap().to_string(),
            format!("@@{repo}//{package}:local")
        );
        assert_eq!(
            values.get(1).unwrap().to_string(),
            format!("@@{repo}//pkg:item")
        );
        assert_eq!(
            values.get(2).unwrap().to_string(),
            format!("@@{mapped}//mapped:item")
        );
        assert!(values.get(0).unwrap().ptr_eq(*values.get(3).unwrap()));
    }
}

#[test]
fn nested_imported_runtime_label_uses_innermost_module_and_missing_mapping_refuses() {
    let leaf = freeze(
        "def leaf(ctor): return str(ctor('@alias//pkg:item'))\n",
        Some(&identity("@@leaf+//leaf:defs.bzl", "selected+")),
        None,
    );
    let middle = freeze(
        "load('child.bzl', 'leaf')\ndef invoke(ctor): return leaf(ctor)\nconstructor = Label\n",
        Some(&identity("@@middle+//middle:defs.bzl", "wrong+")),
        Some(&Loader(leaf)),
    );
    let callable = middle.get("invoke").unwrap();
    let constructor = middle.get("constructor").unwrap();
    drop(middle);
    let scratch = Module::new();
    let mut eval = Evaluator::new(&scratch);
    assert_eq!(
        eval.eval_function(callable.value(), &[constructor.value()], &[])
            .unwrap()
            .unpack_str(),
        Some("@@selected+//pkg:item")
    );
    let mut missing = identity("@@missing+//pkg:defs.bzl", "unused+");
    missing.repository_mapping = Arc::from([]);
    let module = freeze(
        "def resolve(): return Label('@alias//pkg:item')\n",
        Some(&missing),
        None,
    );
    let callable = module
        .get("resolve")
        .unwrap()
        .owned_value(scratch.frozen_heap());
    let error = eval
        .eval_function(callable, &[], &[])
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("repository '@alias' is not visible from @@missing+//pkg:defs.bzl"),
        "{error}"
    );
}

#[test]
fn label_passthrough_and_missing_original_context_do_not_fall_back_to_current_module() {
    let scratch = Module::new();
    let label = scratch.heap().alloc_simple(StarlarkLabel::new(
        CanonicalLabel::parse("@@origin+//pkg:item").unwrap(),
    ));
    scratch.set("existing", label);
    let globals = GlobalsBuilder::standard().with(label_globals).build();
    let mut eval = Evaluator::new(&scratch);
    eval.eval_module(
        AstModule::parse(
            "BUILD.bazel",
            "result = Label(existing)\n".to_owned(),
            &Dialect::Bazel,
        )
        .unwrap(),
        &globals,
    )
    .unwrap();
    assert!(scratch.get("result").unwrap().ptr_eq(label));
    let error = eval
        .eval_module(
            AstModule::parse(
                "BUILD.bazel",
                "bad = Label(':direct')\n".to_owned(),
                &Dialect::Bazel,
            )
            .unwrap(),
            &globals,
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("Label() can only be used"), "{error}");
    drop(eval);
    let no_context = freeze("def resolve(): return Label(':missing')\n", None, None);
    let callable = no_context.get("resolve").unwrap();
    drop(no_context);
    attach_defining_module_context(&scratch, &identity("@@caller+//wrong:defs.bzl", "wrong+"))
        .unwrap();
    assert!(
        attach_defining_module_context(&scratch, &identity("@@other+//wrong:defs.bzl", "wrong+"))
            .is_err()
    );
    let error = Evaluator::new(&scratch)
        .eval_function(callable.value(), &[], &[])
        .unwrap_err()
        .to_string();
    assert!(error.contains("Label() can only be used"), "{error}");
}
