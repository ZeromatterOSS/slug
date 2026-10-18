//! Native registration coordinates survive the traced-prepare/frozen-finish boundary.
use std::collections::BTreeMap;

use slug_build_api_v2::*;
use slug_configuration_v2::CanonicalStringMap;
use slug_configuration_v2::HostPathFlavor;
use slug_configuration_v2::NormalizedBazelPath;
use slug_configuration_v2::RetainedActionEnvironment;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::subrule_invocation::EvaluatorArgCallGen;
use slug_loading_v2::subrule_invocation::EvaluatorVectorArgGen;
use slug_loading_v2::subrule_invocation::EvaluatorVectorSourceGen;
use starlark::environment::GlobalsBuilder;
use starlark::eval::Evaluator;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::list::AllocList;
use starlark::values::none::NoneType;

use super::*;

#[starlark::starlark_module]
fn gc_helpers(builder: &mut GlobalsBuilder) {
    fn collect(eval: &mut Evaluator<'_, '_, '_>) -> anyhow::Result<NoneType> {
        // The actual Starlark call supplies an initialized bytecode frame.
        // No native argument Value is held across this collection.
        unsafe {
            eval.garbage_collect()
                .map_err(|error| error.into_anyhow())?
        };
        Ok(NoneType)
    }
}

fn root() -> DerivedArtifactRoot {
    let inputs = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap();
    DerivedArtifactRoot::bin(SlugConfiguration::default_target(&inputs).unwrap()).unwrap()
}

fn snapshot(module: &Module) -> ArgsSnapshotRegistration {
    register_args_snapshot(
        module,
        EvaluatorArgsSnapshot {
            calls: vec![EvaluatorArgCallGen::AddAll(EvaluatorVectorArgGen {
                source: EvaluatorVectorSourceGen::Sequence(vec![
                    module.heap().alloc_str("retained").to_value(),
                ]),
                options: RetainedVectorOptions {
                    arg_name: None,
                    format_each: None,
                    before_each: None,
                    join_with: None,
                    format_joined: None,
                    omit_if_empty: true,
                    uniquify: false,
                    expand_directories: false,
                    terminate_with: None,
                },
                map_each: None,
                registration_location: None,
            })],
            format: RetainedParamFileFormat::Multiline,
            param_file: Some(("@%s".into(), true)),
        },
    )
    .unwrap()
}

#[derive(Clone)]
struct Draft {
    values: Vec<CompactString>,
    format: RetainedParamFileFormat,
    param_file: Option<(CompactString, bool)>,
}

fn draft(snapshot: EvaluatorArgsSnapshot<'_>) -> anyhow::Result<Draft> {
    let mut values = Vec::new();
    for call in snapshot.calls {
        let EvaluatorArgCallGen::AddAll(vector) = call else {
            anyhow::bail!("expected vector")
        };
        let EvaluatorVectorSourceGen::Sequence(sequence) = vector.source else {
            anyhow::bail!("expected sequence")
        };
        for value in sequence {
            values.push(
                value
                    .unpack_str()
                    .ok_or_else(|| anyhow::anyhow!("expected string"))?
                    .into(),
            );
        }
    }
    Ok(Draft {
        values,
        format: snapshot.format,
        param_file: snapshot.param_file,
    })
}

fn complete(
    draft: Draft,
) -> anyhow::Result<(RetainedArgsRecipe, Option<RetainedSpawnParamFilePolicy>)> {
    let recipe = RetainedArgsRecipe::new(
        draft
            .values
            .into_iter()
            .map(|value| {
                RetainedArgCall::Scalar(RetainedScalarArg::new(
                    None::<&str>,
                    RetainedScalarValue::String(value),
                    None::<&str>,
                ))
            })
            .collect::<Vec<_>>(),
        draft.format,
    );
    Ok((
        recipe,
        draft
            .param_file
            .map(|(format, always)| RetainedSpawnParamFilePolicy::new(format, always)),
    ))
}

fn spawn_header(root: &DerivedArtifactRoot) -> ActionSpec {
    let unused = AnalysisArtifact::Source(CanonicalLabel::parse("@@//:unused").unwrap());
    ActionSpec::spawn(SpawnSpec::new(
        RetainedSpawnInvocation::Executable(SpawnExecutable::Path(
            NormalizedBazelPath::new(HostPathFlavor::Unix, "tool").unwrap(),
        )),
        RetainedCommandLine::new(Vec::new()),
        ArtifactInputs::new(Vec::new()),
        ArtifactInputs::new(Vec::new()),
        vec![ActionOutput::new(
            root.clone(),
            "spawn",
            ActionOutputKind::File,
        )],
        Some(unused),
        RetainedActionEnvironment::default().for_action(false, [("FIXED", "value")]),
        CanonicalStringMap::from_pairs([("no-sandbox", "1")]),
        "Generic",
        Some("progress %{output}"),
    ))
    .with_exec_group("named")
    .with_exec_properties(BTreeMap::from([("cpu".to_owned(), "native".to_owned())]))
}

#[test]
fn pending_rows_prepare_gc_traced_snapshots_then_finish_without_module_and_keep_order() {
    let module = Module::new();
    let registration = snapshot(&module);
    let root = root();
    let header = spawn_header(&root);
    let line = PendingCommandLine {
        segments: vec![
            PendingCommandLineSegment::LiteralRun(Arc::from([CompactString::new("before")])),
            PendingCommandLineSegment::ArgsSnapshot(registration.clone()),
            PendingCommandLineSegment::LiteralRun(Arc::from([CompactString::new("after")])),
        ],
    };
    let mut actions = CtxActions::<PendingRegisteredAction>::new_typed(root.clone());
    actions
        .write(
            ActionOutput::new(root.clone(), "ordinary", ActionOutputKind::File),
            "unchanged",
            false,
        )
        .unwrap();
    actions
        .register_pending(PendingRegisteredAction::spawn(&module, header.clone(), line).unwrap())
        .unwrap();
    actions
        .register_pending(PendingRegisteredAction::args_write(
            ActionOutput::new(root.clone(), "write", ActionOutputKind::File),
            registration,
            true,
        ))
        .unwrap();
    assert!(actions.registry().actions()[0].as_action_spec().is_some());
    assert!(actions.registry().actions()[1].as_action_spec().is_none());
    // The Args collector is the only evaluator root for its dynamic string.
    // Unreachable allocation makes the collection assertion discriminating.
    module
        .heap()
        .alloc(AllocList(vec!["discarded".to_owned(); 8192]));
    // Top-level statement bytecode can collect before entering collect().
    // Measure the complete evaluation interval, including that safe checkpoint.
    let before = module.heap().allocated_bytes();
    Evaluator::new(&module)
        .eval_module(
            AstModule::parse("pending_gc.bzl", "collect()".to_owned(), &Dialect::Bazel).unwrap(),
            &GlobalsBuilder::standard().with(gc_helpers).build(),
        )
        .unwrap();
    let after = module.heap().allocated_bytes();
    assert!(
        after < before,
        "GC must reclaim unreachable heap allocations: before={before}, after={after}"
    );
    let mut storage_owner = super::super::Publisher::new(&module);
    let mut prepared = actions
        .registry()
        .actions()
        .iter()
        .map(|action| {
            action
                .prepare(&module, &mut draft, &mut |r, b| {
                    storage_owner
                        .resource_storage(r, b, false)
                        .map_err(|error| error.to_string())
                })
                .unwrap()
        })
        .collect::<Vec<_>>()
        .into_iter();
    drop(storage_owner);
    slug_loading_v2::subrule_invocation::clear_args_snapshots(&module).unwrap();
    drop(module.freeze().unwrap());
    let actions = actions
        .try_map::<ActionSpec, anyhow::Error>(|_| prepared.next().unwrap().complete(&mut complete))
        .unwrap();
    assert!(prepared.next().is_none());
    let rows = actions.registry().actions();
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows[0].kind(),
        &ActionKind::Write {
            content: "unchanged".to_owned(),
            is_executable: false
        }
    );
    let spawn = rows[1].spawn_spec().unwrap();
    assert_eq!(
        spawn.render_argv().unwrap(),
        ["tool", "before", "retained", "after"]
    );
    assert_eq!(rows[1].outputs(), header.outputs());
    assert_eq!(rows[1].exec_group(), header.exec_group());
    assert_eq!(rows[1].exec_properties(), header.exec_properties());
    assert!(spawn.inputs().sources().is_empty());
    assert!(spawn.tools().sources().is_empty());
    assert!(header.spawn_spec().unwrap().inputs().sources().is_empty());
    assert!(header.spawn_spec().unwrap().tools().sources().is_empty());
    assert_eq!(
        spawn.unused_inputs_list(),
        header.spawn_spec().unwrap().unused_inputs_list()
    );
    assert_eq!(
        spawn.environment(),
        header.spawn_spec().unwrap().environment()
    );
    assert_eq!(
        spawn.execution_requirements(),
        header.spawn_spec().unwrap().execution_requirements()
    );
    assert_eq!(
        spawn.progress_message(),
        header.spawn_spec().unwrap().progress_message()
    );
    let [_, RetainedCommandLineSegment::ArgsSnapshot(args), _] = spawn.command_line().segments()
    else {
        panic!("ordered segments")
    };
    assert!(args.param_file().unwrap().use_always());
    let write = rows[2].args_write_spec().unwrap();
    assert!(write.is_executable());
    assert_eq!(write.render_content().unwrap(), "retained\n");
}

#[test]
fn pending_headers_refuse_conflicts_and_snapshot_membership_before_publication() {
    let module = Module::new();
    let token = snapshot(&module);
    let output_root = root();
    let output = ActionOutput::new(output_root.clone(), "same", ActionOutputKind::File);
    let mut actions = CtxActions::<PendingRegisteredAction>::new_typed(output_root.clone());
    actions
        .write(output.clone(), "already registered", false)
        .unwrap();
    assert!(
        actions
            .register_pending(PendingRegisteredAction::args_write(
                output,
                token.clone(),
                false
            ))
            .is_err()
    );
    assert_eq!(actions.registry().actions().len(), 1);
    let pending = PendingRegisteredAction::args_write(
        ActionOutput::new(output_root, "other", ActionOutputKind::File),
        token,
        false,
    );
    let other = Module::new();
    assert!(
        pending
            .prepare(
                &other,
                &mut |_snapshot| -> anyhow::Result<Draft> {
                    panic!("membership check must precede draft publication")
                },
                &mut |_, _| panic!("ArgsWrite must not allocate Spawn storage")
            )
            .is_err()
    );
    slug_loading_v2::subrule_invocation::clear_args_snapshots(&module).unwrap();
    assert!(
        pending
            .prepare(
                &module,
                &mut |_snapshot| -> anyhow::Result<Draft> {
                    panic!("stale token must fail before draft publication")
                },
                &mut |_, _| panic!("ArgsWrite must not allocate Spawn storage")
            )
            .is_err()
    );
}

#[test]
fn pending_command_line_captures_literal_presence_and_rejects_invalid_entries() {
    let module = Module::new();
    let (empty, present) = PendingCommandLine::capture(&module, None).unwrap();
    assert!(!present && empty.segments.is_empty());
    let values = module.heap().alloc(AllocList(["one", "two"]));
    let (line, present) = PendingCommandLine::capture(&module, Some(values)).unwrap();
    assert!(present);
    let prepared = line
        .prepare(&module, &mut |_snapshot| -> anyhow::Result<Draft> {
            panic!("literals have no Args snapshots")
        })
        .unwrap();
    let retained = prepared.complete(&mut complete).unwrap();
    assert_eq!(retained.render().unwrap(), ["one", "two"]);
    assert!(PendingCommandLine::capture(&module, Some(Value::new_none())).is_err());
    assert!(
        PendingCommandLine::capture(
            &module,
            Some(module.heap().alloc(AllocList([module.heap().alloc(17)])))
        )
        .is_err()
    );
}

#[test]
fn pending_spawn_inputs_snapshot_membership_and_use_shared_end_lowerer() {
    use slug_loading_v2::provider::StarlarkDepsetSuccessorGen;
    use slug_loading_v2::subrule_invocation::AnalysisActionCallScope;
    use slug_loading_v2::subrule_invocation::AnalysisArtifactValue;
    use starlark::environment::Globals;

    let module = Module::new();
    let file = module
        .frozen_heap()
        .alloc(AnalysisArtifactValue::new(AnalysisArtifact::Source(
            CanonicalLabel::parse("@@//:input").unwrap(),
        )));
    // This evaluator value has no retained native graph. The common END lowerer
    // must pack it once when shared by a provider root and both input roles.
    let set = slug_loading_v2::provider::alloc_starlark_depset_parts(
        module.frozen_heap(),
        DepsetOrder::Default,
        Some("File".into()),
        AnalysisDepsetOccurrence::new(),
        None,
        1,
        vec![StarlarkDepsetSuccessorGen::Direct(file)],
    )
    .to_value();
    let tools = module.heap().alloc(AllocList([set]));
    module.set("tools", tools);
    let header = spawn_header(&root());
    let pending =
        PendingRegisteredAction::spawn(&module, header.clone(), PendingCommandLine::default())
            .unwrap()
            .with_inputs(
                &module,
                Some(set),
                Some(tools),
                AnalysisActionCallScope::Root,
            )
            .unwrap();
    Evaluator::new(&module)
        .eval_module(
            AstModule::parse("mutation.bzl", "tools.clear()".to_owned(), &Dialect::Bazel).unwrap(),
            &Globals::standard(),
        )
        .unwrap();
    let mut lowerer = AnalysisValueLowerer::default();
    let provider_root = lowerer.lower(set, "provider root").unwrap();
    let provenance = ExecutableArtifactProvenance::default();
    let pair = pending
        .lower_inputs(&module, &mut lowerer, &provenance)
        .unwrap()
        .unwrap();
    assert_eq!(pair.0.sources().len(), 1);
    assert_eq!(
        pair.1.sources().len(),
        1,
        "later outer-list mutation must not erase captured membership"
    );
    let AnalysisValueKind::Depset(retained) = provider_root.kind() else {
        panic!("provider depset")
    };
    let expected = ArtifactInputs::new(vec![ArtifactInputSource::Depset(
        RetainedArtifactInputs::new(retained.clone()).unwrap(),
    )]);
    for inputs in [&pair.0, &pair.1] {
        assert!(matches!(inputs.sources(), [ArtifactInputSource::Depset(_)]));
        let mut artifacts = Vec::new();
        inputs
            .visit(|artifact| artifacts.push(artifact.clone()))
            .unwrap();
        assert_eq!(
            artifacts,
            [AnalysisArtifact::Source(
                CanonicalLabel::parse("@@//:input").unwrap()
            )]
        );
    }
    let again = lowerer.lower(set, "same source identity").unwrap();
    let AnalysisValueKind::Depset(again) = again.kind() else {
        panic!("same depset")
    };
    assert!(retained.shares_store_with(again));
    assert!(
        pending
            .prepare(&module, &mut draft, &mut |_, _| panic!(
                "missing lowered inputs must refuse before prepared storage"
            ))
            .is_err(),
        "captured inputs must never fall back to independently lowered prototype fields"
    );
    // This legacy semantic fixture uses the real Publisher storage owner but
    // intentionally clears its empty-resource collector before original freeze.
    let mut publication = super::super::Publisher::new(&module);
    let prepared = pending
        .prepare_with_inputs(
            &module,
            Some(pair),
            &mut draft,
            &mut |_| anyhow::bail!("this fixture contains no resource token"),
            &mut |r, b| {
                publication
                    .resource_storage(r, b, false)
                    .map_err(|error| error.to_string())
            },
        )
        .unwrap();
    drop(publication);
    drop(lowerer);
    slug_loading_v2::subrule_invocation::clear_args_snapshots(&module).unwrap();
    drop(module.freeze().unwrap());
    let action = prepared.complete(&mut complete).unwrap();
    // These two expected-only values use a persistent real bounded oracle;
    // they do not stand in for production's same-Publisher preparation.
    let mut expected_storage =
        slug_build_api_v2::published_value::PublishedValueArenaBuilder::new();
    let mut admit_spawn = |r, b| {
        expected_storage
            .charge_storage(slug_build_api_v2::DepsetStorageCharge {
                nodes: 0,
                edges: 0,
                visits: r,
                bytes: b,
            })
            .map_err(|error| error.to_string())
    };
    resource_storage::prepared::<Draft>(&mut admit_spawn).unwrap();
    let expected_action = PreparedAction::<Draft>::Spawn(Box::new(PreparedSpawn {
        header: header.clone(),
        command_line: PreparedCommandLine {
            segments: Vec::new(),
        },
        inputs: Some((expected.clone(), expected)),
        resource_set: None,
    }))
    .complete(&mut complete)
    .unwrap();
    let providers = ProviderCollection::from_values(
        vec![ProviderValue::Occurrence(ProviderOccurrence::new(
            ProviderIdentity::user(ProviderId::new("//:unrelated.bzl", "Info").unwrap()),
            [("files", provider_root.clone())],
        ))],
        false,
    )
    .unwrap();
    assert!(
        publication_eq_providers_and_actions(&providers, &providers, [(&action, &expected_action)]),
        "both action input roles must share the native provider occurrence"
    );
    // An equal-content independent occurrence must fail that joint alias check.
    let independent = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::artifact(AnalysisArtifact::Source(
            CanonicalLabel::parse("@@//:input").unwrap(),
        ))],
        Vec::new(),
    )
    .unwrap();
    assert!(!retained.shares_occurrence_with(&independent));
    let split = ArtifactInputs::new(vec![ArtifactInputSource::Depset(
        RetainedArtifactInputs::new(independent).unwrap(),
    )]);
    resource_storage::prepared::<Draft>(&mut admit_spawn).unwrap();
    let split_action = PreparedAction::<Draft>::Spawn(Box::new(PreparedSpawn {
        header,
        command_line: PreparedCommandLine {
            segments: Vec::new(),
        },
        inputs: Some((split.clone(), split)),
        resource_set: None,
    }))
    .complete(&mut complete)
    .unwrap();
    assert!(
        !publication_eq_providers_and_actions(&providers, &providers, [(&action, &split_action)]),
        "content equality alone must not satisfy the provider/action alias assertion"
    );
}
