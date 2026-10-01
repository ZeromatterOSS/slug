//! Direct invocation of the unchanged helper; proxy publication is tested separately.
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
                && spawn.render_argv()[0] == "toolchain/bin/ar")
        );
        assert!(spawns.iter().any(
            |(name, spawn)| *name == "CppLink" && spawn.render_argv()[0] == "toolchain/bin/gcc"
        ));
        for (_, spawn) in spawns {
            assert!(spawn.render_argv().iter().any(|arg| arg.contains("self.o")));
            assert!(spawn.resource_set().is_some());
            assert!(!spawn.outputs().is_empty());
        }
        assert!(!names(&objects).is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}

async fn prepare_actual_helper(
    dice: &Arc<Dice>,
    root: &std::path::Path,
    observed: bool,
    retained: Option<Arc<slug_build_api_v2::RustToolchainCallableSourceAuthority>>,
) -> Result<
    slug_loading_v2::PreparedRustToolchainHelperBinding,
    slug_loading_v2::RustToolchainHelperError,
> {
    use slug_bzlmod_v2::SourcePreparationOutcome;
    use slug_loading_v2::RustToolchainHelperMode;
    use slug_loading_v2::RustToolchainHelperRequest;

    // Reuse the real fixture's snapshot, path epoch and selected repository
    // injection before entering the public Loading owner on the same Dice.
    let result = super::request_configured(
        dice,
        root,
        root_epoch(root),
        observed,
        "subject",
        config(&[]),
    )
    .await
    .unwrap();
    assert!(result.actions().is_empty());
    let mut transaction = dice
        .updater_with_data(UserComputationData {
            cycle_detector: Some(analysis_cycle_detector()),
            ..Default::default()
        })
        .commit()
        .await;
    let workspace = NormalizedAbsolutePath::new(root.to_path_buf()).unwrap();
    let route = match transaction
        .compute(&slug_loading_v2::HostCanonicalRepositoryLoadRouteKey::new(
            workspace.clone(),
            CanonicalRepoName::new("rules_rust+").unwrap(),
        ))
        .await
        .unwrap()
    {
        SourcePreparationOutcome::Complete(result) => result.as_ref().as_ref().unwrap().clone(),
        other => panic!("actual caller route requires inputs: {other:?}"),
    };
    let package = match transaction
        .compute(&slug_loading_v2::RepositoryPackageLoadKey::new_canonical(
            route.input().clone(),
            PackagePath::root(),
        ))
        .await
        .unwrap()
    {
        SourcePreparationOutcome::Complete(result) => result,
        other => panic!("actual caller package requires inputs: {other:?}"),
    };
    let package = package.as_ref().as_ref().unwrap();
    let slug_loading_v2::PackageTargetKind::StarlarkRule(rule) = &package.targets[0].kind else {
        panic!("genuine Starlark rule attachment");
    };
    let caller = rule
        .source_identities_by_filename()
        .iter()
        .find(|(_, source)| {
            source.identity.label.to_string() == "@@rules_rust+//rust/private:toolchain.bzl"
        })
        .unwrap()
        .1
        .clone();
    let request = retained
        .map(RustToolchainHelperRequest::Retained)
        .unwrap_or(RustToolchainHelperRequest::Producer(caller));
    let mode = if observed {
        RustToolchainHelperMode::Observed
    } else {
        RustToolchainHelperMode::Legacy
    };
    match slug_loading_v2::prepare_rust_toolchain_helper(&mut transaction, workspace, request, mode)
        .await
    {
        SourcePreparationOutcome::Complete(Ok((result, _))) => result,
        other => panic!("actual helper preparation requires inputs/frontier: {other:?}"),
    }
}

#[tokio::test]
async fn actual_prepared_helper_tracks_transitive_import_and_restores_authority() {
    for observed in [false, true] {
        let root = workspace();
        fs::write(root.join("rules_rust/proof.bzl"),
            "load('//rust/private:toolchain.bzl','rust_toolchain')\ndef _impl(ctx):\n    return []\nsubject = rule(implementation=_impl)\n",
        ).unwrap();
        fs::write(
            root.join("rules_rust/BUILD.bazel"),
            "load(':proof.bzl','subject')\nsubject(name='subject',visibility=['//visibility:public'])\n",
        )
        .unwrap();
        let caller_path = root.join("rules_rust/rust/private/toolchain.bzl");
        let helper_path = root.join("rules_rust/rust/private/rust_allocator_libraries.bzl");
        let caller_bytes = fs::read(&caller_path).unwrap();
        let helper_bytes = fs::read(&helper_path).unwrap();
        let import_path = root.join("bazel_skylib/lib/paths.bzl");
        let original = fs::read(&import_path).unwrap();
        let leaf = CanonicalLabel::parse("@@bazel_skylib+//lib:paths.bzl").unwrap();
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let a = prepare_actual_helper(&dice, &root, observed, None)
            .await
            .unwrap();
        let authority_a = a.authority().clone();
        assert!(
            !authority_a
                .helper
                .direct_children
                .iter()
                .any(|child| child.label == leaf)
        );
        let leaf_a = authority_a
            .helper
            .reachable
            .iter()
            .find(|source| source.identity.label == leaf)
            .unwrap();
        let helper_a = authority_a
            .helper
            .reachable
            .iter()
            .find(|source| source.identity == authority_a.helper.root)
            .unwrap();
        let mut edited = original.clone();
        edited.extend_from_slice(b"\n# WP877 transitive import B\n");
        fs::write(&import_path, edited).unwrap();
        assert!(matches!(
            prepare_actual_helper(&dice, &root, observed, Some(authority_a.clone())).await,
            Err(slug_loading_v2::RustToolchainHelperError::Authentication(message))
                if message.as_ref() == "retained helper authority changed"
        ));
        let b = prepare_actual_helper(&dice, &root, observed, None)
            .await
            .unwrap();
        let authority_b = b.authority();
        assert_eq!(authority_a.caller, authority_b.caller);
        assert_eq!(
            authority_b
                .caller
                .digest
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            "c4b613cee96540a94fbdf4fbdca7b8dc4ef6d3082024c4d3636afc2e9c4d468e"
        );
        assert_eq!(
            helper_a
                .digest
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            "ae4acb50ac6a1b922254a07346d97b4649810d33836f2be4824fd0b7a81e536e"
        );
        assert_eq!(authority_a.helper.root, authority_b.helper.root);
        assert_eq!(
            authority_a.helper.direct_children,
            authority_b.helper.direct_children
        );
        assert_ne!(
            authority_a.helper.fingerprint,
            authority_b.helper.fingerprint
        );
        let leaf_b = authority_b
            .helper
            .reachable
            .iter()
            .find(|source| source.identity.label == leaf)
            .unwrap();
        assert_eq!(leaf_a.identity, leaf_b.identity);
        assert_ne!(leaf_a.digest, leaf_b.digest);
        assert_eq!(
            helper_a,
            authority_b
                .helper
                .reachable
                .iter()
                .find(|source| source.identity == authority_b.helper.root)
                .unwrap()
        );
        assert_eq!(
            authority_a.helper.reachable.len(),
            authority_b.helper.reachable.len()
        );
        for (a, b) in authority_a
            .helper
            .reachable
            .iter()
            .zip(authority_b.helper.reachable.iter())
        {
            assert_eq!(a.identity, b.identity);
            if a.identity.label != leaf {
                assert_eq!(a.digest, b.digest);
            }
        }
        assert_eq!(fs::read(&caller_path).unwrap(), caller_bytes);
        assert_eq!(fs::read(&helper_path).unwrap(), helper_bytes);
        fs::write(&import_path, &original).unwrap();
        let restored = prepare_actual_helper(&dice, &root, observed, Some(authority_a.clone()))
            .await
            .unwrap();
        assert_eq!(restored.authority(), &authority_a);
        assert_ne!(restored.authority(), authority_b);
        assert_eq!(fs::read(&import_path).unwrap(), original);
    }
}
