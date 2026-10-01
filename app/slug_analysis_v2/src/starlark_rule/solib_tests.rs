//! Native construction through the production synchronous action sink.
use slug_build_api_v2::RunfilesRepositoryMapping;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::ActionEnvironmentHost;
use slug_configuration_v2::native::host::ActionEnvironmentHostOs;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_identity_v2::CanonicalLabel;

use super::*;
use crate::execution_groups::ConfiguredExecGroupRow;
use crate::result::ToolchainTopology;

fn sink() -> Arc<SynchronousAnalysisActionSink> {
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap()
    .with_action_environment_host(ActionEnvironmentHost::without_environment(
        ActionEnvironmentHostOs::Linux,
    ));
    let key = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@rules_rust+//ffi/cc/allocator_library:owner").unwrap(),
        crate::ConfigurationKey::from_slug(SlugConfiguration::default_target(&host).unwrap()),
    );
    sink_for(key)
}

fn sink_for(key: ConfiguredTargetKey) -> Arc<SynchronousAnalysisActionSink> {
    let context = Arc::new(ConfiguredActionOwnerContext::unresolved_default(key.clone()).unwrap());
    let group = ConfiguredExecGroupRow::new(
        ConfiguredExecGroup::Default,
        Arc::from([]),
        Arc::from([]),
        context,
    )
    .unwrap();
    Arc::new(SynchronousAnalysisActionSink {
        workspace_status: Arc::new(slug_build_api_v2::WorkspaceStatusArtifacts::new(
            slug_workspace_v2::NormalizedAbsolutePath::new("/fixture/workspace").unwrap(),
        )),
        root: DerivedArtifactRoot::bin(key.configuration().slug_configuration().unwrap().clone())
            .unwrap(),
        actions: Arc::new(Mutex::new(CtxActions::new(
            DerivedArtifactRoot::bin(key.configuration().slug_configuration().unwrap().clone())
                .unwrap(),
        ))),
        declared_outputs: Mutex::new(SmallMap::new()),
        package_path: "ffi/cc/allocator_library".to_owned(),
        owner: key.artifact_owner(),
        typed_configuration: Ok(None),
        executable_provenance: Arc::new(ExecutableArtifactProvenance::default()),
        execution_tags: CanonicalStringMap::default(),
        target_package: Arc::new(RunfilesPackageMetadata::new(
            key.label().package().clone(),
            Arc::new(RunfilesRepositoryMapping::empty()),
        )),
        exec_groups: ConfiguredExecGroupCollection::new(
            vec![group],
            Arc::from([]),
            ToolchainTopology::new(Vec::new(), None).unwrap(),
            false,
        )
        .unwrap(),
    })
}

fn generated(configuration: &[u8], kind: ActionOutputKind) -> AnalysisArtifact {
    AnalysisArtifact::Derived {
        owner: AnalysisConfiguredTargetKey::new(
            CanonicalLabel::parse("@@rules_rust+//ffi/cc/allocator_library:producer").unwrap(),
            Arc::<[u8]>::from(configuration),
        ),
        output: ActionOutput::new(
            crate::analysis_value::test_artifact_root(),
            "external/rules_rust+/ffi/cc/allocator_library/liballocator.so",
            kind,
        ),
    }
}

#[test]
fn native_solib_sink_keeps_input_owner_and_bin_relative_output() {
    let sink = sink();
    let module = Module::new();
    let input = generated(b"producer-A", ActionOutputKind::File);
    let value = module
        .heap()
        .alloc_simple(AnalysisArtifactValue::new(input.clone()));
    let context = AnalysisEvaluationContext::new(
        Arc::from([]),
        [],
        sink.owner.label().clone(),
        sink.clone(),
        FrozenValue::new_none(),
        Arc::from([]),
    );
    let actions = AnalysisActions::new(
        sink.clone(),
        context.root_token(),
        "rule context",
        AnalysisActionCallScope::Root,
    );
    let output = actions
        .register_dynamic_library_symlink(value, "_solib_k8", false, false)
        .unwrap();
    let repeated = actions
        .register_dynamic_library_symlink(value, "_solib_k8", false, false)
        .unwrap();
    assert_eq!(output.artifact(), repeated.artifact());
    let AnalysisArtifact::Derived { owner, output } = output.artifact() else {
        panic!("derived output required")
    };
    assert_eq!(owner, &sink.owner);
    assert_eq!(
        output.root_relative_path(),
        "_solib_k8/libexternal_Srules_Urust+_Sffi_Scc_Sallocator_Ulibrary_Sliballocator.so"
    );
    assert_eq!(output.kind(), ActionOutputKind::File);
    let registry = sink.actions.lock().unwrap();
    assert_eq!(registry.registry().actions().len(), 1);
    let action = &registry.registry().actions()[0];
    assert_eq!(action.solib_symlink_spec().unwrap().input(), &input);
    assert_eq!(action.solib_symlink_spec().unwrap().output(), output);
    assert!(action.symlink_spec().is_none());
}

#[test]
fn native_solib_sink_rejects_types_collisions_and_declared_directory() {
    let module = Module::new();
    let sink = sink();
    assert!(
        sink.dynamic_library_symlink(Value::new_none(), "_solib_k8", false, false)
            .unwrap_err()
            .to_string()
            .contains("must be a File")
    );
    for artifact in [generated(b"A", ActionOutputKind::Directory)] {
        let value = module
            .heap()
            .alloc_simple(AnalysisArtifactValue::new(artifact));
        assert!(
            sink.dynamic_library_symlink(value, "_solib_k8", false, false)
                .is_err()
        );
    }
    let value = module
        .heap()
        .alloc_simple(AnalysisArtifactValue::new(generated(
            b"A",
            ActionOutputKind::File,
        )));
    let output = sink
        .dynamic_library_symlink(value, "_solib_k8", false, false)
        .unwrap();
    let changed = module
        .heap()
        .alloc_simple(AnalysisArtifactValue::new(generated(
            b"B",
            ActionOutputKind::File,
        )));
    assert!(
        sink.dynamic_library_symlink(changed, "_solib_k8", false, false)
            .is_err()
    );
    assert_eq!(sink.actions.lock().unwrap().registry().actions().len(), 1);
    let fresh = self::sink();
    let AnalysisArtifact::Derived { output, .. } = output.artifact() else {
        unreachable!()
    };
    fresh
        .declare_bin_relative_output(
            output.root_relative_path().to_owned(),
            ActionOutputKind::Directory,
        )
        .unwrap();
    assert!(
        fresh
            .dynamic_library_symlink(value, "_solib_k8", false, false)
            .unwrap_err()
            .to_string()
            .contains("different type")
    );
    assert!(
        fresh
            .actions
            .lock()
            .unwrap()
            .registry()
            .actions()
            .is_empty()
    );
}

#[test]
fn native_solib_sink_configured_input_a_b_a_restores_structural_action() {
    let module = Module::new();
    let mut snapshots = Vec::new();
    for configuration in [b"A", b"B", b"A"] {
        let sink = sink();
        let input = module
            .heap()
            .alloc_simple(AnalysisArtifactValue::new(generated(
                configuration,
                ActionOutputKind::File,
            )));
        sink.dynamic_library_symlink(input, "_solib_k8", false, false)
            .unwrap();
        snapshots.push(sink.actions.lock().unwrap().registry().actions()[0].clone());
    }
    assert_ne!(snapshots[0], snapshots[1]);
    assert_eq!(snapshots[0], snapshots[2]);
    assert_eq!(snapshots[0].outputs(), snapshots[1].outputs());
}

#[test]
fn source_stdlib_solib_uses_current_consumer_and_restores_configuration() {
    let module = Module::new();
    let baseline = sink();
    let target = baseline.root.configuration().clone();
    let execution = target.to_exec().unwrap();
    let input = AnalysisArtifact::Source(
        CanonicalLabel::parse("@@rust_linux+//lib/rustlib:libstd.so").unwrap(),
    );
    let library = module
        .heap()
        .alloc_simple(AnalysisArtifactValue::new(input.clone()));
    let mut snapshots = Vec::new();
    for configuration in [target.clone(), execution, target.clone()] {
        let key = ConfiguredTargetKey::new(
            baseline.owner.label().clone(),
            crate::ConfigurationKey::from_slug(configuration.clone()),
        );
        let sink = sink_for(key.clone());
        let context = AnalysisEvaluationContext::new(
            Arc::from([]),
            [],
            key.label().clone(),
            sink.clone(),
            FrozenValue::new_none(),
            Arc::from([]),
        );
        let actions = AnalysisActions::new(
            sink.clone(),
            context.root_token(),
            "rule context",
            AnalysisActionCallScope::Root,
        );
        let output = actions
            .register_dynamic_library_symlink(library, "_solib_k8", true, true)
            .unwrap();
        let repeated = actions
            .register_dynamic_library_symlink(library, "_solib_k8", true, true)
            .unwrap();
        assert_eq!(output.artifact(), repeated.artifact());
        let AnalysisArtifact::Derived { owner, output } = output.artifact() else {
            panic!("current consumer output")
        };
        assert_eq!(owner, &key.artifact_owner());
        assert_eq!(output.root().configuration(), &configuration);
        assert_eq!(
            output.root_relative_path(),
            "_solib_k8/_U_A_Arules_Urust+_S_Sffi_Scc_Sallocator_Ulibrary_Cowner___Uexternal_Srust_Ulinux+_Slib_Srustlib/libstd.so"
        );
        let registry = sink.actions.lock().unwrap();
        assert_eq!(registry.registry().actions().len(), 1);
        let action = &registry.registry().actions()[0];
        assert_eq!(action.solib_symlink_spec().unwrap().input(), &input);
        assert_eq!(action.solib_symlink_spec().unwrap().output(), output);
        snapshots.push(action.clone());
    }
    assert_ne!(snapshots[0], snapshots[1]);
    assert_eq!(snapshots[0], snapshots[2]);

    let mut source_snapshots = Vec::new();
    for label in [
        "@@rust_linux+//lib/rustlib:libstd.so",
        "@@rust_linux+//:lib/rustlib/libstd.so",
        "@@rust_linux+//lib/rustlib:libstd.so",
    ] {
        let current = sink();
        let source = AnalysisArtifact::Source(CanonicalLabel::parse(label).unwrap());
        let value = module
            .heap()
            .alloc_simple(AnalysisArtifactValue::new(source.clone()));
        current
            .dynamic_library_symlink(value, "_solib_k8", true, true)
            .unwrap();
        let registry = current.actions.lock().unwrap();
        let action = &registry.registry().actions()[0];
        assert_eq!(action.solib_symlink_spec().unwrap().input(), &source);
        source_snapshots.push(action.clone());
    }
    assert_eq!(source_snapshots[0].outputs(), source_snapshots[1].outputs());
    assert_ne!(source_snapshots[0], source_snapshots[1]);
    assert_eq!(source_snapshots[0], source_snapshots[2]);

    let other = sink_for(ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//other:consumer").unwrap(),
        crate::ConfigurationKey::from_slug(target),
    ));
    let output = other
        .dynamic_library_symlink(library, "_solib_k8", true, true)
        .unwrap();
    let AnalysisArtifact::Derived { owner, output } = output.artifact() else {
        panic!("other consumer output")
    };
    assert_eq!(owner, &other.owner);
    assert_eq!(
        output.root_relative_path(),
        "_solib_k8/_U_A_A_S_Sother_Cconsumer___Uexternal_Srust_Ulinux+_Slib_Srustlib/libstd.so"
    );
    assert_ne!(output, &snapshots[0].outputs()[0]);
}
