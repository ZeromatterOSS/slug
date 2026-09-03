use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use dice::DetectCycles;
use dice::Dice;
use dice::UserComputationData;
use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_bzlmod_v2::inject_root_module_request_inputs;
use slug_loading_v2::AttributeKind;
use slug_loading_v2::BzlModuleEvaluator;
use slug_loading_v2::ConfiguredDependencyDefault;
use slug_loading_v2::PackageTargetKind;
use slug_loading_v2::bzl_load_cycle_detector;
use slug_loading_v2::keys::WorkspaceDirectoryEntry;
use slug_loading_v2::keys::WorkspaceDirectoryEntryKind;
use slug_loading_v2::keys::WorkspaceDirectorySnapshot;
use slug_loading_v2::keys::WorkspaceDirectorySnapshotKey;
use slug_loading_v2::keys::WorkspaceDirectoryValue;
use slug_loading_v2::keys::WorkspaceFileValue;
use slug_loading_v2::keys::WorkspaceSnapshot;
use slug_loading_v2::keys::WorkspaceSnapshotKey;
use slug_workspace_v2::WorkspaceRawFileValue;
use slug_workspace_v2::WorkspaceRawSnapshot;
use slug_workspace_v2::WorkspaceRawSnapshotKey;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::values::Value;

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("slug-subrule-{name}-{nanos}"));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("MODULE.bazel"), "").unwrap();
    root
}

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

fn directory_snapshot(root: &Path) -> WorkspaceDirectorySnapshot {
    let mut pending = vec![root.to_path_buf()];
    let mut directories = Vec::new();
    while let Some(directory) = pending.pop() {
        let mut entries = Vec::new();
        for entry in fs::read_dir(&directory).unwrap() {
            let entry = entry.unwrap();
            let file_type = entry.file_type().unwrap();
            let kind = if file_type.is_file() {
                WorkspaceDirectoryEntryKind::RegularFile
            } else if file_type.is_dir() {
                pending.push(entry.path());
                WorkspaceDirectoryEntryKind::Directory
            } else if file_type.is_symlink() {
                WorkspaceDirectoryEntryKind::Symlink
            } else {
                WorkspaceDirectoryEntryKind::Other
            };
            entries.push(WorkspaceDirectoryEntry {
                name: entry.file_name().to_str().unwrap().into(),
                kind,
            });
        }
        directories.push((directory, WorkspaceDirectoryValue::present(entries)));
    }
    WorkspaceDirectorySnapshot {
        directories: Arc::new(directories.into_iter().collect()),
    }
}

fn snapshot(root: &Path) -> Arc<WorkspaceSnapshot> {
    fn visit(directory: &Path, files: &mut Vec<(PathBuf, WorkspaceFileValue)>) {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                visit(&entry.path(), files);
            } else if entry.file_type().unwrap().is_file() {
                files.push((
                    entry.path(),
                    WorkspaceFileValue::Present(Arc::new(
                        fs::read_to_string(entry.path()).unwrap(),
                    )),
                ));
            }
        }
    }
    let mut files = Vec::new();
    visit(root, &mut files);
    Arc::new(WorkspaceSnapshot {
        files: Arc::new(files.into_iter().collect()),
    })
}

fn raw_snapshot(text: &WorkspaceSnapshot) -> Arc<WorkspaceRawSnapshot> {
    Arc::new(WorkspaceRawSnapshot {
        files: Arc::new(
            text.files
                .iter()
                .map(|(path, value)| {
                    let value = match value {
                        WorkspaceFileValue::Present(source) => {
                            WorkspaceRawFileValue::Present(Arc::from(source.as_bytes()))
                        }
                        WorkspaceFileValue::Absent => WorkspaceRawFileValue::Absent,
                        WorkspaceFileValue::ReadError(error) => {
                            WorkspaceRawFileValue::ReadError(error.clone())
                        }
                    };
                    (path.clone(), value)
                })
                .collect(),
        ),
    })
}

fn try_load_package(
    workspace: &Path,
    package: &Path,
) -> anyhow::Result<slug_loading_v2::LegacyLoadedPackage> {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let text = snapshot(workspace);
    let raw = raw_snapshot(&text);
    let directories = Arc::new(directory_snapshot(workspace));
    let evaluator = BzlModuleEvaluator::new(workspace)?;
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async move {
            let mut data = UserComputationData::default();
            data.cycle_detector = Some(bzl_load_cycle_detector());
            let mut updater = dice.updater_with_data(data);
            updater.changed_to(vec![(
                WorkspaceSnapshotKey {
                    workspace: workspace.to_path_buf(),
                },
                text,
            )])?;
            updater.changed_to(vec![(
                WorkspaceRawSnapshotKey {
                    workspace: workspace.to_path_buf(),
                },
                raw,
            )])?;
            updater.changed_to(vec![(
                WorkspaceDirectorySnapshotKey {
                    workspace: workspace.to_path_buf(),
                },
                directories,
            )])?;
            inject_root_module_request_inputs(
                &mut updater,
                workspace,
                BzlmodCommandPolicyKey::from_flags(None, false).map_err(anyhow::Error::msg)?,
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None)
                    .map_err(anyhow::Error::msg)?,
                LockfileMode::Update,
            )?;
            let mut transaction = updater.commit().await;
            evaluator.evaluate_package(&mut transaction, package).await
        })
}

fn load_package(workspace: &Path, package: &Path) -> slug_loading_v2::LegacyLoadedPackage {
    try_load_package(workspace, package).unwrap()
}

fn starlark_rule<'a>(
    package: &'a slug_loading_v2::LegacyLoadedPackage,
    name: &str,
) -> &'a slug_loading_v2::package::StarlarkRuleImplementation {
    let target = package
        .targets
        .iter()
        .find(|target| target.name == name)
        .unwrap();
    let PackageTargetKind::StarlarkRule(rule) = &target.kind else {
        panic!("{name} must be a Starlark rule")
    };
    rule
}

#[test]
fn subrule_and_configuration_field_are_bzl_only_and_freeze_exact_hidden_names() {
    let workspace = scratch("globals");
    let package = workspace.join("pkg");
    write(
        &package.join("field.bzl"),
        "FIELD = configuration_field(fragment = 'cpp', name = 'fdo_optimize')\n",
    );
    write(
        &package.join("defs.bzl"),
        r#"
load(":field.bzl", "FIELD")
def _check_fields():
    if FIELD != configuration_field(fragment = "cpp", name = "fdo_optimize"):
        fail("same typed producer must compare equal across modules")
    if FIELD == configuration_field(fragment = "cpp", name = "fdo_profile"):
        fail("field identity must discriminate")

_check_fields()

def _subrule_impl(ctx, _literal, _late):
    fail("loading must not invoke the subrule")

def _make_subrule():
    candidate = subrule(
        implementation = _subrule_impl,
        attrs = {
            "_literal": attr.label(default = "//tools:literal"),
            "_late": attr.label(default = FIELD),
        },
    )
    distinct = subrule(implementation = _subrule_impl)
    if candidate != candidate:
        fail("a transient subrule must equal itself")
    if candidate == distinct:
        fail("distinct transient subrules must not compare equal")
    return candidate

my_subrule = _make_subrule()

def check_exported_subrule():
    if str(my_subrule) != "<subrule my_subrule>":
        fail("exported repr changed: " + str(my_subrule))

def _rule_impl(ctx):
    return []

my_rule = rule(implementation = _rule_impl, subrules = [my_subrule])
"#,
    );
    write(
        &package.join("BUILD.bazel"),
        "load(':defs.bzl', 'check_exported_subrule', 'my_rule')\ncheck_exported_subrule()\nmy_rule(name = 'subject')\n",
    );

    let loaded = load_package(&workspace, &package);
    let rule = starlark_rule(&loaded, "subject");
    assert_eq!(rule.attached_subrule_count(), 1);
    assert_eq!(
        rule.subrule_hidden_attribute_names().collect::<Vec<_>>(),
        [
            "$//pkg:defs.bzl%my_subrule%_literal",
            "://pkg:defs.bzl%my_subrule%_late",
        ]
    );
    assert_eq!(
        rule.subrule_attribute_spans().collect::<Vec<_>>(),
        [("my_subrule", 0, 2)]
    );
    let configured = rule.configured_dependency_attributes().collect::<Vec<_>>();
    assert_eq!(configured.len(), 2);
    assert_eq!(configured[0].user_name(), Some("_literal"));
    assert_eq!(configured[0].kind(), AttributeKind::Label);
    assert!(configured[0].is_hidden());
    let ConfiguredDependencyDefault::Literal(literal) = configured[0].default() else {
        panic!("literal hidden row lost its retained default")
    };
    let mut labels = Vec::new();
    literal.labels(&mut labels);
    assert_eq!(labels[0].to_string(), "@@//tools:literal");
    assert_eq!(configured[1].user_name(), Some("_late"));
    let ConfiguredDependencyDefault::ConfigurationField(field) = configured[1].default() else {
        panic!("late-bound hidden row lost its typed default")
    };
    assert_eq!(field.field().fragment_name(), "cpp");
    assert_eq!(field.field().field_name(), "fdo_optimize");

    let build_only = workspace.join("build_only");
    write(
        &build_only.join("BUILD.bazel"),
        "subrule(implementation = lambda ctx: None)\n",
    );
    let error = try_load_package(&workspace, &build_only)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("subrule") && error.contains("not found"),
        "{error}"
    );
}

#[test]
fn coverage_cpp_and_selected_java_fields_freeze_for_private_labels() {
    let workspace = scratch("coverage-field");
    let package = workspace.join("pkg");
    write(
        &package.join("defs.bzl"),
        r#"
# rules_java 9.1.0 java/bazel/rules/bazel_java_test.bzl SHA-256:
# 33b1b5e205c6658c661be6b0cd1b30fe0339d78f0b4fbc061a350024a412f412
COVERAGE = configuration_field(fragment = "coverage", name = "output_generator")
def _check():
    if COVERAGE != configuration_field(fragment = "coverage", name = "output_generator"):
        fail("coverage producer identity changed")
    if COVERAGE == configuration_field(fragment = "cpp", name = "fdo_profile"):
        fail("fragment identity collapsed")
_check()

def _sub_impl(ctx, _coverage): return None
coverage_sub = subrule(
    implementation = _sub_impl,
    attrs = {"_coverage": attr.label(default = COVERAGE, cfg = "exec", executable = True)},
)
def _impl(ctx): return []
coverage_rule = rule(
    implementation = _impl,
    attrs = {"_coverage": attr.label(default = COVERAGE)},
    subrules = [coverage_sub],
)

CUSTOM_MALLOC = configuration_field(fragment = "cpp", name = "custom_malloc")
def _malloc_sub_impl(ctx, _malloc): return None
malloc_sub = subrule(
    implementation = _malloc_sub_impl,
    attrs = {"_malloc": attr.label(default = CUSTOM_MALLOC)},
)
malloc_rule = rule(
    implementation = _impl,
    attrs = {"_malloc": attr.label(default = CUSTOM_MALLOC)},
    subrules = [malloc_sub],
)

# rules_java 9.1.0 java/common/rules/java_toolchain.bzl SHA-256:
# 5ad6511cdef925246961c7e7a9039475c192371fedbf909c63cf92334779e875
JAVA_BYTECODE = configuration_field(fragment = "java", name = "java_toolchain_bytecode_optimizer")
JAVA_LOCAL = configuration_field("java", "local_java_optimization_configuration")
def _check_java():
    if JAVA_BYTECODE == JAVA_LOCAL: fail("selected Java fields collapsed")
_check_java()
def _java_impl(ctx): fail("Java toolchain implementation ran during loading")
def _java_initializer(**kwargs): fail("selected Java initializer ran during declaration")
JAVA_ATTRS = {
    "_bytecode_optimizer": attr.label(default = JAVA_BYTECODE, cfg = "exec", executable = True),
    "_local_java_optimization_configuration": attr.label(default = JAVA_LOCAL, cfg = "exec", allow_files = True),
}
java_toolchain = rule(implementation = _java_impl, initializer = _java_initializer, attrs = JAVA_ATTRS)
java_no_initializer_rule = rule(implementation = _java_impl, attrs = JAVA_ATTRS)
"#,
    );
    write(
        &package.join("java_export.bzl"),
        "load(':defs.bzl', 'java_no_initializer_rule', 'java_toolchain')\nexported_java_no_initializer_rule = java_no_initializer_rule\nexported_java_toolchain = java_toolchain\n",
    );
    write(
        &package.join("BUILD.bazel"),
        "load(':defs.bzl', 'coverage_rule', 'malloc_rule')\nload(':java_export.bzl', 'exported_java_no_initializer_rule', 'exported_java_toolchain')\ncoverage_rule(name = 'subject')\nmalloc_rule(name = 'malloc')\nexported_java_no_initializer_rule(name = 'java_no_initializer')\n",
    );

    let loaded = load_package(&workspace, &package);
    assert!(
        loaded
            .targets
            .iter()
            .all(|target| target.name != "java_toolchain")
    );
    let rule = starlark_rule(&loaded, "subject");
    let configured = rule.configured_dependency_attributes().collect::<Vec<_>>();
    assert_eq!(configured.len(), 2);
    for attribute in configured {
        let ConfiguredDependencyDefault::ConfigurationField(identity) = attribute.default() else {
            panic!("coverage dependency lost typed field identity")
        };
        assert_eq!(identity.field().fragment_name(), "coverage");
        assert_eq!(identity.field().field_name(), "output_generator");
    }
    let lifted = rule
        .configured_dependency_attributes()
        .find(|attribute| attribute.user_name() == Some("_coverage"))
        .unwrap();
    assert!(lifted.exec_configuration());
    assert!(lifted.executable());

    let malloc = starlark_rule(&loaded, "malloc");
    let configured = malloc
        .configured_dependency_attributes()
        .collect::<Vec<_>>();
    assert_eq!(configured.len(), 2);
    for attribute in configured {
        let ConfiguredDependencyDefault::ConfigurationField(identity) = attribute.default() else {
            panic!("custom malloc dependency lost typed field identity")
        };
        assert_eq!(identity.field().fragment_name(), "cpp");
        assert_eq!(identity.field().field_name(), "custom_malloc");
    }

    let java = starlark_rule(&loaded, "java_no_initializer");
    assert_eq!(
        java.late_bound_rule_attributes().collect::<Vec<_>>(),
        [
            ("_bytecode_optimizer", "java_toolchain_bytecode_optimizer"),
            (
                "_local_java_optimization_configuration",
                "local_java_optimization_configuration",
            ),
        ]
    );
    let java = java.configured_dependency_attributes().collect::<Vec<_>>();
    assert_eq!(java.len(), 2);
    assert!(java[0].exec_configuration() && java[0].executable());
    assert!(java[1].exec_configuration());
    assert!(java[1].file_admissibility().is_any_file());
    for attribute in java {
        let ConfiguredDependencyDefault::ConfigurationField(identity) = attribute.default() else {
            panic!("selected Java dependency lost typed field identity")
        };
        assert_eq!(identity.field().fragment_name(), "java");
        assert_eq!(identity.tools_repository().as_str(), "bazel_tools");
    }

    for (name, source, expected) in [
        (
            "unknown-coverage-field",
            "X = configuration_field(fragment = 'coverage', name = 'coverage_report_generator')\n",
            "invalid configuration field name 'coverage_report_generator' on fragment 'coverage'",
        ),
        (
            "public-attribute",
            "X = configuration_field(fragment = 'coverage', name = 'output_generator')\ndef impl(ctx): return []\nbad = rule(implementation = impl, attrs = {'coverage': attr.label(default = X)})\n",
            "the attribute must be private (i.e. start with '_'). Found 'coverage'",
        ),
        (
            "public-cpp-attribute",
            "X = configuration_field(fragment = 'cpp', name = 'fdo_profile')\ndef impl(ctx): return []\nbad = rule(implementation = impl, attrs = {'profile': attr.label(default = X)})\n",
            "the attribute must be private (i.e. start with '_'). Found 'profile'",
        ),
        (
            "non-label",
            "X = configuration_field(fragment = 'coverage', name = 'output_generator')\ndef impl(ctx): return []\nbad = rule(implementation = impl, attrs = {'_coverage': attr.string(default = X)})\n",
            "configuration_field may only be the default of attr.label",
        ),
    ] {
        let package = workspace.join(name);
        write(&package.join("defs.bzl"), source);
        write(&package.join("BUILD.bazel"), "load(':defs.bzl', 'bad')\n");
        let error = try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{name}: {error}");
    }

    for field in ["launcher", "proguard_top", "bytecode_optimizer", "missing"] {
        let package = workspace.join(field);
        write(
            &package.join("defs.bzl"),
            &format!("X = configuration_field(fragment = 'java', name = '{field}')\n"),
        );
        write(&package.join("BUILD.bazel"), "load(':defs.bzl', 'X')\n");
        let error = try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(&format!(
                "invalid configuration field name '{field}' on fragment 'java'"
            )),
            "{error}"
        );
    }

    let build_only = workspace.join("build-field");
    write(
        &build_only.join("BUILD.bazel"),
        "X = configuration_field(fragment = 'coverage', name = 'output_generator')\n",
    );
    let error = try_load_package(&workspace, &build_only)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("configuration_field") && error.contains("not found"),
        "{error}"
    );
}

#[test]
fn subrule_declaration_validation_matches_the_admitted_bazel_matrix() {
    let cases = [
        (
            "implementation",
            "bad = subrule(implementation = DefaultInfo)\n",
            "subrule implementation must be a Starlark function",
        ),
        (
            "public",
            "bad = subrule(implementation = lambda ctx: None, attrs = {'foo': attr.label(default = '//x:y')})\n",
            "subrules may only define private attributes",
        ),
        (
            "computed",
            "bad = subrule(implementation = lambda ctx: None, attrs = {'_foo': attr.label(default = lambda: '')})\n",
            "subrules cannot define computed defaults",
        ),
        (
            "type",
            "bad = subrule(implementation = lambda ctx: None, attrs = {'_foo': attr.int(default = 1)})\n",
            "subrule attributes may only be label or lists of labels",
        ),
        (
            "default",
            "bad = subrule(implementation = lambda ctx: None, attrs = {'_foo': attr.label()})\n",
            "no default value specified",
        ),
        (
            "transition",
            "t = transition(implementation = lambda settings: {}, inputs = [], outputs = ['//command_line_option:platforms'])\nbad = subrule(implementation = lambda ctx: None, attrs = {'_foo': attr.label(default = '//x:y', cfg = t)})\n",
            "subrules may only have target/exec attributes",
        ),
        (
            "toolchains",
            "bad = subrule(implementation = lambda ctx: None, toolchains = ['//a:x', '//b:y'])\n",
            "subrules may require at most 1 toolchain",
        ),
        (
            "fragment",
            "FIELD = configuration_field(fragment = 'missing', name = 'x')\n",
            "invalid configuration fragment name 'missing'",
        ),
        (
            "field",
            "FIELD = configuration_field(fragment = 'cpp', name = 'missing')\n",
            "invalid configuration field name 'missing' on fragment 'cpp'",
        ),
        (
            "identifier",
            "bad = subrule(implementation = lambda ctx: None, attrs = {'_bad-name': attr.label(default = '//x:y')})\n",
            "attribute name `_bad-name` is not a valid identifier",
        ),
        (
            "repository-late-bound",
            "def impl(ctx): pass\nbad = repository_rule(impl, attrs = {'x': attr.label(default = configuration_field(fragment = 'cpp', name = 'zipper'))})\n",
            "unsupported repository_rule attribute schema 'x'",
        ),
        (
            "repository-coverage-late-bound",
            "def impl(ctx): pass\nbad = repository_rule(impl, attrs = {'x': attr.label(default = configuration_field(fragment = 'coverage', name = 'output_generator'))})\n",
            "unsupported repository_rule attribute schema 'x'",
        ),
        (
            "repository-computed",
            "def impl(ctx): pass\nbad = repository_rule(impl, attrs = {'x': attr.label(default = lambda: '')})\n",
            "unsupported repository_rule attribute schema 'x'",
        ),
        (
            "tag-late-bound",
            "bad = tag_class(attrs = {'x': attr.label(default = configuration_field(fragment = 'cpp', name = 'zipper'))})\n",
            "tag attribute `x` does not support deferred defaults",
        ),
        (
            "tag-coverage-late-bound",
            "bad = tag_class(attrs = {'x': attr.label(default = configuration_field(fragment = 'coverage', name = 'output_generator'))})\n",
            "tag attribute `x` does not support deferred defaults",
        ),
        (
            "macro-coverage-late-bound",
            "def impl(name, visibility, x): pass\nbad = macro(implementation = impl, attrs = {'x': attr.label(default = configuration_field(fragment = 'coverage', name = 'output_generator'))})\n",
            "macro attribute 'x' uses an unsupported dependency constraint",
        ),
        (
            "subrule-coverage-aspect",
            "def aspect_impl(target, ctx): return []\na = aspect(implementation = aspect_impl)\ndef impl(ctx, **kwargs): return None\nbad = subrule(implementation = impl, attrs = {'_x': attr.label(default = configuration_field(fragment = 'coverage', name = 'output_generator'), aspects = [a])})\n",
            "Found `aspects` extra named parameter(s) for call to label",
        ),
        (
            "tag-computed",
            "bad = tag_class(attrs = {'x': attr.label(default = lambda: '')})\n",
            "tag attribute `x` does not support deferred defaults",
        ),
    ];
    for (name, source, expected) in cases {
        let workspace = scratch(name);
        let package = workspace.join("pkg");
        write(&package.join("defs.bzl"), source);
        write(&package.join("BUILD.bazel"), "load(':defs.bzl', 'bad')\n");
        let error = try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{name}: {error}");
    }

    let workspace = scratch("attribute-length");
    let package = workspace.join("pkg");
    let accepted_name = format!("_{}", "a".repeat(127));
    write(
        &package.join("defs.bzl"),
        &format!(
            "good = subrule(implementation = lambda ctx, **kwargs: None, attrs = {{'{accepted_name}': attr.label(default = '//x:y')}})\ndef _impl(ctx): return []\nr = rule(implementation = _impl, subrules = [good])\n"
        ),
    );
    write(
        &package.join("BUILD.bazel"),
        "load(':defs.bzl', 'r')\nr(name = 'accepted')\n",
    );
    assert_eq!(
        starlark_rule(&load_package(&workspace, &package), "accepted")
            .subrule_hidden_attribute_names()
            .count(),
        1
    );

    let rejected_name = format!("_{}", "a".repeat(128));
    write(
        &package.join("defs.bzl"),
        &format!(
            "bad = subrule(implementation = lambda ctx, **kwargs: None, attrs = {{'{rejected_name}': attr.label(default = '//x:y')}})\n"
        ),
    );
    let error = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(error.contains("name is too long (129 > 128)"), "{error}");
}

#[test]
fn duplicate_nested_and_shared_diamond_keep_set_identity_and_ordered_spans() {
    let workspace = scratch("diamond");
    let package = workspace.join("pkg");
    write(
        &package.join("defs.bzl"),
        r#"
def _impl(ctx):
    return None

base = subrule(implementation = _impl, attrs = {"_base": attr.label(default = "//x:base")})
left = subrule(implementation = _impl, attrs = {"_left": attr.label(default = "//x:left")}, subrules = [base])
right = subrule(implementation = _impl, attrs = {"_right": attr.label(default = "//x:right")}, subrules = [base, base])

def _rule_impl(ctx):
    return []

first = rule(implementation = _rule_impl, subrules = [right, left, right])
second = rule(implementation = _rule_impl, subrules = [left, right])
"#,
    );
    write(
        &package.join("BUILD.bazel"),
        "load(':defs.bzl', 'first', 'second')\nfirst(name = 'one')\nsecond(name = 'two')\n",
    );
    let loaded = load_package(&workspace, &package);
    let first = starlark_rule(&loaded, "one");
    let second = starlark_rule(&loaded, "two");
    assert_eq!(first.attached_subrule_count(), 3);
    assert_eq!(second.attached_subrule_count(), 3);
    assert_eq!(
        first.direct_subrule_names().collect::<Vec<_>>(),
        ["left", "right"]
    );
    assert_eq!(
        second.direct_subrule_names().collect::<Vec<_>>(),
        ["left", "right"]
    );
    assert!(!first.direct_subrule_names().any(|name| name == "base"));
    assert_eq!(first.subrule_callables().count(), 3);
    assert!(
        first
            .subrule_callables()
            .all(|(_, _, value)| value.to_value().get_type() == "subrule")
    );
    assert_eq!(
        first.subrule_definition_names().collect::<Vec<_>>(),
        second.subrule_definition_names().collect::<Vec<_>>()
    );
    assert_eq!(
        first.subrule_attribute_spans().collect::<Vec<_>>(),
        [("right", 0, 1), ("base", 1, 1), ("left", 2, 1)]
    );
    assert_eq!(
        second.subrule_attribute_spans().collect::<Vec<_>>(),
        [("left", 0, 1), ("base", 1, 1), ("right", 2, 1)]
    );
}

#[test]
fn rules_cc_fdo_declaration_and_rule_attachment_freeze_without_invocation() {
    let workspace = scratch("fdo");
    let package = workspace.join("cc/private/rules_impl/fdo");
    write(
        &package.join("fdo_context.bzl"),
        r#"
FdoProfileInfo = provider()
FdoPrefetchHintsInfo = provider()
PropellerOptimizeInfo = provider()
MemProfProfileInfo = provider()
TemplateVariableInfo = platform_common.TemplateVariableInfo
def _check_template_variable_info():
    if type(TemplateVariableInfo) != "analysis_builtin" or str(TemplateVariableInfo) != "TemplateVariableInfo":
        fail("platform_common.TemplateVariableInfo loading token changed")

_check_template_variable_info()

def _create_fdo_context(ctx, **kwargs):
    fail("loading must not invoke create_fdo_context")

create_fdo_context = subrule(
    implementation = _create_fdo_context,
    fragments = ["cpp"],
    attrs = {
        "_fdo_optimize": attr.label(default = configuration_field(fragment = "cpp", name = "fdo_optimize"), allow_files = True, providers = [[DefaultInfo], [FdoProfileInfo]]),
        "_xfdo_profile": attr.label(default = configuration_field(fragment = "cpp", name = "xbinary_fdo"), providers = [FdoProfileInfo]),
        "_fdo_profile": attr.label(default = configuration_field(fragment = "cpp", name = "fdo_profile"), providers = [FdoProfileInfo]),
        "_csfdo_profile": attr.label(default = configuration_field(fragment = "cpp", name = "cs_fdo_profile"), providers = [FdoProfileInfo]),
        "_fdo_prefetch_hints": attr.label(default = configuration_field(fragment = "cpp", name = "fdo_prefetch_hints"), providers = [FdoPrefetchHintsInfo]),
        "_propeller_optimize": attr.label(default = configuration_field(fragment = "cpp", name = "propeller_optimize"), providers = [PropellerOptimizeInfo]),
        "_memprof_profile": attr.label(default = configuration_field(fragment = "cpp", name = "memprof_profile"), providers = [MemProfProfileInfo]),
        "_proto_profile": attr.label(default = configuration_field(fragment = "cpp", name = "proto_profile_path"), allow_single_file = True),
    },
)

def _cc_toolchain_impl(ctx):
    fail("user rule implementation must not run")

cc_toolchain = rule(
    implementation = _cc_toolchain_impl,
    attrs = {
        "_libc_top": attr.label(default = configuration_field(fragment = "cpp", name = "libc_top")),
        "_zipper": attr.label(default = configuration_field(fragment = "cpp", name = "zipper"), cfg = "exec", executable = True),
    },
    subrules = [create_fdo_context],
)

ordinary_only = rule(
    implementation = _cc_toolchain_impl,
    attrs = {"_zipper": attr.label(default = configuration_field(fragment = "cpp", name = "zipper"))},
)
"#,
    );
    write(
        &package.join("BUILD.bazel"),
        "load(':fdo_context.bzl', 'cc_toolchain', 'ordinary_only')\ncc_toolchain(name = 'toolchain')\nordinary_only(name = 'ordinary')\n",
    );
    let loaded = load_package(&workspace, &package);
    let rule = starlark_rule(&loaded, "toolchain");
    assert_eq!(rule.attached_subrule_count(), 1);
    assert_eq!(rule.subrule_hidden_attribute_names().count(), 8);
    assert!(rule.subrule_hidden_attribute_names().all(|name| {
        name.starts_with("://cc/private/rules_impl/fdo:fdo_context.bzl%create_fdo_context%_")
    }));
    assert_eq!(rule.subrule_fragments().collect::<Vec<_>>(), ["cpp"]);
    assert_eq!(
        rule.late_bound_rule_attributes().collect::<Vec<_>>(),
        [("_libc_top", "libc_top"), ("_zipper", "zipper")]
    );
    let configured = rule.configured_dependency_attributes().collect::<Vec<_>>();
    assert_eq!(configured.len(), 10);
    assert_eq!(configured[0].name(), "_libc_top");
    assert_eq!(configured[0].user_name(), None);
    assert!(!configured[0].is_hidden());
    assert!(!configured[0].exec_configuration());
    assert!(!configured[0].executable());
    assert!(configured[0].required_providers().is_empty());
    let ConfiguredDependencyDefault::ConfigurationField(libc) = configured[0].default() else {
        panic!("ordinary libc row lost its typed default")
    };
    assert_eq!(libc.field().field_name(), "libc_top");
    assert_eq!(configured[1].name(), "_zipper");
    assert!(configured[1].exec_configuration());
    assert!(configured[1].executable());
    let proto = configured
        .iter()
        .copied()
        .find(|attribute| attribute.user_name() == Some("_proto_profile"))
        .unwrap();
    assert_eq!(proto.kind(), AttributeKind::Label);
    assert!(proto.file_admissibility().single_artifact());
    assert!(proto.file_admissibility().is_any_file());
    let fdo = configured
        .iter()
        .copied()
        .find(|attribute| attribute.user_name() == Some("_fdo_optimize"))
        .unwrap();
    assert!(fdo.file_admissibility().is_any_file());
    assert!(!fdo.file_admissibility().single_artifact());
    assert_eq!(
        fdo.required_providers()
            .iter()
            .map(|alternative| alternative
                .iter()
                .map(|provider| provider.name())
                .collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        [vec!["DefaultInfo"], vec!["FdoProfileInfo"]]
    );

    let module = Module::new();
    let mut evaluator = Evaluator::new(&module);
    let error = evaluator
        .eval_function(rule.frozen_value().to_value(), &[Value::new_none()], &[])
        .unwrap_err()
        .to_string();
    assert!(
        error.contains(
            "configured analysis of subrule 'create_fdo_context' reached the deferred invocation boundary"
        ),
        "{error}"
    );
    assert!(!error.contains("user rule implementation must not run"));

    let ordinary = starlark_rule(&loaded, "ordinary");
    assert_eq!(
        ordinary.late_bound_rule_attributes().collect::<Vec<_>>(),
        [("_zipper", "zipper")]
    );
    let error = evaluator
        .eval_function(
            ordinary.frozen_value().to_value(),
            &[Value::new_none()],
            &[],
        )
        .unwrap_err()
        .to_string();
    assert!(
        error.contains(
            "configured analysis of rule attribute '_zipper' reached the deferred late-bound value materialization boundary"
        ),
        "{error}"
    );
    assert!(!error.contains("user rule implementation must not run"));
}
