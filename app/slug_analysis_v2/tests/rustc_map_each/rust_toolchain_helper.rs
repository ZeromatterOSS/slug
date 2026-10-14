//! Genuine helper invocation through the current analysis evaluator.
use slug_build_api_v2::AnalysisValue;

use super::super::cc_configure_features::config;
use super::super::cc_configure_features::field;
use super::*;

const PROBE: &str = include_str!("rust_toolchain_helper.bzl");
const FILES: &[&str] = &[
    "liballoc.a",
    "libbuiltins.a",
    "libcore.a",
    "libpanic_abort.a",
    "libpanic_unwind.a",
    "libmiddle.a",
    "libmemchr.a",
    "libstd.a",
    "libtest.a",
    "liballocator.a",
    "libstd.so",
    "self.o",
];

fn source(root: &std::path::Path, marker: &str) {
    fs::write(
        root.join("rules_rust/rust/private/helper_proof.bzl"),
        PROBE.replace("MARKER", marker),
    )
    .unwrap();
}

fn workspace() -> PathBuf {
    let root = super::cc_toolchain::workspace();
    source(&root, "A");
    fs::write(
        root.join("rules_rust/proof.bzl"),
        "load('//rust/private:helper_proof.bzl', 'stdlib', 'subject')\n",
    )
    .unwrap();
    for file in FILES {
        fs::write(root.join(file), "authored input\n").unwrap();
    }
    let files: Vec<_> = FILES.iter().map(|f| format!("@@//:{f}")).collect();
    let mut exports = FILES.to_vec();
    exports.extend(["header.h", "static.a", "dynamic.so", "other.a"]);
    fs::write(
        root.join("BUILD.bazel"),
        format!("exports_files({exports:?})\n"),
    )
    .unwrap();
    fs::write(
        root.join("rules_rust/BUILD.bazel"),
        format!(
            "load(':proof.bzl','stdlib','subject')\n\
        stdlib(name='std',files={files:?})\n\
        stdlib(name='empty_std',files={files:?},empty=True)\n\
        stdlib(name='objects',files={files:?},empty=True,self_contained=True)\n\
        subject(name='impl',stdlib=':std')\n\
        subject(name='cc',stdlib=':std',allocator_kind='cc')\n\
        subject(name='no_std',stdlib=':std',mode='no_std_with_alloc')\n\
        subject(name='dynamic',stdlib=':std',dynamic=True)\n\
        subject(name='empty',stdlib=':empty_std',allocator_kind='none')\n\
        subject(name='self_contained',stdlib=':objects',allocator_kind='none')\n\
        subject(name='bad_mode',stdlib=':std',mode='invalid')\n\
        subject(name='bad_provider',stdlib='@@//:libstd.a')\n\
        subject(name='bad_type',stdlib=':std',allocator_kind='bad_type')\n\
        subject(name='bad_fields',stdlib=':std',allocator_kind='bad_fields')\n"
        ),
    )
    .unwrap();
    root
}

async fn request(
    dice: &Arc<Dice>,
    root: &std::path::Path,
    observed: bool,
    target: &str,
    configuration: ConfigurationKey,
) -> Result<Arc<ConfiguredNodeResult>, String> {
    super::request_configured(
        dice,
        root,
        root_epoch(root),
        observed,
        target,
        configuration,
    )
    .await
}

fn member<'a>(value: &'a AnalysisValue, name: &str) -> &'a AnalysisValue {
    let AnalysisValueKind::Struct(fields) = value.kind() else {
        panic!("row struct")
    };
    fields.get(name).unwrap()
}

fn list(value: &AnalysisValue) -> &[AnalysisValue] {
    let AnalysisValueKind::List(values) = value.kind() else {
        panic!("list")
    };
    values
}

fn names(result: &ConfiguredNodeResult) -> Vec<Vec<&str>> {
    list(field(result, "rows"))
        .iter()
        .map(|row| {
            list(member(row, "libraries"))
                .iter()
                .map(|lib| member(lib, "name").as_str().unwrap())
                .collect()
        })
        .collect()
}

fn assert_static(result: &ConfiguredNodeResult, target: &str) {
    let standard = [
        "libtest.a",
        "libstd.a",
        "libpanic_unwind.a",
        "libmiddle.a",
        "libmemchr.a",
        "libcore.a",
        "libbuiltins.a",
        "liballoc.a",
    ];
    let mut expected = if target == "no_std" {
        vec![
            "libpanic_abort.a",
            "libcore.a",
            "libbuiltins.a",
            "liballoc.a",
        ]
    } else {
        standard.to_vec()
    };
    if target != "cc" {
        expected.push("liballocator.a");
    }
    let mut rows = vec![expected];
    if target == "cc" {
        rows.push(vec!["liballocator.a"]);
    }
    assert_eq!(names(result), rows);
    let actual_rows = list(field(result, "rows"));
    assert_eq!(
        member(&actual_rows[0], "owner").as_str(),
        Some("@@rules_rust+//:std")
    );
    if target == "cc" {
        assert_eq!(
            member(&actual_rows[1], "owner").as_str(),
            Some("@@rules_rust+//:cc")
        );
    }
    for row in actual_rows {
        for library in list(member(row, "libraries")) {
            assert_eq!(member(library, "pic"), member(library, "path"));
            assert!(matches!(
                member(library, "resolved").kind(),
                AnalysisValueKind::None
            ));
        }
    }
    assert!(result.actions().is_empty());
}

#[tokio::test]
async fn actual_allocator_helper_orders_libraries_and_restores_source_configuration() {
    for observed in [false, true] {
        let root = workspace();
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let fast = config(&[]);
        for target in ["impl", "cc", "no_std"] {
            let first = request(&dice, &root, observed, target, fast.clone())
                .await
                .unwrap();
            assert_static(&first, target);
            assert_eq!(field(&first, "marker").as_str(), Some("A:fastbuild"));
            let changed = request(
                &dice,
                &root,
                observed,
                target,
                config(&[(NativeCommandOption::CompilationMode, "dbg")]),
            )
            .await
            .unwrap();
            assert_static(&changed, target);
            assert_eq!(field(&changed, "marker").as_str(), Some("A:dbg"));
            assert_ne!(first, changed);
            assert_eq!(
                request(&dice, &root, observed, target, fast.clone())
                    .await
                    .unwrap(),
                first
            );
            source(&root, "B");
            let changed = request(&dice, &root, observed, target, fast.clone())
                .await
                .unwrap();
            assert_static(&changed, target);
            assert_eq!(field(&changed, "marker").as_str(), Some("B:fastbuild"));
            assert_ne!(first, changed);
            source(&root, "A");
            assert_eq!(
                request(&dice, &root, observed, target, fast.clone())
                    .await
                    .unwrap(),
                first
            );
        }
        let empty = request(&dice, &root, observed, "empty", fast.clone())
            .await
            .unwrap();
        assert!(matches!(
            field(&empty, "empty").kind(),
            AnalysisValueKind::Boolean(true)
        ));
        assert!(names(&empty).is_empty());
        assert!(empty.actions().is_empty());
        for (target, phrase) in [
            (
                "bad_mode",
                "Requested 'invalid' std mode is currently not supported",
            ),
            (
                "bad_provider",
                "must be a target providing `rust_common.stdlib_info`",
            ),
            (
                "bad_type",
                "Unexpected type of allocator_library, it must be a struct",
            ),
            (
                "bad_fields",
                "must provide either a cc_info or an allocator_libraries_impl_info",
            ),
        ] {
            let error = request(&dice, &root, observed, target, fast.clone())
                .await
                .unwrap_err();
            assert!(error.contains(phrase), "{error}");
            assert!(error.contains("rust_allocator_libraries.bzl"), "{error}");
        }
        fs::remove_dir_all(root).unwrap();
    }
}

#[tokio::test]
async fn actual_allocator_helper_dynamic_and_self_contained_use_consumer_actions() {
    for observed in [false, true] {
        let root = workspace();
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let dynamic = request(&dice, &root, observed, "dynamic", config(&[]))
            .await
            .unwrap();
        assert_eq!(names(&dynamic), [vec!["libtest.a", "libstd.so"]]);
        let libraries = list(member(&list(field(&dynamic, "rows"))[0], "libraries"));
        assert_eq!(
            member(&libraries[1], "resolved").as_str(),
            Some("libstd.so")
        );
        let [action] = dynamic.actions() else {
            panic!("one native Solib action")
        };
        assert!(action.solib_symlink_spec().is_some());
        assert_eq!(action.mnemonic(), "SolibSymlink");
        assert!(
            member(&libraries[1], "path")
                .as_str()
                .unwrap()
                .contains("_solib_k8/")
        );

        let objects = request(&dice, &root, observed, "self_contained", config(&[]))
            .await
            .unwrap();
        let spawns: Vec<_> = objects
            .actions()
            .iter()
            .filter_map(|action| action.spawn_spec().map(|spawn| (action.mnemonic(), spawn)))
            .collect();
        assert_eq!(
            spawns.len(),
            2,
            "archive and nodeps dynamic defaults both remain enabled"
        );
        assert!(
            spawns.iter().any(|(name, spawn)| *name == "CppArchive"
                && spawn.render_argv().unwrap()[0] == "toolchain/bin/ar")
        );
        assert!(spawns.iter().any(
            |(name, spawn)| *name == "CppLink" && spawn.render_argv().unwrap()[0] == "toolchain/bin/gcc"
        ));
        for (_, spawn) in spawns {
            assert!(spawn.render_argv().unwrap().iter().any(|arg| arg.contains("self.o")));
            assert!(spawn.resource_set().is_some());
            assert!(!spawn.outputs().is_empty());
        }
        assert!(!names(&objects).is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
