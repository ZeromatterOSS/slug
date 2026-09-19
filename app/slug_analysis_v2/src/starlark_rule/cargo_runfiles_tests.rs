//! Source-derived private-helper proof, not full Cargo loading/toolchain analysis.
//! Unrelated top-level imports/rule declarations are inert authored scaffolding.
//! The invoked helper/local _rlocationpath, ctx/Args, artifact and depset values,
//! retained lowering, and configured action publication are the production owners.
use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::ConfiguredTargetValue;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::BzlModuleSourceProvenance;
use slug_loading_v2::subrule_invocation::EvaluatorVectorMapEachGen;
use starlark::environment::FrozenModule;
use starlark::environment::Globals;
use starlark::environment::GlobalsBuilder;
use starlark::environment::LibraryExtension;
use starlark::eval::FileLoader;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;

const SOURCE: &str =
    include_str!("../../tests/fixtures/cargo-runfiles-args/cargo_build_script.bzl");
const LABEL: &str = "@@rules_rust+//cargo/private:cargo_build_script.bzl";
const FILENAME: &str = "/fixture/rules_rust/cargo/private/cargo_build_script.bzl";
const SHA: &str = "6147df938723ec58cef646169814c85a72fa0f74080471756e3c1891d02f4630";

#[derive(Debug)]
struct Declarations {
    owner: AnalysisConfiguredTargetKey,
    actions: Mutex<CtxActions>,
}
impl AnalysisActionSink for Declarations {
    fn declare_file(&self, path: &str) -> anyhow::Result<AnalysisArtifactValue> {
        let output = self
            .actions
            .lock()
            .unwrap()
            .declare_file(format!("pkg/{path}"))?;
        Ok(AnalysisArtifactValue::new(AnalysisArtifact::Derived {
            owner: self.owner.clone(),
            output,
        }))
    }
    fn declare_directory(&self, path: &str) -> anyhow::Result<AnalysisArtifactValue> {
        let output = self
            .actions
            .lock()
            .unwrap()
            .declare_directory(format!("pkg/{path}"))?;
        Ok(AnalysisArtifactValue::new(AnalysisArtifact::Derived {
            owner: self.owner.clone(),
            output,
        }))
    }
    fn write(&self, _: Value<'_>, _: Value<'_>, _: bool) -> anyhow::Result<()> {
        unreachable!("helper declares only a directory")
    }
    fn run_shell(&self, _: Value<'_>, _: &str, _: Value<'_>) -> anyhow::Result<()> {
        unreachable!()
    }
    fn run(&self, _: AnalysisRunRequest<'_>) -> anyhow::Result<()> {
        unreachable!()
    }
    fn artifact_symlink(
        &self,
        _: Value<'_>,
        _: Value<'_>,
        _: bool,
        _: Option<&str>,
    ) -> anyhow::Result<()> {
        unreachable!()
    }
    fn absolute_symlink(&self, _: Value<'_>, _: &str, _: Option<&str>) -> anyhow::Result<()> {
        unreachable!()
    }
}

// The helper calls only depset(transitive=...). Build the ordinary typed DAG;
// no flattening or callback behavior is implemented by this bridge.
#[starlark_module]
fn typed_globals(builder: &mut GlobalsBuilder) {
    fn depset<'v>(
        #[starlark(require = named)] transitive: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let children =
            ListRef::from_value(transitive).ok_or_else(|| anyhow::anyhow!("transitive list"))?;
        let mut lowerer = AnalysisValueLowerer::default();
        let children = children
            .iter()
            .map(|value| {
                let lowered = lowerer
                    .lower(value, "test depset")
                    .map_err(anyhow::Error::msg)?;
                let AnalysisValueKind::Depset(value) = lowered.kind() else {
                    anyhow::bail!("typed depset required");
                };
                Ok(value.clone())
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let retained = AnalysisDepset::new(DepsetOrder::Default, Vec::new(), children)?;
        AnalysisValueMaterializer::new(eval.frozen_heap())
            .value(&AnalysisValue::depset(retained))
            .map(FrozenValue::to_value)
            .map_err(anyhow::Error::msg)
    }
}
fn globals() -> Globals {
    GlobalsBuilder::extended_by(&[LibraryExtension::StructType])
        .with(typed_globals)
        .build()
}
fn evaluate(
    module: &Module,
    filename: &str,
    source: &str,
    globals: &Globals,
) -> anyhow::Result<()> {
    let ast = AstModule::parse(filename, source.to_owned(), &Dialect::Bazel)
        .map_err(starlark::Error::into_anyhow)?;
    Evaluator::new(module)
        .eval_module(ast, globals)
        .map_err(starlark::Error::into_anyhow)?;
    Ok(())
}
struct InertImports(FrozenModule);
impl FileLoader for InertImports {
    fn load(&self, _: &str) -> starlark::Result<FrozenModule> {
        Ok(self.0.clone())
    }
}
fn cargo_module(source: &str) -> anyhow::Result<FrozenModule> {
    let scaffold = Module::new();
    evaluate(
        &scaffold,
        "inert_imports.bzl",
        r#"def inert(*args, **kwargs): return None
def identity(value): return value
def placeholder(): return 'unused'
attr = struct(label=inert, label_list=inert, string=inert, string_list=inert, string_dict=inert, bool=inert, int=inert, output=inert)
rule = inert
Label = identity
config_common = struct(toolchain_type=inert)
platform_common = None
OutputGroupInfo = inert
apple_support = struct(path_placeholders=struct(xcode=placeholder, sdkroot=placeholder))
paths = None
BuildSettingInfo = None
ACTION_NAMES = None
cc_common = None
rust_common = struct(crate_info=None)
BuildInfo = None
CrateGroupInfo = None
DepInfo = None
get_compilation_mode_opts = inert
get_linker_and_args = inert
dedent = identity
deduplicate = inert
expand_dict_value_locations = inert
find_cc_toolchain = inert
find_toolchain = inert
name_to_crate_name = identity
"#,
        &globals(),
    )?;
    let loader = InertImports(scaffold.freeze()?);
    let module = Module::new();
    module.import_public_symbols(&loader.0);
    let default_info = slug_loading_v2::provider::alloc_starlark_provider_callable(
        module.frozen_heap(),
        "DefaultInfo",
    )
    .unwrap();
    module.set("DefaultInfo", default_info.to_value());
    let mut evaluator = Evaluator::new(&module);
    evaluator.set_loader(&loader);
    evaluator
        .eval_module(
            AstModule::parse(FILENAME, source.to_owned(), &Dialect::Bazel)
                .map_err(starlark::Error::into_anyhow)?,
            &globals(),
        )
        .map_err(starlark::Error::into_anyhow)?;
    drop(evaluator);
    Ok(module.freeze()?)
}
fn key() -> ConfiguredTargetKey {
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap();
    ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//pkg:build").unwrap(),
        crate::ConfigurationKey::from_slug(SlugConfiguration::default_target(&host).unwrap()),
    )
}
fn artifact(label: &str) -> AnalysisArtifact {
    AnalysisArtifact::Source(CanonicalLabel::parse(label).unwrap())
}
fn derived(
    label: &str,
    configuration: &[u8],
    path: &str,
    kind: ActionOutputKind,
) -> AnalysisArtifact {
    AnalysisArtifact::Derived {
        owner: AnalysisConfiguredTargetKey::new(
            CanonicalLabel::parse(label).unwrap(),
            configuration,
        ),
        output: ActionOutput::new(path, kind),
    }
}
fn rows() -> Vec<AnalysisArtifact> {
    vec![
        artifact("@@//pkg:main"),
        artifact("@@dep+//pkg:external"),
        derived(
            "@@//pkg:owner",
            b"main",
            "pkg/generated",
            ActionOutputKind::File,
        ),
        derived(
            "@@dep+//pkg:owner",
            b"external",
            "pkg/external-generated",
            ActionOutputKind::File,
        ),
        derived("@@//pkg:fake", b"one", "pkg/fake", ActionOutputKind::File),
        derived("@@//pkg:fake", b"two", "pkg/fake", ActionOutputKind::File),
    ]
}
fn target(
    heap: &starlark::values::FrozenHeap,
    owner: &AnalysisConfiguredTargetKey,
    files: AnalysisDepset,
) -> FrozenValue {
    let runfiles = RetainedRunfiles::from_parts(
        Vec::new(),
        vec![files.clone()],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        RunfilesConflictPolicy::Warn,
    )
    .unwrap();
    let info =
        DefaultInfo::from_effective(files, runfiles, RetainedRunfiles::empty(), None).unwrap();
    let providers =
        ProviderCollection::from_values(vec![ProviderValue::DefaultInfo(info)], false).unwrap();
    AnalysisValueMaterializer::new(heap)
        .value(&AnalysisValue::configured_target(
            ConfiguredTargetValue::new(owner.clone(), providers),
        ))
        .unwrap()
}
#[derive(Clone, Copy)]
enum Provenance {
    Valid,
    Missing,
    Duplicate,
    Relabeled,
    WrongDigest,
    BadFake,
    BadWorkspace,
}

fn proof(
    source: &str,
    provenance: Provenance,
    fake_index: usize,
    directory: bool,
    workspace: Option<&str>,
    gc_pressure: bool,
) -> anyhow::Result<ConfiguredNodeResult> {
    let cargo = cargo_module(source)?;
    let helper = cargo.get_assigned("_create_runfiles_dir")?.0;
    let key = key();
    let owner = key.artifact_owner();
    let sink: Arc<dyn AnalysisActionSink> = Arc::new(Declarations {
        owner: owner.clone(),
        actions: Mutex::new(CtxActions::new()),
    });
    let identity = BzlModuleIdentity {
        label: CanonicalLabel::parse(if matches!(provenance, Provenance::Relabeled) {
            "@@spoof+//cargo/private:cargo_build_script.bzl"
        } else {
            LABEL
        })
        .unwrap(),
        workspace_path: FILENAME.into(),
        repository_mapping: Arc::from([]),
    };
    let mut digest = [0; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&SHA[index * 2..index * 2 + 2], 16).unwrap();
    }
    if matches!(provenance, Provenance::WrongDigest) {
        digest[0] ^= 1;
    }
    let row = (
        CompactString::new(FILENAME),
        BzlModuleSourceProvenance::new(identity, digest),
    );
    let identities = match provenance {
        Provenance::Missing => vec![],
        Provenance::Duplicate => vec![row.clone(), row],
        _ => vec![row],
    };
    let analysis = AnalysisEvaluationContext::new(
        Arc::from([]),
        [],
        key.label().clone(),
        sink.clone(),
        FrozenValue::new_none(),
        identities.into(),
    );
    let module = Module::new();
    let mut inputs = rows();
    if directory {
        inputs.push(derived(
            "@@//pkg:tree",
            b"main",
            "pkg/tree",
            ActionOutputKind::Directory,
        ));
    }
    let fake = inputs[fake_index].clone();
    let shared = AnalysisDepset::new(
        DepsetOrder::Default,
        inputs
            .iter()
            .cloned()
            .map(AnalysisValue::artifact)
            .collect(),
        Vec::new(),
    )?;
    let script = target(module.frozen_heap(), &owner, shared.clone());
    let data = target(module.frozen_heap(), &owner, shared.clone());
    let fake = if matches!(provenance, Provenance::BadFake) {
        module.frozen_heap().alloc("not a File")
    } else {
        module.frozen_heap().alloc(AnalysisArtifactValue::new(fake))
    };
    let context = module.heap().alloc(AnalysisContextGen {
        action_sink: sink,
        token: analysis.root_token(),
        retained_owner: owner,
        target_label: key.label().clone(),
        package_path: "pkg".into(),
        dependencies: Arc::from([]),
        executables: Arc::from([AnalysisExecutable {
            attribute: "data_runfiles".into(),
            executable: Some(fake),
        }]),
        resolved_attributes: Arc::from([]),
        predeclared_outputs: Arc::from([]),
        build_setting_value: None,
        fragments: Value::new_none(),
        toolchain: None,
        exec_groups: None,
    });
    // Keep the original module heap alive with the callable. No source export or
    // edited copy participates in the callsite identity check.
    module.set("helper", module.frozen_heap().alloc(helper).to_value());
    evaluate(
        &module,
        "trampoline.bzl",
        if matches!(provenance, Provenance::BadWorkspace) {
            "def invoke(ctx, script, data):\n    wrapped = struct(label=ctx.label, actions=ctx.actions, executable=ctx.executable, workspace_name=42)\n    return helper(wrapped, script, data, ['keep'])\n"
        } else {
            "def invoke(ctx, script, data):\n    return helper(ctx, script, data, ['keep'])\n"
        },
        &globals(),
    )?;
    let mut evaluator = Evaluator::new(&module);
    evaluator.extra = Some(&analysis);
    let result = evaluator
        .eval_function(
            module.get("invoke").unwrap(),
            &[context, script.to_value(), data.to_value()],
            &[],
        )
        .map_err(starlark::Error::into_anyhow)?;
    module.set("saved", result);
    // The helper frame and its captured locals have exited. Root Args through
    // the module while allocating discardable lists. The final module statement
    // reaches the evaluator GC safe point with a valid frame.
    // fake_exe is frozen, so this proves Args/caller lifetime; Loading separately
    // tests tracing a mutable fake_exe wrapper rooted only by Args.
    if gc_pressure {
        evaluator.eval_module(AstModule::parse("allocation_pressure.bzl", "def churn():\n    for i in range(1000):\n        temporary = [str(i)] * 256\nchurn()\ncollected = True\n".to_owned(), &Dialect::Bazel).map_err(starlark::Error::into_anyhow)?, &globals()).map_err(starlark::Error::into_anyhow)?;
        assert!(
            module.heap().allocated_bytes() < module.heap().peak_allocated_bytes(),
            "allocation pressure must actually collect before snapshot lowering"
        );
    }
    let saved = TupleRef::from_value(module.get("saved").unwrap()).unwrap();
    let values = saved.content();
    let mut snapshot = StarlarkArgs::snapshot(values[2]).unwrap();
    // Exercise the retained captured scalar independently: production ctx has
    // a fixed _main workspace name, not a configurable workspace option.
    if let Some(workspace) = workspace {
        for call in &mut snapshot.calls {
            if let EvaluatorArgCallGen::AddAll(vector) = call {
                if let Some(EvaluatorVectorMapEachGen::CargoRunfiles { workspace_name, .. }) =
                    &mut vector.map_each
                {
                    *workspace_name = workspace.into();
                }
            }
        }
    }
    let mut lowerer = AnalysisValueLowerer::default();
    let input_value = lowerer
        .lower(values[1], "Cargo helper inputs")
        .map_err(anyhow::Error::msg)?;
    let AnalysisValueKind::Depset(input_depset) = input_value.kind() else {
        unreachable!()
    };
    assert_eq!(
        input_depset.to_list().len(),
        inputs.len(),
        "argv filtering cannot remove action input artifacts"
    );
    let (recipe, policy) = lower_args_snapshot(snapshot, &mut lowerer)?;
    let output = AnalysisArtifactValue::from_starlark(values[0])
        .unwrap()
        .artifact()
        .clone();
    let AnalysisArtifact::Derived { output, .. } = output else {
        unreachable!()
    };
    let spawn = SpawnSpec::new(
        RetainedSpawnInvocation::Executable(SpawnExecutable::Artifact(artifact("@@//:runner"))),
        RetainedCommandLine::new(vec![RetainedCommandLineSegment::ArgsSnapshot(
            RetainedSpawnArgsSnapshot::new(recipe, policy),
        )]),
        ArtifactInputs::new(vec![ArtifactInputSource::Depset(
            RetainedArtifactInputs::new(input_depset.clone())?,
        )]),
        ArtifactInputs::new(Vec::new()),
        vec![output],
        None,
        Default::default(),
        Default::default(),
        "CargoBuildScriptRun",
        None::<String>,
    );
    drop(evaluator);
    let context = Arc::new(ConfiguredActionOwnerContext::unresolved_default(key.clone()).unwrap());
    ConfiguredNodeResult::new_rule(
        key,
        ProviderCollection::from_values(Vec::new(), false).unwrap(),
        None,
        RunfilesPackageDepset::empty(),
    )
    .with_action_specs(vec![ActionSpec::spawn(spawn)], vec![context])
    .map_err(anyhow::Error::msg)
}

#[test]
fn unchanged_cargo_helper_retains_captures_after_caller_exit_and_gc() {
    let result = proof(SOURCE, Provenance::Valid, 4, false, None, true).unwrap();
    let expanded = result.actions()[0]
        .spawn_spec()
        .unwrap()
        .expand_forced_param_files()
        .unwrap();
    assert_eq!(
        expanded.argv(),
        [
            "runner",
            "--cargo_manifest_args=@pkg/build.cargo_runfiles-0.params"
        ]
    );
    assert_eq!(expanded.param_files().len(), 1);
    // The unchanged helper leaves the Args format at SHELL_QUOTED. Bazel
    // ShellEscaper excludes = from its safe alphabet, so mappings are quoted.
    assert_eq!(expanded.param_files()[0].bytes(), b"pkg/build.cargo_runfiles\nkeep\n'pkg/main=_main/pkg/main'\n'external/dep+/pkg/external=dep+/pkg/external'\n'pkg/generated=_main/pkg/generated'\n'pkg/external-generated=_main/pkg/external-generated'\n'pkg/fake=_main/pkg/fake'\n");
    // fake/config-one filtered, same rendered path/config-two retained.
    assert_eq!(
        result.actions()[0]
            .spawn_spec()
            .unwrap()
            .inputs()
            .sources()
            .len(),
        1
    );
}

#[test]
fn cargo_helper_capture_mutations_restore_configured_publication() {
    let a = proof(SOURCE, Provenance::Valid, 4, false, None, false).unwrap();
    let fake_b = proof(SOURCE, Provenance::Valid, 0, false, None, false).unwrap();
    let workspace_b = proof(SOURCE, Provenance::Valid, 4, false, Some("other"), false).unwrap();
    let restored = proof(SOURCE, Provenance::Valid, 4, false, None, false).unwrap();
    assert_ne!(a, fake_b);
    assert_ne!(a, workspace_b);
    assert_eq!(a, restored);
    let render = |value: &ConfiguredNodeResult| {
        value.actions()[0]
            .spawn_spec()
            .unwrap()
            .expand_forced_param_files()
            .unwrap()
            .param_files()[0]
            .bytes()
            .to_vec()
    };
    assert_ne!(render(&a), render(&fake_b));
    assert_ne!(render(&a), render(&workspace_b));
    assert_eq!(render(&a), render(&restored));
}

#[test]
fn cargo_helper_rejects_unauthenticated_source_and_directory_inputs() {
    for provenance in [
        Provenance::Missing,
        Provenance::Duplicate,
        Provenance::Relabeled,
        Provenance::WrongDigest,
    ] {
        assert!(proof(SOURCE, provenance, 4, false, None, false).is_err());
    }
    for (provenance, expected) in [
        (Provenance::BadFake, "fake_exe must be a File"),
        (Provenance::BadWorkspace, "workspace_name must be a string"),
    ] {
        let error = proof(SOURCE, provenance, 4, false, None, false)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{error}");
    }
    let changed = format!("{SOURCE}\n# source mutation\n");
    assert!(proof(&changed, Provenance::Valid, 4, false, None, false).is_err());
    let directory = proof(SOURCE, Provenance::Valid, 4, true, None, false)
        .unwrap_err()
        .to_string();
    assert!(
        directory.contains("regular") || directory.contains("Directory"),
        "{directory}"
    );
    assert!(proof(SOURCE, Provenance::Valid, 4, false, None, false).is_ok());
}
