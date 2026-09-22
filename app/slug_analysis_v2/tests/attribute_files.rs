//! Configured File projections over real prerequisite providers, not execution.
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisValue;
use slug_configuration_v2::SlugConfigurationKind;

use super::*;

const DEFINITIONS: &str = r#"
Proof = provider(fields = ["files", "file", "executable", "label"])

def _group(ctx):
    return [DefaultInfo(files = depset(
        direct = [f for dep in ctx.attr.srcs for f in dep[DefaultInfo].files.to_list()],
        transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps], order = ctx.attr.order))]
group = rule(implementation = _group, attrs = {
    "srcs": attr.label_list(allow_files = True), "deps": attr.label_list(), "order": attr.string(default = "default"),
})

def _generated(ctx):
    out = ctx.actions.declare_file("generated.txt")
    tree = ctx.actions.declare_directory("tree")
    ctx.actions.write(out, "generated")
    ctx.actions.run(outputs = [tree], executable = "unexecuted-tree-tool")
    return [DefaultInfo(files = depset([out, tree]))]
generated = rule(implementation = _generated)

def _tree(ctx):
    return [DefaultInfo(files = depset([ctx.attr.dep[DefaultInfo].files.to_list()[1]]))]
tree = rule(implementation = _tree, attrs = {"dep": attr.label()})

def _tool(ctx):
    out = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.write(out, "inert executable", is_executable = True)
    return [DefaultInfo(files = depset([ctx.attr.data[DefaultInfo].files.to_list()[0]]),
        executable = out, runfiles = ctx.runfiles(files = ctx.attr.hidden[DefaultInfo].files.to_list()))]
tool = rule(implementation = _tool, executable = True, attrs = {
    "data": attr.label(allow_files = True), "hidden": attr.label(allow_files = True),
})

def _probe(ctx):
    files = ctx.files
    file = ctx.file
    if type(ctx.attr.source) != "Target" or ctx.attr.source.label.name != "a.txt":
        fail("ctx.attr no longer exposes the configured target")
    if files.optional != [] or file.optional != None or files.empty_list != []:
        fail("optional dependency projection changed")
    for name in ["unknown", "text", "out"]:
        if hasattr(files, name) or hasattr(file, name):
            fail("ineligible field leaked: " + name)
    for name in ["vector", "mixed", "plain", "string_keyed", "label_keyed", "list_dict", "generated", "target_tool", "exec_tool"]:
        if hasattr(file, name):
            fail("non-single attribute leaked: " + name)
    if file.source != files.source[0] or file.source != ctx.attr.source[DefaultInfo].files.to_list()[0]:
        fail("single artifact identity differs")
    if files.target_tool != files.exec_tool or files.target_tool != [file.source]:
        fail("runfiles or executable-only File leaked into default file projection")
    if ctx.executable.target_tool in files.target_tool or ctx.executable.exec_tool in files.exec_tool:
        fail("executable-only File leaked")
    if ctx.attr.mode == "append":
        files.vector.append(file.source)
    elif ctx.attr.mode == "assign":
        files.vector[0] = file.source
    elif ctx.attr.mode == "unknown":
        return [Proof(files = files.unknown)]
    return [Proof(files = files, file = file,
        executable = struct(target_tool = ctx.executable.target_tool, exec_tool = ctx.executable.exec_tool),
        label = ctx.attr.source.label)]

probe = rule(implementation = _probe, attrs = {
    "source": attr.label(allow_single_file = True),
    "tree": attr.label(allow_single_file = True), "mixed": attr.label_list(),
    "optional": attr.label(allow_single_file = True),
    "plain": attr.label(), "vector": attr.label_list(), "empty_list": attr.label_list(),
    "string_keyed": attr.string_keyed_label_dict(),
    "label_keyed": attr.label_keyed_string_dict(), "list_dict": attr.label_list_dict(),
    "generated": attr.label(), "exec_generated": attr.label(cfg = "exec"),
    "target_tool": attr.label(executable = True, cfg = "target"),
    "exec_tool": attr.label(executable = True, cfg = "exec"),
    "text": attr.string(), "mode": attr.string(), "out": attr.output(),
})

def _forward(ctx):
    captured = ctx.attr.dep[Proof]
    # Already frozen/published ordinary structs and immutable lists remain usable.
    if captured.file.source != captured.files.source[0]:
        fail("captured projection alias lost")
    if ctx.attr.mutate:
        captured.files.vector.append(captured.file.source)
    return [captured]
forward = rule(implementation = _forward, attrs = {"dep": attr.label(), "mutate": attr.bool()})

def _single(ctx):
    ctx.actions.write(ctx.actions.declare_file("should-not-exist"), "effect")
    fail("IMPLEMENTATION_ENTERED_BEFORE_CARDINALITY_CHECK")
single = rule(implementation = _single, attrs = {"dep": attr.label(allow_single_file = True)})

def _single_false(ctx):
    if ctx.attr.fail_if_entered:
        fail("IMPLEMENTATION_ENTERED_BEFORE_CARDINALITY_CHECK")
    if ctx.attr.dep == None:
        if ctx.file.dep != None or ctx.files.dep != []:
            fail("absent single-false dependency changed")
    elif ctx.file.dep != ctx.attr.dep[DefaultInfo].files.to_list()[0] or ctx.files.dep != [ctx.file.dep]:
        fail("single-false projection lost artifact identity")
    return [Proof(files = ctx.files, file = ctx.file)]
single_false = rule(implementation = _single_false, attrs = {
    "dep": attr.label(allow_single_file = False), "fail_if_entered": attr.bool(),
})
"#;

const BUILD: &str = r#"load(':defs.bzl', 'group', 'generated', 'tree', 'tool', 'probe', 'forward', 'single', 'single_false')
exports_files(['a.txt','b.txt','c.txt','d.txt','hidden.txt'])
group(name='leaf',srcs=['a.txt','b.txt'])
group(name='left',srcs=['c.txt'],deps=[':leaf'])
group(name='right',srcs=['d.txt'],deps=[':leaf'])
group(name='empty')
group(name='topo',srcs=['a.txt','b.txt'],order='topological')
group(name='conly',srcs=['c.txt'])
group(name='pre',srcs=['a.txt'],deps=[':conly'],order='preorder')
generated(name='generated')
tree(name='tree',dep=':generated')
tool(name='tool',data='a.txt',hidden='hidden.txt')
probe(name='probe',source='a.txt',tree=':tree',mixed=[':topo',':pre'],plain=':left',vector=[':left',':right'],
    string_keyed={'right':':right','left':':left'},
    label_keyed={':left':'left',':right':'right'},
    list_dict={'right':[':right'],'left':[':left']},
    generated=':generated',exec_generated=':generated',target_tool=':tool',exec_tool=':tool')
forward(name='forward',dep=':probe')
forward(name='frozen_mutation',dep=':probe',mutate=True)
single(name='zero',dep=':empty')
single(name='multiple',dep=':leaf')
single_false(name='false_absent')
single_false(name='false_one',dep=':conly')
single_false(name='false_zero',dep=':empty',fail_if_entered=True)
single_false(name='false_multiple',dep=':leaf',fail_if_entered=True)
"#;

fn workspace() -> PathBuf {
    let root = scratch();
    fs::write(root.join("MODULE.bazel"), "module(name='root')\n").unwrap();
    fs::write(root.join("defs.bzl"), DEFINITIONS).unwrap();
    fs::write(root.join("BUILD.bazel"), BUILD).unwrap();
    for file in ["a.txt", "b.txt", "c.txt", "d.txt", "hidden.txt"] {
        fs::write(root.join(file), "source\n").unwrap();
    }
    root
}

async fn request(
    dice: &Arc<Dice>,
    root: &std::path::Path,
    target: &str,
    configuration: ConfigurationKey,
) -> Result<Arc<ConfiguredNodeResult>, String> {
    root_target_request_with_configuration(
        dice,
        root,
        &format!("@@//:{target}"),
        configuration,
        Arc::new(RootActivationTracker::default()),
    )
    .await
}

fn field<'a>(result: &'a ConfiguredNodeResult, name: &str) -> &'a AnalysisValue {
    result
        .providers()
        .occurrences()
        .find_map(|provider| provider.field(name))
        .unwrap()
}
fn member<'a>(value: &'a AnalysisValue, name: &str) -> &'a AnalysisValue {
    let AnalysisValueKind::Struct(fields) = value.kind() else {
        panic!("captured struct")
    };
    fields.get(name).unwrap()
}
fn artifacts(value: &AnalysisValue) -> Vec<&AnalysisArtifact> {
    let AnalysisValueKind::List(values) = value.kind() else {
        panic!("immutable File list")
    };
    values
        .iter()
        .map(|v| match v.kind() {
            AnalysisValueKind::Artifact(file) => file,
            _ => panic!("File"),
        })
        .collect()
}
fn paths(value: &AnalysisValue) -> Vec<String> {
    artifacts(value)
        .into_iter()
        .map(|f| f.path().into_owned())
        .collect()
}
fn check(result: &ConfiguredNodeResult, changed: bool) {
    assert!(result.actions().is_empty());
    let files = field(result, "files");
    let source = member(field(result, "file"), "source");
    assert!(matches!(
        source.kind(),
        AnalysisValueKind::Artifact(AnalysisArtifact::Source(_))
    ));
    assert!(matches!(
        member(field(result, "file"), "optional").kind(),
        AnalysisValueKind::None
    ));
    let first = if changed { "d.txt" } else { "b.txt" };
    // Native stable parent traverses a topological child's stored reverse
    // order, not that child's independently flattened toList result.
    assert_eq!(paths(member(files, "mixed")), ["b.txt", "a.txt", "c.txt"]);
    let AnalysisValueKind::Artifact(AnalysisArtifact::Derived { output, .. }) =
        member(field(result, "file"), "tree").kind()
    else {
        panic!("singleton Directory File")
    };
    assert_eq!(output.kind(), ActionOutputKind::Directory);
    assert_eq!(paths(member(files, "plain")), ["a.txt", first, "c.txt"]);
    for name in ["vector", "label_keyed"] {
        let expected = if changed {
            vec!["a.txt", "d.txt", "c.txt"]
        } else {
            vec!["a.txt", "b.txt", "c.txt", "d.txt"]
        };
        assert_eq!(paths(member(files, name)), expected, "{name}");
    }
    for name in ["string_keyed", "list_dict"] {
        let expected = if changed {
            vec!["a.txt", "d.txt", "c.txt"]
        } else {
            vec!["a.txt", "b.txt", "d.txt", "c.txt"]
        };
        assert_eq!(paths(member(files, name)), expected, "{name}");
    }
    for (name, kind) in [
        ("generated", SlugConfigurationKind::Target),
        ("exec_generated", SlugConfigurationKind::Exec),
    ] {
        let files = artifacts(member(files, name));
        assert_eq!(files.len(), 2);
        for (file, output_kind) in files
            .iter()
            .zip([ActionOutputKind::File, ActionOutputKind::Directory])
        {
            let AnalysisArtifact::Derived { owner, output } = file else {
                panic!("generated File")
            };
            assert_eq!(owner.label().to_string(), "@@//:generated");
            assert_eq!(output.root().configuration().kind(), kind);
            assert_eq!(output.kind(), output_kind);
        }
    }
    assert_ne!(member(files, "generated"), member(files, "exec_generated"));
    assert_ne!(
        member(field(result, "executable"), "target_tool"),
        member(field(result, "executable"), "exec_tool")
    );
    assert_eq!(paths(member(files, "target_tool")), ["a.txt"]);
    assert_eq!(paths(member(files, "exec_tool")), ["a.txt"]);
}

#[tokio::test]
async fn configured_attribute_file_collections_preserve_shapes_order_roots_and_published_capture() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let direct = request(&dice, &root, "probe", typed_action_test_configuration())
        .await
        .unwrap();
    check(&direct, false);
    let forwarded = request(&dice, &root, "forward", typed_action_test_configuration())
        .await
        .unwrap();
    check(&forwarded, false);
    for name in ["files", "file", "executable", "label"] {
        assert_eq!(field(&direct, name), field(&forwarded, name));
    }
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn explicit_single_file_false_keeps_single_artifact_presence_and_cardinality() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let absent = request(
        &dice,
        &root,
        "false_absent",
        typed_action_test_configuration(),
    )
    .await
    .unwrap();
    assert!(matches!(
        member(field(&absent, "file"), "dep").kind(),
        AnalysisValueKind::None
    ));
    assert!(paths(member(field(&absent, "files"), "dep")).is_empty());
    let single = request(&dice, &root, "false_one", typed_action_test_configuration())
        .await
        .unwrap();
    assert_eq!(paths(member(field(&single, "files"), "dep")), ["c.txt"]);
    assert!(matches!(
        member(field(&single, "file"), "dep").kind(),
        AnalysisValueKind::Artifact(_)
    ));
    for name in ["false_zero", "false_multiple"] {
        let error = request(&dice, &root, name, typed_action_test_configuration())
            .await
            .err()
            .expect("present dependency needs exactly one File");
        assert!(
            error.contains("ctx.file.dep requires exactly one File"),
            "{error}"
        );
        assert!(
            !error.contains("IMPLEMENTATION_ENTERED_BEFORE_CARDINALITY_CHECK"),
            "{error}"
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn configured_attribute_file_collections_restore_changes_and_missing_sources_in_one_dice() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let configuration = typed_action_test_configuration();
    let first = request(&dice, &root, "forward", configuration.clone())
        .await
        .unwrap();
    check(&first, false);
    fs::write(
        root.join("BUILD.bazel"),
        BUILD.replace(
            "name='leaf',srcs=['a.txt','b.txt']",
            "name='leaf',srcs=['a.txt','d.txt']",
        ),
    )
    .unwrap();
    let changed = request(&dice, &root, "forward", configuration.clone())
        .await
        .unwrap();
    check(&changed, true);
    assert_ne!(first, changed);
    fs::write(root.join("BUILD.bazel"), BUILD).unwrap();
    let restored = request(&dice, &root, "forward", configuration.clone())
        .await
        .unwrap();
    assert_eq!(first, restored);
    check(&restored, false);
    let configured = request(
        &dice,
        &root,
        "forward",
        super::cc_configure_features::config(&[(NativeCommandOption::Copt, "-DATTRIBUTE_INPUT")]),
    )
    .await
    .unwrap();
    check(&configured, false);
    assert_ne!(first, configured);
    assert_eq!(
        member(field(&first, "file"), "source"),
        member(field(&configured, "file"), "source")
    );
    assert_ne!(
        member(field(&first, "files"), "generated"),
        member(field(&configured, "files"), "generated")
    );
    let configuration_restored = request(&dice, &root, "forward", configuration.clone())
        .await
        .unwrap();
    assert_eq!(first, configuration_restored);
    check(&configuration_restored, false);
    fs::remove_file(root.join("b.txt")).unwrap();
    let error = request(&dice, &root, "forward", configuration.clone())
        .await
        .err()
        .expect("missing source rejected");
    assert!(error.contains("b.txt"), "{error}");
    fs::write(root.join("b.txt"), "source\n").unwrap();
    let recreated = request(&dice, &root, "forward", configuration)
        .await
        .unwrap();
    assert_eq!(first, recreated);
    check(&recreated, false);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn configured_attribute_file_collections_reject_mutation_and_single_cardinality_before_impl()
{
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    for target in ["zero", "multiple"] {
        let error = request(&dice, &root, target, typed_action_test_configuration())
            .await
            .err()
            .expect("single cardinality rejected");
        assert!(error.contains("one") || error.contains("single"), "{error}");
        assert!(
            !error.contains("IMPLEMENTATION_ENTERED_BEFORE_CARDINALITY_CHECK"),
            "{error}"
        );
    }
    let error = request(
        &dice,
        &root,
        "frozen_mutation",
        typed_action_test_configuration(),
    )
    .await
    .err()
    .expect("frozen list cannot mutate");
    assert!(
        error.to_ascii_lowercase().contains("immutable") || error.contains("frozen"),
        "{error}"
    );
    for mode in ["append", "assign", "unknown"] {
        fs::write(
            root.join("BUILD.bazel"),
            BUILD.replace("name='probe',", &format!("name='probe',mode='{mode}',")),
        )
        .unwrap();
        let error = request(&dice, &root, "probe", typed_action_test_configuration())
            .await
            .err()
            .expect("invalid projection operation rejected");
        assert!(
            if mode == "unknown" {
                error.contains("unknown")
            } else {
                error.to_ascii_lowercase().contains("immutable") || error.contains("frozen")
            },
            "{error}"
        );
    }
    fs::write(root.join("BUILD.bazel"), BUILD).unwrap();
    check(
        &request(&dice, &root, "probe", typed_action_test_configuration())
            .await
            .unwrap(),
        false,
    );
    fs::remove_dir_all(root).unwrap();
}
