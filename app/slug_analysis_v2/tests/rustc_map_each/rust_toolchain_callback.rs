//! Original nested closure through producer disposal, aliases and fresh consumers.
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::RustToolchainCallable;
use slug_configuration_v2::StarlarkOption;
use slug_configuration_v2::StarlarkOptionScope;
use slug_configuration_v2::StarlarkOptionValue;

use super::super::cc_configure_features::config;
use super::super::cc_configure_features::field;
use super::*;

const PROBE: &str = include_str!("rust_toolchain_callback.bzl");
const UTILS: &str = "rules_rust/rust/private/utils.bzl";
const LIBS: &[&str] = &[
    "liballoc.rlib",
    "libcompiler_builtins.rlib",
    "libcore.rlib",
    "libpanic_abort.rlib",
    "libpanic_unwind.rlib",
    "libmemchr.rlib",
    "libstd.rlib",
    "libtest.rlib",
    "libstd.so",
];

fn source(root: &std::path::Path, marker: &str) {
    fs::write(
        root.join("rules_rust/rust/private/callback_proof.bzl"),
        PROBE.replace("MARKER", marker),
    )
    .unwrap();
}

fn workspace() -> PathBuf {
    let root = super::cc_toolchain::workspace();
    source(&root, "A");
    // Exercise the real built-in host alias and native constraint/platform rules.
    for (path, contents) in [
        ("platforms/REPO.bazel", ""),
        ("platforms/.bazelignore", ""),
        (
            "platforms/cpu/BUILD.bazel",
            "package(default_visibility=['//visibility:public'])\nconstraint_setting(name='cpu')\nconstraint_value(name='x86_64',constraint_setting=':cpu')\n",
        ),
        (
            "platforms/os/BUILD.bazel",
            "package(default_visibility=['//visibility:public'])\nconstraint_setting(name='os')\nconstraint_value(name='linux',constraint_setting=':os')\n",
        ),
        (
            "platforms/host/BUILD.bazel",
            "package(default_visibility=['//visibility:public'])\nplatform(name='host',constraint_values=['@@platforms//cpu:x86_64','@@platforms//os:linux'])\n",
        ),
    ] {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    let exports: Vec<_> = LIBS
        .iter()
        .copied()
        .chain(["rustc", "rustdoc", "rustc_lib"])
        .collect();
    for name in &exports {
        fs::write(
            root.join(name),
            "authored toolchain input; no tool executes\n",
        )
        .unwrap();
    }
    let existing = fs::read_to_string(root.join("BUILD.bazel")).unwrap();
    fs::write(
        root.join("BUILD.bazel"),
        format!("{existing}\nexports_files({exports:?})\n"),
    )
    .unwrap();
    let module = fs::read_to_string(root.join("MODULE.bazel")).unwrap();
    fs::write(
        root.join("MODULE.bazel"),
        format!("{module}\nregister_toolchains('@rules_rust//:cpp')\n"),
    )
    .unwrap();
    let existing = fs::read_to_string(root.join("rules_rust/BUILD.bazel")).unwrap();
    let libs: Vec<_> = LIBS.iter().map(|name| format!("@@//:{name}")).collect();
    fs::write(root.join("rules_rust/BUILD.bazel"), format!(
        "load('//rust/private:toolchain.bzl','rust_toolchain','rust_stdlib_filegroup')\n\
        load('//rust/private:callback_proof.bzl','allocator','relay',rust_callback_consumer='consumer')\n\
        {existing}\n\
        toolchain(name='cpp',toolchain=':producer',toolchain_type='@bazel_tools//tools/cpp:toolchain_type',\
            exec_compatible_with=['@@platforms//cpu:x86_64','@@platforms//os:linux'])\n\
        filegroup(name='rustc_lib_files',srcs=['@@//:rustc_lib'])\n\
        allocator(name='allocator')\n\
        rust_stdlib_filegroup(name='stdlib',srcs={libs:?})\n\
        rust_toolchain(name='rust_producer',rustc='@@//:rustc',rust_doc='@@//:rustdoc',\
            rustc_lib=':rustc_lib_files',rust_std=':stdlib',allocator_library=':allocator',\
            global_allocator_library=':allocator',binary_ext='',dylib_ext='.so',staticlib_ext='.a',stdlib_linkflags=[],\
            exec_triple='x86_64-unknown-linux-gnu',target_triple='x86_64-unknown-linux-gnu')\n\
        relay(name='relay',dep=':rust_producer')\n\
        relay(name='nested',dep=':relay',nested=True)\n\
        rust_callback_consumer(name='rust_consumer',dep=':nested')\n\
        rust_callback_consumer(name='rust_fresh_consumer',dep=':nested')\n"
    )).unwrap();
    let settings = root.join("rules_rust/rust/settings");
    fs::create_dir_all(&settings).unwrap();
    let mut build = String::from(
        "load('@bazel_skylib//rules:common_settings.bzl','bool_flag','int_flag','string_flag')\n\
        load('//rust/private:lto.bzl','rust_lto_flag')\n\
        load(':incompatible.bzl','incompatible_flag')\n\
        package(default_visibility=['//visibility:public'])\n",
    );
    for name in [
        "experimental_compile_rustdoc_tests",
        "experimental_use_allocator_libraries_with_mangled_symbols",
        "experimental_use_coverage_metadata_files",
        "experimental_use_global_allocator",
        "no_std",
        "pipelined_compilation",
        "rename_first_party_crates",
        "toolchain_generated_sysroot",
        "experimental_link_std_dylib",
        "experimental_use_cc_common_link",
        "require_explicit_unstable_features",
    ] {
        build.push_str(&format!(
            "bool_flag(name='{name}',build_setting_default=False)\n"
        ));
    }
    build.push_str(
        "int_flag(name='codegen_units',build_setting_default=1)\n\
        string_flag(name='toolchain_linker_preference',build_setting_default='none')\n\
        string_flag(name='third_party_dir',build_setting_default='third_party')\n\
        rust_lto_flag(name='lto',build_setting_default='off')\n",
    );
    for name in [
        "incompatible_do_not_include_data_in_compile_data",
        "incompatible_do_not_include_transitive_data_in_compile_inputs",
    ] {
        build.push_str(&format!("incompatible_flag(name='{name}',build_setting_default=False,issue='authored fixture')\n"));
    }
    fs::write(settings.join("BUILD.bazel"), build).unwrap();
    root
}

fn configured(dynamic: bool) -> ConfigurationKey {
    let configuration = config(&[(
        NativeCommandOption::ForcePic,
        if dynamic { "true" } else { "false" },
    )]);
    ConfigurationKey::from_slug(
        configuration
            .slug_configuration()
            .unwrap()
            .with_host_platform_label(
                &CanonicalLabel::parse("@@bazel_tools//tools:host_platform").unwrap(),
            )
            .with_starlark_option(StarlarkOption::new(
                CanonicalLabel::parse("@@rules_rust+//rust/settings:experimental_link_std_dylib")
                    .unwrap(),
                StarlarkOptionValue::Boolean(dynamic),
                StarlarkOptionScope::Default,
            )),
    )
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

fn callback(result: &ConfiguredNodeResult) -> &RustToolchainCallable {
    let AnalysisValueKind::RustToolchainCallable(value) = field(result, "callback").kind() else {
        panic!("genuine original callback retained")
    };
    value
}

fn check(result: &ConfiguredNodeResult, target: &str, marker: &str, dynamic: bool) {
    let AnalysisValueKind::List(values) = field(result, "libraries").kind() else {
        panic!("library list")
    };
    let names: Vec<_> = values.iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(
        names,
        if dynamic {
            vec!["libtest.a", "libstd.so"]
        } else {
            vec![
                "libtest.a",
                "libstd.a",
                "libpanic_unwind.a",
                "libmemchr.a",
                "libcore.a",
                "libcompiler_builtins.a",
                "liballoc.a",
            ]
        }
    );
    assert_eq!(field(result, "marker").as_str(), Some(marker));
    let model = callback(result).model();
    assert!(matches!(
        model.cc_toolchain.kind(),
        AnalysisValueKind::Provider(_)
    ));
    assert!(matches!(
        model.feature_configuration.kind(),
        AnalysisValueKind::CcFeatureConfiguration(_)
    ));
    assert!(
        matches!(model.experimental_link_std_dylib.kind(), AnalysisValueKind::Boolean(value) if value == dynamic)
    );
    assert!(matches!(
        model.rust_std.kind(),
        AnalysisValueKind::ConfiguredTarget(_)
    ));
    assert_eq!(
        model.source.caller.identity.label.to_string(),
        "@@rules_rust+//rust/private:toolchain.bzl"
    );
    assert_eq!(
        model.source.helper.root.label.to_string(),
        "@@rules_rust+//rust/private:rust_allocator_libraries.bzl"
    );
    let writes: Vec<_> = result
        .actions()
        .iter()
        .filter(|action| action.mnemonic() == "FileWrite")
        .collect();
    assert_eq!(writes.len(), 1);
    assert_eq!(
        writes[0].kind(),
        &slug_build_api_v2::ActionKind::Write {
            content: format!("{marker}:{}", names.join(",")),
            is_executable: false,
        }
    );
    assert!(
        writes[0].outputs()[0]
            .root_relative_path()
            .ends_with(&format!("external/rules_rust+/{target}.txt"))
    );
    assert_eq!(
        result
            .actions()
            .iter()
            .filter(|action| action.solib_symlink_spec().is_some())
            .count(),
        usize::from(dynamic)
    );
    assert_eq!(result.actions().len(), 1 + usize::from(dynamic));
}

fn utils_digest(result: &ConfiguredNodeResult) -> [u8; 32] {
    callback(result)
        .model()
        .source
        .helper
        .reachable
        .iter()
        .find(|source| source.identity.label.to_string() == "@@rules_rust+//rust/private:utils.bzl")
        .unwrap()
        .digest
}

#[tokio::test]
async fn original_rust_callback_publishes_aliases_invokes_fresh_consumers_and_restores_imports() {
    for observed in [false, true] {
        let root = workspace();
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let fast = configured(false);
        let first = request(&dice, &root, observed, "rust_consumer", fast.clone())
            .await
            .unwrap();
        check(&first, "rust_consumer", "A", false);
        let fresh = request(&dice, &root, observed, "rust_fresh_consumer", fast.clone())
            .await
            .unwrap();
        check(&fresh, "rust_fresh_consumer", "A", false);
        assert!(field(&first, "callback").publication_eq(field(&fresh, "callback")));
        let changed = request(&dice, &root, observed, "rust_consumer", configured(true))
            .await
            .unwrap();
        check(&changed, "rust_consumer", "A", true);
        assert!(!field(&first, "callback").publication_eq(field(&changed, "callback")));
        assert_eq!(
            request(&dice, &root, observed, "rust_consumer", fast.clone())
                .await
                .unwrap(),
            first
        );
        source(&root, "B");
        let changed = request(&dice, &root, observed, "rust_consumer", fast.clone())
            .await
            .unwrap();
        check(&changed, "rust_consumer", "B", false);
        assert_ne!(changed, first);
        source(&root, "A");
        assert_eq!(
            request(&dice, &root, observed, "rust_consumer", fast.clone())
                .await
                .unwrap(),
            first
        );

        let path = root.join(UTILS);
        let original = fs::read(&path).unwrap();
        let mut edited = original.clone();
        edited.extend_from_slice(b"\n# authored transitive import discriminator\n");
        fs::write(&path, edited).unwrap();
        let changed = request(&dice, &root, observed, "rust_consumer", fast.clone())
            .await
            .unwrap();
        check(&changed, "rust_consumer", "A", false);
        assert_ne!(utils_digest(&changed), utils_digest(&first));
        assert_ne!(
            callback(&changed).model().source.helper.fingerprint,
            callback(&first).model().source.helper.fingerprint
        );
        assert!(!field(&changed, "callback").publication_eq(field(&first, "callback")));
        fs::write(path, original).unwrap();
        let restored = request(&dice, &root, observed, "rust_consumer", fast)
            .await
            .unwrap();
        check(&restored, "rust_consumer", "A", false);
        assert_eq!(utils_digest(&restored), utils_digest(&first));
        assert_eq!(
            callback(&restored).model().source,
            callback(&first).model().source
        );
        assert_eq!(restored, first);
        fs::remove_dir_all(root).unwrap();
    }
}
