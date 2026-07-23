use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use dice::DetectCycles;
use dice::Dice;
use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::AttributeKind;
use slug_loading_v2::AttributeProvenance;
use slug_loading_v2::BzlModuleEvaluator;
use slug_loading_v2::CoercedAttributeValue;
use slug_loading_v2::PackageTarget;
use slug_loading_v2::PackageTargetKind;
use slug_loading_v2::file_discovery::BUILD_FILE_FALLBACK;
use slug_loading_v2::file_discovery::BUILD_FILE_PRIMARY;
use slug_loading_v2::file_discovery::MODULE_FILE;
use slug_loading_v2::file_discovery::find_build_file;
use slug_loading_v2::file_discovery::find_workspace_root;
use slug_loading_v2::file_discovery::is_bazel_build_file;
use slug_loading_v2::file_discovery::is_bzl_file;
use slug_loading_v2::keys::WorkspaceDirectoryEntry;
use slug_loading_v2::keys::WorkspaceDirectoryEntryKind;
use slug_loading_v2::keys::WorkspaceDirectorySnapshot;
use slug_loading_v2::keys::WorkspaceDirectorySnapshotKey;
use slug_loading_v2::keys::WorkspaceDirectoryValue;
use slug_loading_v2::keys::WorkspaceFileValue;
use slug_loading_v2::keys::WorkspaceSnapshot;
use slug_loading_v2::keys::WorkspaceSnapshotKey;

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("slug-loading-v2-{name}-{nanos}"));
    fs::create_dir_all(&root).unwrap();
    root
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

fn load_package(workspace: &Path, package: &Path) -> slug_loading_v2::LoadedPackage {
    try_load_package(workspace, package).unwrap()
}

fn try_load_package(
    workspace: &Path,
    package: &Path,
) -> anyhow::Result<slug_loading_v2::LoadedPackage> {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut paths = vec![
        workspace.join(MODULE_FILE),
        workspace.join(BUILD_FILE_PRIMARY),
        workspace.join(BUILD_FILE_FALLBACK),
        package.join(BUILD_FILE_PRIMARY),
        package.join(BUILD_FILE_FALLBACK),
    ];
    for entry in fs::read_dir(package).unwrap() {
        let path = entry.unwrap().path();
        if is_bzl_file(&path) {
            paths.push(path);
        }
    }
    let files = paths
        .into_iter()
        .map(|path| {
            let value = match fs::read_to_string(&path) {
                Ok(source) => WorkspaceFileValue::Present(Arc::new(source)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    WorkspaceFileValue::Absent
                }
                Err(error) => WorkspaceFileValue::ReadError(Arc::new(error.to_string())),
            };
            (path, value)
        })
        .collect();
    let evaluator = BzlModuleEvaluator::new(workspace).unwrap();
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async move {
            let mut updater = dice.updater();
            updater
                .changed_to(vec![(
                    (WorkspaceSnapshotKey {
                        workspace: workspace.to_path_buf(),
                    }),
                    Arc::new(WorkspaceSnapshot {
                        files: Arc::new(files),
                    }),
                )])
                .unwrap();
            updater
                .changed_to(vec![(
                    (WorkspaceDirectorySnapshotKey {
                        workspace: workspace.to_path_buf(),
                    }),
                    Arc::new(directory_snapshot(workspace)),
                )])
                .unwrap();
            let mut transaction = updater.commit().await;
            evaluator.evaluate_package(&mut transaction, package).await
        })
}

#[test]
fn workspace_root_requires_module_bazel() {
    let root = scratch("root");
    let pkg = root.join("pkg");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(root.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    assert_eq!(find_workspace_root(&pkg).unwrap().path(), root.as_path());

    let missing = scratch("missing");
    fs::write(missing.join("WORKSPACE"), "# ignored\n").unwrap();
    let err = find_workspace_root(&missing).unwrap_err();
    assert!(err.contains(MODULE_FILE));
}

#[test]
fn build_file_discovery_is_bazel_only() {
    let pkg = scratch("build-file");
    fs::write(pkg.join(BUILD_FILE_FALLBACK), "# fallback\n").unwrap();
    assert_eq!(
        find_build_file(&pkg).unwrap(),
        pkg.join(BUILD_FILE_FALLBACK)
    );
    fs::write(pkg.join(BUILD_FILE_PRIMARY), "# primary\n").unwrap();
    assert_eq!(find_build_file(&pkg).unwrap(), pkg.join(BUILD_FILE_PRIMARY));

    let other_1 = pkg.join(concat!("BU", "CK"));
    let other_2 = pkg.join(concat!("TAR", "GETS"));
    fs::write(&other_1, "# ignored\n").unwrap();
    fs::write(&other_2, "# ignored\n").unwrap();
    assert!(!is_bazel_build_file(&other_1));
    assert!(!is_bazel_build_file(&other_2));
}

#[test]
fn recognizes_bzl_extension_only() {
    assert!(is_bzl_file(&PathBuf::from("defs.bzl")));
    assert!(!is_bzl_file(&PathBuf::from("defs.star")));
}

#[test]
fn package_load_evaluates_loaded_macro_and_bazel_package_globals() {
    let workspace = scratch("package-load");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("defs.bzl"),
        "def make_export(name, src):\n    native.exports_files([src])\n    native.filegroup(name = name, srcs = [src])\n",
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"make_export\")\npackage(default_visibility = [\"//visibility:public\"])\nexports_files([\"data.txt\"])\nfilegroup(name = \"fg\", srcs = [\"data.txt\"])\nalias(name = \"alias_fg\", actual = \":fg\")\nmake_export(name = \"macro_file\", src = \"macro.txt\")\n",
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);

    assert_eq!(loaded.default_visibility, vec!["//visibility:public"]);
    assert_eq!(
        loaded.targets,
        vec![
            PackageTarget {
                name: "data.txt".to_owned(),
                kind: PackageTargetKind::ExportedFile,
            },
            PackageTarget {
                name: "fg".to_owned(),
                kind: PackageTargetKind::Filegroup {
                    srcs: vec!["data.txt".to_owned()],
                },
            },
            PackageTarget {
                name: "alias_fg".to_owned(),
                kind: PackageTargetKind::Alias {
                    actual: ":fg".to_owned(),
                },
            },
            PackageTarget {
                name: "macro.txt".to_owned(),
                kind: PackageTargetKind::ExportedFile,
            },
            PackageTarget {
                name: "macro_file".to_owned(),
                kind: PackageTargetKind::Filegroup {
                    srcs: vec!["macro.txt".to_owned()],
                },
            },
        ]
    );
}

#[test]
fn package_load_registers_a_generic_starlark_rule_without_executing_it() {
    let workspace = scratch("rule-load");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("defs.bzl"),
        "def _impl(ctx):\n    out = ctx.actions.declare_file(ctx.label.name + \".txt\")\n    return [DefaultInfo(files = depset([out]))]\n\nexample = rule(implementation = _impl)\n",
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"example\")\nexample(name = \"registered\")\n",
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);

    assert_eq!(loaded.targets.len(), 1,);
    assert_eq!(loaded.targets[0].name, "registered");
    assert!(matches!(
        loaded.targets[0].kind,
        PackageTargetKind::StarlarkRule(_)
    ));
    let PackageTargetKind::StarlarkRule(implementation) = &loaded.targets[0].kind else {
        unreachable!()
    };
    assert!(implementation.dependencies().is_empty());
}

#[test]
fn rule_deps_schema_retains_exact_normalized_order_and_rejects_other_shapes() {
    let workspace = scratch("rule-deps");
    let package = workspace.join("parent");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("defs.bzl"),
        "def _impl(ctx):\n    return [DefaultInfo(files = depset([]))]\n\nwith_deps = rule(implementation = _impl, attrs = {\"deps\": attr.label_list()})\nwithout_deps = rule(implementation = _impl)\n",
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"with_deps\")\nwith_deps(name = \"ordered\", deps = [\"//leaf:second\", \":local\", \"//leaf:first\"], visibility = [\"//visibility:public\"])\nwith_deps(name = \"omitted\")\n",
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    let dependencies = loaded
        .targets
        .iter()
        .find_map(|target| match &target.kind {
            PackageTargetKind::StarlarkRule(implementation) if target.name == "ordered" => {
                Some(implementation.dependencies())
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(
        dependencies,
        [
            CanonicalLabel::parse("@@//leaf:second").unwrap(),
            CanonicalLabel::parse("@@//parent:local").unwrap(),
            CanonicalLabel::parse("@@//leaf:first").unwrap(),
        ]
    );
    let omitted = loaded
        .targets
        .iter()
        .find_map(|target| match &target.kind {
            PackageTargetKind::StarlarkRule(implementation) if target.name == "omitted" => {
                Some(implementation.dependencies())
            }
            _ => None,
        })
        .unwrap();
    assert!(omitted.is_empty());

    let bad_builds = [
        (
            "with_deps(name = \"bad\", unknown = [])\n",
            "unknown attribute `unknown`",
        ),
        (
            "without_deps(name = \"bad\", deps = [])\n",
            "unknown attribute `deps`",
        ),
        (
            "with_deps(name = \"bad\", deps = (\":one\",))\n",
            "attribute `deps` must be a list of labels",
        ),
        (
            "with_deps(name = \"bad\", deps = [1])\n",
            "attribute `deps` must contain only string labels",
        ),
        (
            "with_deps(name = \"bad\", deps = [\"relative\"])\n",
            "dependency label must be package-relative",
        ),
        (
            "with_deps(name = \"bad\", deps = [\"@repo//leaf:one\"])\n",
            "external repository dependency labels are not supported",
        ),
        (
            "with_deps(name = \"bad\", deps = [\"@@repo//leaf:one\"])\n",
            "external repository dependency labels are not supported",
        ),
    ];
    for (invocation, expected) in bad_builds {
        fs::write(
            package.join(BUILD_FILE_PRIMARY),
            format!("load(\":defs.bzl\", \"with_deps\", \"without_deps\")\n{invocation}"),
        )
        .unwrap();
        let error = try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "error: {error}");
    }
}

#[test]
fn rule_attribute_schema_retains_provenance_selectors_dicts_and_generated_outputs() {
    let workspace = scratch("attribute-metadata");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("defs.bzl"),
        r#"
def _impl(ctx):
    return [DefaultInfo()]

probe = rule(
    implementation = _impl,
    attrs = {
        "one": attr.label(),
        "many": attr.label_list(default = [":default"]),
        "note": attr.string(default = "text"),
        "_implicit": attr.label(default = ":implicit"),
        "chosen": attr.label_list(),
        "string_labels": attr.string_keyed_label_dict(),
        "label_strings": attr.label_keyed_string_dict(),
        "label_lists": attr.label_list_dict(),
        "out": attr.output(mandatory = True),
        "outs": attr.output_list(mandatory = True),
        "trailing": attr.label_list(),
        "locked": attr.label(configurable = False),
        "chained": attr.label_list(),
        "nested_prefix": attr.label_list(),
    },
)
"#,
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        r#"
load(":defs.bzl", "probe")
probe(
    name = "metadata",
    one = ":source",
    chosen = [":shared"] + select({":condition": [":linux"], "//conditions:default": [":fallback"]}),
    string_labels = {"local": ":source"},
    label_strings = {":default": "default"},
    label_lists = {"local": [":implicit", ":source"]},
    out = "one.out",
    outs = ["two.out", "three.out"],
    trailing = select({":condition": [":linux"], "//conditions:default": [":fallback"]}) + [":after"],
    chained = [":before"] + select({":condition": [":one"], "//conditions:default": [":one_default"]}) + select({":second_condition": [":two"], "//conditions:default": [":two_default"]}) + [":after"] + [":again"],
    nested_prefix = [":outer"] + ([":inner"] + select({":condition": [":selected"]})),
)
"#,
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    let PackageTargetKind::StarlarkRule(rule) = &loaded.targets[0].kind else {
        panic!("expected Starlark rule")
    };
    assert_eq!(
        rule.schema()
            .iter()
            .map(|schema| schema.kind())
            .collect::<Vec<_>>(),
        vec![
            AttributeKind::Label,
            AttributeKind::LabelList,
            AttributeKind::String,
            AttributeKind::Label,
            AttributeKind::LabelList,
            AttributeKind::StringKeyedLabelDict,
            AttributeKind::LabelKeyedStringDict,
            AttributeKind::LabelListDict,
            AttributeKind::Output,
            AttributeKind::OutputList,
            AttributeKind::LabelList,
            AttributeKind::Label,
            AttributeKind::LabelList,
            AttributeKind::LabelList,
        ]
    );
    assert_eq!(rule.schema()[3].declaration_name(), "_implicit");
    assert_eq!(rule.schema()[3].query_name(), "$implicit");
    assert!(rule.schema()[3].default().is_some());
    assert!(rule.schema()[8].mandatory());
    assert!(rule.schema()[0].configurable());
    assert!(!rule.schema()[8].configurable());
    assert!(!rule.schema()[9].configurable());
    assert!(!rule.schema()[11].configurable());
    assert!(rule.schema()[2].dependency_reachable() == false);
    assert!(matches!(
        rule.schema()[0].default(),
        Some(CoercedAttributeValue::None)
    ));
    assert!(
        matches!(rule.schema()[4].default(), Some(CoercedAttributeValue::LabelList(values)) if values.is_empty())
    );
    assert!(
        matches!(rule.schema()[5].default(), Some(CoercedAttributeValue::StringKeyedLabelDict(values)) if values.is_empty())
    );
    assert!(
        matches!(rule.schema()[6].default(), Some(CoercedAttributeValue::LabelKeyedStringDict(values)) if values.is_empty())
    );
    assert!(
        matches!(rule.schema()[7].default(), Some(CoercedAttributeValue::LabelListDict(values)) if values.is_empty())
    );
    assert!(
        matches!(rule.schema()[9].default(), Some(CoercedAttributeValue::OutputList(values)) if values.is_empty())
    );

    let values = rule.values();
    assert_eq!(values[0].provenance, AttributeProvenance::Explicit);
    assert_eq!(values[1].provenance, AttributeProvenance::Default);
    assert_eq!(values[2].provenance, AttributeProvenance::Default);
    assert_eq!(values[3].provenance, AttributeProvenance::Implicit);
    assert!(matches!(
        values.iter().find(|value| value.declaration_name == "note").unwrap().value.as_ref(),
        CoercedAttributeValue::String(value) if value == "text"
    ));
    assert!(matches!(
        values
            .iter()
            .find(|value| value.declaration_name == "string_labels")
            .unwrap()
            .value
            .as_ref(),
        CoercedAttributeValue::StringKeyedLabelDict(_)
    ));
    assert!(matches!(
        values
            .iter()
            .find(|value| value.declaration_name == "label_strings")
            .unwrap()
            .value
            .as_ref(),
        CoercedAttributeValue::LabelKeyedStringDict(_)
    ));
    assert!(matches!(
        values
            .iter()
            .find(|value| value.declaration_name == "label_lists")
            .unwrap()
            .value
            .as_ref(),
        CoercedAttributeValue::LabelListDict(_)
    ));
    assert!(matches!(
        values
            .iter()
            .find(|value| value.declaration_name == "chosen")
            .unwrap()
            .value
            .as_ref(),
        CoercedAttributeValue::Concatenation(_, _)
    ));
    let mut chosen_labels = Vec::new();
    values
        .iter()
        .find(|value| value.declaration_name == "chosen")
        .unwrap()
        .value
        .labels(&mut chosen_labels);
    assert_eq!(
        chosen_labels,
        vec![
            CanonicalLabel::parse("@@//pkg:shared").unwrap(),
            CanonicalLabel::parse("@@//pkg:linux").unwrap(),
            CanonicalLabel::parse("@@//pkg:fallback").unwrap(),
        ]
    );
    assert!(!chosen_labels.contains(&CanonicalLabel::parse("@@//pkg:condition").unwrap()));
    let mut trailing_labels = Vec::new();
    values
        .iter()
        .find(|value| value.declaration_name == "trailing")
        .unwrap()
        .value
        .labels(&mut trailing_labels);
    assert_eq!(
        trailing_labels,
        vec![
            CanonicalLabel::parse("@@//pkg:linux").unwrap(),
            CanonicalLabel::parse("@@//pkg:fallback").unwrap(),
            CanonicalLabel::parse("@@//pkg:after").unwrap(),
        ]
    );
    let mut chained_labels = Vec::new();
    values
        .iter()
        .find(|value| value.declaration_name == "chained")
        .unwrap()
        .value
        .labels(&mut chained_labels);
    assert_eq!(
        chained_labels,
        vec![
            CanonicalLabel::parse("@@//pkg:before").unwrap(),
            CanonicalLabel::parse("@@//pkg:one").unwrap(),
            CanonicalLabel::parse("@@//pkg:one_default").unwrap(),
            CanonicalLabel::parse("@@//pkg:two").unwrap(),
            CanonicalLabel::parse("@@//pkg:two_default").unwrap(),
            CanonicalLabel::parse("@@//pkg:after").unwrap(),
            CanonicalLabel::parse("@@//pkg:again").unwrap(),
        ]
    );
    let mut nested_prefix_labels = Vec::new();
    values
        .iter()
        .find(|value| value.declaration_name == "nested_prefix")
        .unwrap()
        .value
        .labels(&mut nested_prefix_labels);
    assert_eq!(
        nested_prefix_labels,
        vec![
            CanonicalLabel::parse("@@//pkg:outer").unwrap(),
            CanonicalLabel::parse("@@//pkg:inner").unwrap(),
            CanonicalLabel::parse("@@//pkg:selected").unwrap(),
        ]
    );

    assert!(matches!(
        loaded.targets[1].kind,
        PackageTargetKind::GeneratedFile { ref generating_rule, ref label }
            if generating_rule == "metadata" && label == &CanonicalLabel::parse("@@//pkg:one.out").unwrap()
    ));
    assert_eq!(loaded.targets[1].name, "one.out");
    assert_eq!(loaded.targets[3].name, "three.out");

    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"probe\")\nprobe(name = \"missing_output\")\n",
    )
    .unwrap();
    assert!(
        try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string()
            .contains("missing value for mandatory attribute `out`")
    );
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"probe\")\nexports_files([\"one.out\"])\nprobe(name = \"colliding_output\", out = \"one.out\", outs = [])\n",
    )
    .unwrap();
    assert!(
        try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string()
            .contains("target 'one.out' declared more than once")
    );
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"probe\")\nprobe(name = \"locked_select\", locked = select({\":condition\": \":source\"}), out = \"one.out\", outs = [])\n",
    )
    .unwrap();
    assert!(
        try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string()
            .contains("attribute `locked` is not configurable")
    );
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"probe\")\nprobe(name = \"output_select\", out = select({\":condition\": \"one.out\"}), outs = [])\n",
    )
    .unwrap();
    assert!(
        try_load_package(&workspace, &package)
            .unwrap_err()
            .to_string()
            .contains("attribute `out` is not configurable")
    );
    for invalid in ["//other:out", "../out", "bad:name"] {
        fs::write(
            package.join(BUILD_FILE_PRIMARY),
            format!("load(\":defs.bzl\", \"probe\")\nprobe(name = \"bad_output\", out = \"{invalid}\", outs = [])\n"),
        )
        .unwrap();
        assert!(
            try_load_package(&workspace, &package)
                .unwrap_err()
                .to_string()
                .contains("output label must name a valid target in this package")
        );
    }
}

#[test]
fn build_and_macro_native_glob_share_the_prepared_package_listing() {
    let workspace = scratch("glob-callable");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(package.join("sub")).unwrap();
    fs::create_dir_all(package.join("subpackage")).unwrap();
    fs::write(package.join("keep.txt"), "keep\n").unwrap();
    fs::write(package.join("skip.txt"), "skip\n").unwrap();
    fs::write(package.join("sub/child.txt"), "child\n").unwrap();
    fs::write(package.join("subpackage/BUILD.bazel"), "# boundary\n").unwrap();
    fs::write(package.join("subpackage/hidden.txt"), "hidden\n").unwrap();
    fs::write(
        package.join("defs.bzl"),
        "def make_group():\n    native.filegroup(name = \"macro\", srcs = native.glob((\"sub/*.txt\",)))\n",
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"make_group\")\nfilegroup(name = \"direct\", srcs = glob([\"*.txt\"], exclude = [\"skip.txt\"]))\nfilegroup(name = \"dirs\", srcs = glob([\"*\"], exclude_directories = 0))\nfilegroup(name = \"omitted\", srcs = glob(allow_empty = True))\nmake_group()\n",
    )
    .unwrap();

    let loaded = load_package(&workspace, &package);
    assert_eq!(
        loaded
            .targets
            .iter()
            .map(|target| (target.name.clone(), target.kind.clone()))
            .collect::<Vec<_>>(),
        vec![
            (
                "direct".to_owned(),
                PackageTargetKind::Filegroup {
                    srcs: vec!["keep.txt".to_owned()]
                }
            ),
            (
                "dirs".to_owned(),
                PackageTargetKind::Filegroup {
                    srcs: vec![
                        "BUILD.bazel".to_owned(),
                        "defs.bzl".to_owned(),
                        "keep.txt".to_owned(),
                        "skip.txt".to_owned(),
                        "sub".to_owned()
                    ]
                }
            ),
            (
                "omitted".to_owned(),
                PackageTargetKind::Filegroup { srcs: Vec::new() },
            ),
            (
                "macro".to_owned(),
                PackageTargetKind::Filegroup {
                    srcs: vec!["sub/child.txt".to_owned()]
                }
            ),
        ]
    );
    assert_eq!(loaded.used_globs.len(), 4);
}

#[test]
fn glob_reports_context_and_allow_empty_type_errors() {
    let workspace = scratch("glob-errors");
    let package = workspace.join("pkg");
    fs::write(workspace.join(MODULE_FILE), "module(name = \"root\")\n").unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "filegroup(name = \"bad\", srcs = glob([\"*.txt\"], allow_empty = 5))\n",
    )
    .unwrap();
    let error = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(error.contains("expected boolean for argument `allow_empty`, got `5`"));

    fs::write(
        package.join("defs.bzl"),
        "BAD = glob([\"*.txt\"], allow_empty = True)\n",
    )
    .unwrap();
    fs::write(
        package.join(BUILD_FILE_PRIMARY),
        "load(\":defs.bzl\", \"BAD\")\n",
    )
    .unwrap();
    let error = try_load_package(&workspace, &package)
        .unwrap_err()
        .to_string();
    assert!(error.contains("glob() may only be called while evaluating a BUILD package"));
}
