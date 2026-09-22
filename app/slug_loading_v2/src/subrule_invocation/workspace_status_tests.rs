use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::WorkspaceStatusArtifacts;
use slug_workspace_v2::NormalizedAbsolutePath;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;

fn pair(path: &str) -> WorkspaceStatusArtifacts {
    WorkspaceStatusArtifacts::new(NormalizedAbsolutePath::new(path).unwrap())
}

fn set(module: &Module, name: &str, value: AnalysisArtifact) {
    module.set(
        name,
        module
            .heap()
            .alloc_simple(AnalysisArtifactValue::new(value)),
    );
}

fn evaluate<'v>(module: &'v Module, source: &str) -> starlark::Result<Value<'v>> {
    Evaluator::new(module).eval_module(
        AstModule::parse("status.bzl", source.to_owned(), &Dialect::Bazel).unwrap(),
        &crate::package::loading_globals(),
    )
}

#[test]
fn workspace_status_file_fields_hash_and_depset_equality() {
    let module = Module::new();
    let artifacts = pair("/workspace");
    for (name, artifact) in [
        ("stable", artifacts.stable()),
        ("same", artifacts.stable()),
        ("volatile", artifacts.volatile()),
    ] {
        set(
            &module,
            name,
            AnalysisArtifact::WorkspaceStatus(artifact.clone()),
        );
    }
    set(
        &module,
        "other",
        AnalysisArtifact::WorkspaceStatus(pair("/other").stable().clone()),
    );
    evaluate(
        &module,
        r#"
asserts = [
    stable.path == 'bazel-out/stable-status.txt',
    stable.short_path == 'stable-status.txt',
    volatile.path == 'bazel-out/volatile-status.txt',
    volatile.short_path == 'volatile-status.txt',
    stable.basename == 'stable-status.txt', stable.dirname == 'bazel-out',
    stable.extension == 'txt', stable.root.path == 'bazel-out',
    stable.owner == None, stable.label == None,
    not stable.is_source, not stable.is_directory, not stable.is_symlink,
    stable == same, same == stable, stable != volatile, volatile != stable,
    stable != other, other != stable, stable.root == volatile.root,
    stable.root != other.root, {stable: 1}[same] == 1,
    len(depset([stable, same, volatile, other]).to_list()) == 3,
    {stable.root: 1}[volatile.root] == 1,
]
def check():
    if not all(asserts):
        fail(asserts)
check()
"#,
    )
    .unwrap();
    let frozen = module.freeze().unwrap();
    let restored = Module::new();
    restored.import_public_symbols(&frozen);
    evaluate(&restored, "def check_frozen():\n    if not ({stable: 1}[same] == 1 and stable.root == volatile.root):\n        fail('frozen identity')\ncheck_frozen()").unwrap();
    for source in [
        "stable.path = 'bad'",
        "stable.root.path = 'bad'",
        "stable()",
        "stable.root()",
    ] {
        assert!(evaluate(&restored, source).is_err(), "{source}");
    }
}

#[test]
fn workspace_status_cannot_be_claimed_as_declared_rule_output() {
    let owner = AnalysisConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//:rule").unwrap(),
        &b"config"[..],
    );
    let pair = pair("/workspace");
    for artifact in [pair.stable(), pair.volatile()] {
        let value = AnalysisArtifactValue::new(AnalysisArtifact::WorkspaceStatus(artifact.clone()));
        assert!(value.output_for_owner(&owner).is_none());
    }
}

#[test]
fn file_is_symlink_reflects_declared_kind_not_source_or_regular_file() {
    // FileApi.java100–106: declared symlink kind, not filesystem observation.
    let module = Module::new();
    let label = CanonicalLabel::parse("@@//:file").unwrap();
    set(&module, "source", AnalysisArtifact::Source(label.clone()));
    for (name, kind) in [
        ("regular", ActionOutputKind::File),
        ("symlink", ActionOutputKind::Symlink),
        ("tree", ActionOutputKind::Directory),
    ] {
        set(
            &module,
            name,
            AnalysisArtifact::Derived {
                owner: AnalysisConfiguredTargetKey::new(label.clone(), &b"config"[..]),
                output: ActionOutput::new(crate::artifact_root::test_root(), name, kind),
            },
        );
    }
    evaluate(&module, "def check():\n    if source.is_symlink or regular.is_symlink or tree.is_symlink or not symlink.is_symlink:\n        fail('declared symlink kind')\ncheck()").unwrap();
}
