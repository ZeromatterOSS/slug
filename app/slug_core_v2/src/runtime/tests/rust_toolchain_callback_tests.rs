// Private Core integration of the genuine coupled callback proof.
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::AnalysisValueKind;
use slug_build_api_v2::RustToolchainCallable;
use slug_configuration_v2::NativeCommandOption;
use slug_configuration_v2::StarlarkOptionValue;

use super::*;

const PROBE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../slug_analysis_v2/tests/rustc_map_each/rust_toolchain_callback.bzl"
));

include!("rust_toolchain_callback_fixture.rs");

#[derive(Clone)]
struct CallbackAnalysisRoot {
    workspace: NormalizedAbsolutePath,
    requested: CanonicalLabel,
    configuration: ConfigurationKey,
    observed: bool,
}

#[async_trait]
impl NativeCommandRoot for CallbackAnalysisRoot {
    type Terminal = Arc<ConfiguredNodeResult>;

    // Both preparation APIs return actual keys, not an observed epoch. Retain
    // observations=None, ClosureOnly and the default strict terminal policies.
    async fn compute(
        &self,
        transaction: &mut dice::DiceTransaction,
    ) -> Result<slug_bzlmod_v2::SourcePreparationOutcome<Self::Terminal>, NativeDemandSessionError>
    {
        use slug_bzlmod_v2::SourcePreparationOutcome::Complete;
        use slug_bzlmod_v2::SourcePreparationOutcome::Need;

        let value = if self.observed {
            let key = match prepare_configured_node_analysis_observed(
                transaction,
                self.workspace.dupe(),
                self.requested.clone(),
                self.configuration.clone(),
            )
            .await
            {
                Need(need) => return Ok(Need(need)),
                Complete(Err(frontier)) => {
                    return Err(NativeDemandSessionError::Computation(frontier.into()));
                }
                Complete(Ok(Err(error))) => {
                    return Err(NativeDemandSessionError::Computation(error.into()));
                }
                Complete(Ok(Ok(key))) => key,
            };
            match transaction
                .compute(&key)
                .await
                .map_err(|error| NativeDemandSessionError::Computation(error.into()))?
            {
                Need(need) => return Ok(Need(need)),
                Complete(Err(frontier)) => {
                    return Err(NativeDemandSessionError::Computation(frontier.into()));
                }
                Complete(Ok(value)) => value,
            }
        } else {
            let key = match prepare_configured_node_analysis(
                transaction,
                self.workspace.dupe(),
                self.requested.clone(),
                self.configuration.clone(),
            )
            .await
            {
                Need(need) => return Ok(Need(need)),
                Complete(Err(error)) => {
                    return Err(NativeDemandSessionError::Computation(error.into()));
                }
                Complete(Ok(key)) => key,
            };
            match transaction
                .compute(&key)
                .await
                .map_err(|error| NativeDemandSessionError::Computation(error.into()))?
            {
                Need(need) => return Ok(Need(need)),
                Complete(value) => value,
            }
        };
        let result = value
            .as_ref()
            .as_ref()
            .cloned()
            .map_err(|error| NativeDemandSessionError::Computation(error.clone().into()))?;
        Ok(Complete(result))
    }
}

fn field<'a>(result: &'a ConfiguredNodeResult, name: &str) -> &'a AnalysisValue {
    result
        .providers()
        .occurrences()
        .find_map(|provider| provider.field(name))
        .unwrap()
}

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
    "libstd-fixture.so",
];

fn source(root: &std::path::Path, marker: &str) {
    fs::write(
        root.join("rules_rust/rust/private/callback_proof.bzl"),
        PROBE.replace("MARKER", marker),
    )
    .unwrap();
}

fn workspace() -> (PathBuf, tempfile::TempDir) {
    let (root, archives) = assemble_callback_base_workspace();
    source(&root, "A");
    let exports: Vec<_> = LIBS
        .iter()
        .copied()
        .filter(|name| !name.ends_with(".rlib"))
        .chain(["rustc", "rustdoc", "rustc_lib"])
        .collect();
    for name in &exports {
        fs::write(
            root.join(name),
            "authored toolchain input; no tool executes\n",
        )
        .unwrap();
    }
    // The original stdlib rule declares .a siblings of .rlib inputs. Their
    // source repository/package must match that genuine producer's package.
    let rlibs: Vec<_> = LIBS
        .iter()
        .copied()
        .filter(|name| name.ends_with(".rlib"))
        .collect();
    for name in &rlibs {
        fs::write(
            root.join("rules_rust").join(name),
            "authored stdlib input; no tool executes\n",
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
    let libs: Vec<_> = LIBS
        .iter()
        .map(|name| {
            if name.ends_with(".rlib") {
                format!(":{name}")
            } else {
                format!("@@//:{name}")
            }
        })
        .collect();
    fs::write(root.join("rules_rust/BUILD.bazel"), format!(
        "load('//rust/private:toolchain.bzl','rust_toolchain','rust_stdlib_filegroup')\n\
        load('//rust/private:callback_proof.bzl','allocator','relay',rust_callback_consumer='consumer')\n\
        {existing}\n\
        exports_files({rlibs:?})\n\
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
    (root, archives)
}

fn configured(dynamic: bool) -> ConfigurationKey {
    let base = build_test_configuration("target");
    let overlay: CommandConfigurationOverlay = vec![CommandConfigurationOccurrence::native(
        NativeCommandOption::ForcePic,
        Some(if dynamic { "true" } else { "false" }),
        false,
    )]
    .into();
    let configuration = ConfigurationKey::from_slug(
        base.slug_configuration()
            .unwrap()
            .with_command_configuration(base.starlark_options().clone(), &overlay)
            .unwrap(),
    );
    ConfigurationKey::from_slug(
        configuration
            .slug_configuration()
            .unwrap()
            .with_host_platform_label(&CanonicalLabel::parse("@@platforms//host:host").unwrap())
            .with_starlark_option(StarlarkOption::new(
                CanonicalLabel::parse("@@rules_rust+//rust/settings:experimental_link_std_dylib")
                    .unwrap(),
                StarlarkOptionValue::Boolean(dynamic),
                StarlarkOptionScope::Default,
            )),
    )
}

// Legacy MODULE/package keys consume the existing immutable source snapshots.
// Refresh actual source inputs on the same runtime before every request; normal
// demand driving still owns all generated repositories and their receipts.
fn refresh_callback_sources(runtime: &WorkspaceRuntime) {
    let observations = observe_workspace(&runtime.workspace).unwrap();
    let files = observations
        .files
        .into_iter()
        .map(|observation| {
            let observation = runtime.validate_file_observation(observation).unwrap();
            (observation.path, observation.value)
        })
        .collect();
    let raw_files = observations
        .raw_files
        .into_iter()
        .map(|observation| {
            let observation = runtime.validate_raw_file_observation(observation).unwrap();
            (observation.path, observation.value)
        })
        .collect();
    let directories = observations
        .directories
        .into_iter()
        .map(|observation| {
            let observation = runtime.validate_directory_observation(observation).unwrap();
            (observation.path, observation.value)
        })
        .collect();
    runtime.runtime.block_on(async {
        let mut updater = runtime.dice.updater();
        updater
            .changed_to(vec![(
                WorkspaceSnapshotKey {
                    workspace: runtime.workspace.clone(),
                },
                Arc::new(WorkspaceSnapshot {
                    files: Arc::new(files),
                }),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                WorkspaceRawSnapshotKey {
                    workspace: runtime.workspace.clone(),
                },
                Arc::new(WorkspaceRawSnapshot {
                    files: Arc::new(raw_files),
                }),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                WorkspaceDirectorySnapshotKey {
                    workspace: runtime.workspace.clone(),
                },
                Arc::new(WorkspaceDirectorySnapshot {
                    directories: Arc::new(directories),
                }),
            )])
            .unwrap();
        drop(runtime.request_revision.commit(updater).await);
    });
}

fn request(
    runtime: &WorkspaceRuntime,
    root: &Path,
    observed: bool,
    target: &str,
    configuration: ConfigurationKey,
) -> Result<Arc<ConfiguredNodeResult>, NativeDemandSessionError> {
    refresh_callback_sources(runtime);
    let mut inputs = NativeDemandRequestInputBundle::normalized_initial();
    inputs.registry_urls = RegistryUrls::new([format!("file://{}/registry", root.display())]);
    inputs.repository_environment = slug_bzlmod_v2::RepositoryEnvironmentSnapshot::empty();
    let root = CallbackAnalysisRoot {
        workspace: NormalizedAbsolutePath::new(root.to_path_buf()).unwrap(),
        requested: CanonicalLabel::parse(&format!("@@rules_rust+//:{target}")).unwrap(),
        configuration,
        observed,
    };
    let driven = runtime.drive_command(inputs, root)?;
    assert!(driven.terminal_root_count > 0);
    Ok(driven.accepted.terminal_for_test().dupe())
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
            vec!["libtest.a", "libstd-fixture.so"]
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
    let solibs: Vec<_> = result
        .actions()
        .iter()
        .filter(|action| action.solib_symlink_spec().is_some())
        .collect();
    assert_eq!(solibs.len(), usize::from(dynamic));
    if dynamic {
        let action = solibs[0];
        let spec = action.solib_symlink_spec().unwrap();
        let owner = result.configured_target_key().unwrap();
        assert_eq!(action.context().owner(), owner);
        assert_eq!(
            owner.label().to_string(),
            format!("@@rules_rust+//:{target}")
        );
        assert_eq!(
            spec.input(),
            &slug_build_api_v2::AnalysisArtifact::Source(
                CanonicalLabel::parse("@@//:libstd-fixture.so").unwrap()
            )
        );
        assert_eq!(
            spec.output().root(),
            &slug_build_api_v2::DerivedArtifactRoot::bin(
                owner.configuration().slug_configuration().unwrap().clone()
            )
            .unwrap()
        );
        let consumer_component = match target {
            "rust_consumer" => "rust_Uconsumer",
            "rust_fresh_consumer" => "rust_Ufresh_Uconsumer",
            _ => panic!("unexpected consumer"),
        };
        assert_eq!(
            spec.output().root_relative_path(),
            format!("_solib_k8/_U_A_Arules_Urust+_S_S_C{consumer_component}___U/libstd-fixture.so")
        );
        assert_eq!(action.outputs(), &[spec.output().clone()]);
    }
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

#[test]
fn original_rust_callback_publishes_aliases_invokes_fresh_consumers_and_restores_imports() {
    for observed in [false, true] {
        let (root, _archives) = workspace();
        let runtime = test_runtime(&root).unwrap();
        let fast = configured(false);
        // Cold materialization creates immutable roots under /tmp and changes
        // genuinely observed ancestor metadata. Establish the warm-source
        // baseline once; never retry publication equality until it succeeds.
        let warm = request(&runtime, &root, observed, "rust_consumer", fast.clone()).unwrap();
        check(&warm, "rust_consumer", "A", false);
        let first = request(&runtime, &root, observed, "rust_consumer", fast.clone()).unwrap();
        let first_inputs = accepted_native_snapshot(&runtime);
        check(&first, "rust_consumer", "A", false);
        let fresh = request(
            &runtime,
            &root,
            observed,
            "rust_fresh_consumer",
            fast.clone(),
        )
        .unwrap();
        let fresh_inputs = accepted_native_snapshot(&runtime);
        let changed_paths: Vec<_> = first_inputs
            .path_observations
            .observations()
            .iter()
            .filter(|(demand, value)| {
                fresh_inputs
                    .path_observations
                    .get(demand)
                    .is_some_and(|fresh| fresh != *value)
            })
            .map(|(demand, _)| {
                format!(
                    "{:?}:{:?}:{:?}",
                    demand.namespace(),
                    demand.path(),
                    demand.operation()
                )
            })
            .collect();
        let shared_paths = first_inputs
            .path_observations
            .observations()
            .iter()
            .filter(|(demand, _)| fresh_inputs.path_observations.get(demand).is_some())
            .count();
        assert!(
            shared_paths > 0,
            "warm publication compares real shared observations"
        );
        assert!(
            changed_paths.is_empty(),
            "warm publication inputs changed: {changed_paths:?}"
        );
        assert!(
            first_inputs
                .path_observations
                .observations()
                .keys()
                .any(
                    |demand| demand.namespace() == PathObservationNamespace::Host
                        && demand.path().as_path() == Path::new("/tmp")
                        && demand.operation() == PathObservationOperation::Lstat
                        && fresh_inputs.path_observations.get(demand).is_some()
                ),
            "the previously changing ancestor remains observed"
        );
        check(&fresh, "rust_fresh_consumer", "A", false);
        let first_model = callback(&first).model();
        let fresh_model = callback(&fresh).model();
        let AnalysisValueKind::Provider(first_cc) = first_model.cc_toolchain.kind() else {
            panic!("CC provider")
        };
        let AnalysisValueKind::Provider(fresh_cc) = fresh_model.cc_toolchain.kind() else {
            panic!("fresh CC provider")
        };
        let changed_cc_fields: Vec<_> = first_cc
            .fields()
            .iter()
            .filter_map(|(name, value)| {
                (!fresh_cc
                    .field(name)
                    .is_some_and(|fresh| value.publication_eq(fresh)))
                .then_some(name.as_str())
            })
            .collect();
        assert!(
            field(&first, "callback").publication_eq(field(&fresh, "callback")),
            "route_observed={observed}; caller_eq={}; helper_eq={}; captures_eq={:?}; cc_identity_eq={}; cc_schema_eq={}; cc_field_counts={:?}; changed_cc_fields={changed_cc_fields:?}",
            first_model.source.caller == fresh_model.source.caller,
            first_model.source.helper == fresh_model.source.helper,
            [
                first_model
                    .cc_toolchain
                    .publication_eq(&fresh_model.cc_toolchain),
                first_model
                    .feature_configuration
                    .publication_eq(&fresh_model.feature_configuration),
                first_model
                    .experimental_link_std_dylib
                    .publication_eq(&fresh_model.experimental_link_std_dylib),
                first_model.rust_std.publication_eq(&fresh_model.rust_std)
            ],
            first_cc.identity() == fresh_cc.identity(),
            first_cc.has_schema() == fresh_cc.has_schema(),
            (first_cc.fields().len(), fresh_cc.fields().len())
        );

        let changed =
            request(&runtime, &root, observed, "rust_consumer", configured(true)).unwrap();
        check(&changed, "rust_consumer", "A", true);
        let fresh_dynamic = request(
            &runtime,
            &root,
            observed,
            "rust_fresh_consumer",
            configured(true),
        )
        .unwrap();
        check(&fresh_dynamic, "rust_fresh_consumer", "A", true);
        assert!(field(&changed, "callback").publication_eq(field(&fresh_dynamic, "callback")));
        let solib_output = |result: &ConfiguredNodeResult| {
            result
                .actions()
                .iter()
                .find_map(|action| {
                    action
                        .solib_symlink_spec()
                        .map(|spec| spec.output().clone())
                })
                .unwrap()
        };
        assert_ne!(solib_output(&changed), solib_output(&fresh_dynamic));
        assert!(!field(&first, "callback").publication_eq(field(&changed, "callback")));
        assert_eq!(
            request(&runtime, &root, observed, "rust_consumer", fast.clone()).unwrap(),
            first
        );
        source(&root, "B");
        let changed = request(&runtime, &root, observed, "rust_consumer", fast.clone()).unwrap();
        check(&changed, "rust_consumer", "B", false);
        assert_ne!(changed, first);
        source(&root, "A");
        assert_eq!(
            request(&runtime, &root, observed, "rust_consumer", fast.clone()).unwrap(),
            first
        );

        let path = root.join(UTILS);
        let original = fs::read(&path).unwrap();
        let mut edited = original.clone();
        edited.extend_from_slice(b"\n# authored transitive import discriminator\n");
        fs::write(&path, edited).unwrap();
        let changed = request(&runtime, &root, observed, "rust_consumer", fast.clone()).unwrap();
        check(&changed, "rust_consumer", "A", false);
        assert_ne!(utils_digest(&changed), utils_digest(&first));
        assert_ne!(
            callback(&changed).model().source.helper.fingerprint,
            callback(&first).model().source.helper.fingerprint
        );
        assert!(!field(&changed, "callback").publication_eq(field(&first, "callback")));
        fs::write(path, original).unwrap();
        let restored = request(&runtime, &root, observed, "rust_consumer", fast).unwrap();
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
