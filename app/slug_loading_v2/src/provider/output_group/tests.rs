use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisConfiguredTargetKey;
use starlark::environment::FrozenModule;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::FrozenHeap;

use super::*;

fn evaluate(source: &str) -> Result<FrozenModule, String> {
    let module = Module::new();
    let source_file = AnalysisArtifact::Source(CanonicalLabel::parse("@@//pkg:input").unwrap());
    module.set(
        "F",
        module.heap().alloc(AnalysisArtifactValue::new(source_file)),
    );
    for (name, kind) in [
        ("FILE", ActionOutputKind::File),
        ("TREE", ActionOutputKind::Directory),
        ("LINK", ActionOutputKind::Symlink),
        ("RUNFILES", ActionOutputKind::RunfilesTree),
    ] {
        let owner = AnalysisConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//pkg:owner").unwrap(),
            vec![1],
        );
        module.set(
            name,
            module
                .heap()
                .alloc(AnalysisArtifactValue::new(AnalysisArtifact::Derived {
                    owner,
                    output: ActionOutput::new("pkg/same", kind),
                })),
        );
    }
    let ast = AstModule::parse("output_groups.bzl", source.to_owned(), &Dialect::Standard)
        .map_err(|error| error.to_string())?;
    Evaluator::new(&module)
        .eval_module(ast, &crate::package::loading_globals())
        .map_err(|error| error.to_string())?;
    module.freeze().map_err(|error| format!("{error:?}"))
}

#[test]
fn sequences_and_shared_depsets_preserve_artifacts_and_occurrences() {
    let module = evaluate(
        r#"
shared = depset([F, TREE], order = "preorder")
parent = depset([FILE], transitive = [shared], order = "preorder")
X = OutputGroupInfo(list = [F, F, TREE], tuple = (LINK, RUNFILES), shared = shared, parent = parent)
CHECK = [X.list.to_list() == [F, TREE], X.tuple.to_list() == [LINK, RUNFILES], X.shared == shared, X.parent == parent, X.parent.to_list() == [FILE, F, TREE]]
"#,
    ).unwrap();
    assert_eq!(
        module.get("CHECK").unwrap().to_string(),
        "[True, True, True, True, True]"
    );
    let output = module.get("X").unwrap();
    let fields = StarlarkOutputGroupInfo::fields_from_value(output.value()).unwrap();
    let list = fields.iter().find(|(name, _)| name == "list").unwrap().1;
    let (order, _, _, _, _, _) = StarlarkDepset::parts_from_value(list).unwrap();
    assert_eq!(order, DepsetOrder::Default);
    let tuple = fields.iter().find(|(name, _)| name == "tuple").unwrap().1;
    for value in StarlarkDepset::direct_from_value(tuple).unwrap() {
        assert!(AnalysisArtifactValue::from_starlark(value).is_some());
    }
}

#[test]
fn all_empty_canonicalizes_but_mixed_empty_retains_order_and_identity() {
    let module = evaluate(
        r#"
pre = depset(order = "preorder")
post = depset(order = "postorder")
X = OutputGroupInfo(pre = pre, post = post, seq = [])
Y = OutputGroupInfo(pre = pre, nonempty = [F])
CHECK = [X.pre == depset(), X.post == depset(), X.seq == depset(), Y.pre == pre, Y.pre != X.pre, "seq" in X, "absent" not in X]
"#,
    ).unwrap();
    assert_eq!(
        module.get("CHECK").unwrap().to_string(),
        "[True, True, True, True, True, True, True]"
    );
    let value = module.get("Y").unwrap();
    let fields = StarlarkOutputGroupInfo::fields_from_value(value.value()).unwrap();
    let pre = fields.iter().find(|(name, _)| name == "pre").unwrap().1;
    assert_eq!(
        StarlarkDepset::parts_from_value(pre).unwrap().0,
        DepsetOrder::Preorder
    );
}

#[test]
fn arbitrary_names_field_operations_and_utf16_iteration_order() {
    let module = evaluate(
        r#"
X = OutputGroupInfo(**{"\ue000": [], "\U00010000": [], "": [], "not-an-identifier": [], "_validation_transitive": [], "ordinary": []})
CHECK = [X.ordinary == X["ordinary"], X[""] == depset(), 1 not in X, "missing" not in X, hasattr(X, "ordinary"), not hasattr(X, "missing")]
ORDER = [name for name in X]
"#,
    ).unwrap();
    assert_eq!(
        module.get("CHECK").unwrap().to_string(),
        "[True, True, True, True, True, True]"
    );
    let output = module.get("X").unwrap();
    let expected = [
        "",
        "_validation_transitive",
        "not-an-identifier",
        "ordinary",
        "\u{10000}",
        "\u{e000}",
    ];
    // The shared Starlark runtime re-sorts dir() by Rust string order; only
    // provider field iteration uses the pinned Java UTF-16 ordering.
    let mut native_dir_order = expected;
    native_dir_order.sort();
    assert_eq!(output.value().dir_attr(), native_dir_order);
    assert_ne!(native_dir_order, expected);
    let order = module.get("ORDER").unwrap();
    assert_eq!(
        ListRef::from_value(order.value())
            .unwrap()
            .iter()
            .map(|value| value.unpack_str().unwrap())
            .collect::<Vec<_>>(),
        expected
    );
    for source in [
        "X = OutputGroupInfo()[1]",
        "X = OutputGroupInfo()[\"missing\"]",
        "X = OutputGroupInfo().missing",
    ] {
        assert!(evaluate(source).is_err(), "{source}");
    }
}

#[test]
fn invalid_values_are_rejected_at_construction_even_with_empty_siblings() {
    for expression in [
        "None",
        "'path'",
        "1",
        "{}",
        "['path']",
        "(F, 1)",
        "depset(['path'])",
    ] {
        let source = format!("X = OutputGroupInfo(empty = [], wrong = {expression})");
        let error = evaluate(&source).unwrap_err();
        assert!(error.contains("output group 'wrong'"), "{source}: {error}");
    }
    assert!(evaluate("X = OutputGroupInfo([])").is_err());
}

#[test]
fn fresh_frozen_and_rematerialized_values_share_equality_hash_and_operations() {
    let module = evaluate(
        r#"
D = depset([F, TREE])
X = OutputGroupInfo(z = D, empty = depset(order = "preorder"))
Y = OutputGroupInfo(empty = depset(order = "preorder"), z = D)
Z = OutputGroupInfo(z = depset([F, TREE]), empty = depset(order = "preorder"))
LOOKUP = {X: 7}[Y]
CHECK = [X == Y, X != Z, OutputGroupInfo(x = []) == OutputGroupInfo(x = depset(order = "preorder")), OutputGroupInfo() != OutputGroupInfo(x = []), OutputGroupInfo != X]
"#,
    ).unwrap();
    assert_eq!(module.get("LOOKUP").unwrap().to_string(), "7");
    assert_eq!(
        module.get("CHECK").unwrap().to_string(),
        "[True, True, True, True, True]"
    );
    let original = module.get("X").unwrap();
    let heap = FrozenHeap::new();
    let fields = StarlarkOutputGroupInfo::fields_from_value(original.value()).unwrap();
    let rematerialized = StarlarkOutputGroupInfo::alloc(
        &heap,
        fields
            .into_iter()
            .map(|(name, value)| (name, value.unpack_frozen().unwrap())),
    );
    assert!(original.value().equals(rematerialized.to_value()).unwrap());
    assert_eq!(
        original.value().get_hashed().unwrap().hash(),
        rematerialized.to_value().get_hashed().unwrap().hash()
    );
    let empty = alloc_starlark_depset(
        &heap,
        AnalysisDepset::empty(DepsetOrder::Preorder),
        Vec::new(),
    );
    let normalized = StarlarkOutputGroupInfo::alloc(&heap, [("x".into(), empty)]);
    let normalized_fields =
        StarlarkOutputGroupInfo::fields_from_value(normalized.to_value()).unwrap();
    assert_eq!(
        StarlarkDepset::parts_from_value(normalized_fields[0].1)
            .unwrap()
            .0,
        DepsetOrder::Default
    );
    let scratch = Module::new();
    scratch.set("X", rematerialized.to_value());
    let ast = AstModule::parse(
        "materialized.bzl",
        "CHECK = [X.z == X['z'], 'empty' in X, 1 not in X, [name for name in X] == ['empty', 'z']]"
            .to_owned(),
        &Dialect::Standard,
    )
    .unwrap();
    Evaluator::new(&scratch)
        .eval_module(ast, &crate::package::loading_globals())
        .unwrap();
    assert_eq!(
        scratch.get("CHECK").unwrap().to_repr(),
        "[True, True, True, True]"
    );
    let left = heap.alloc(OutputGroupInfo);
    let right = heap.alloc(OutputGroupInfo);
    assert!(left.to_value().equals(right.to_value()).unwrap());
    assert_eq!(
        left.to_value().get_hashed().unwrap().hash(),
        right.to_value().get_hashed().unwrap().hash()
    );
    assert!(
        starlark_provider_identity(left.to_value())
            .unwrap()
            .is_builtin("OutputGroupInfo")
    );
}
