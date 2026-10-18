//! Typed legacy-array conservation and actual unchanged publication cap checks.
use std::mem::align_of;
use std::mem::size_of;

use slug_build_api_v2::published_value::BYTE_LIMIT;
use slug_build_api_v2::published_value::NODE_LIMIT;
use slug_build_api_v2::published_value::PublishedValueArenaBuilder;
use slug_build_api_v2::*;

use super::super::*;
use super::*;

#[allow(dead_code)]
struct LegacyPendingSpawn {
    header: ActionSpec,
    command_line: PendingCommandLine,
    inputs: Option<inputs::PendingSpawnInputs>,
}
#[allow(dead_code)]
enum LegacyPending {
    Ready(ActionSpec),
    Spawn(LegacyPendingSpawn),
    ArgsWrite(PendingArgsWrite),
}
#[allow(dead_code)]
struct LegacyPreparedSpawn<R> {
    header: ActionSpec,
    command_line: PreparedCommandLine<R>,
    inputs: Option<(ArtifactInputs, ArtifactInputs)>,
}
#[allow(dead_code)]
enum LegacyPrepared<R> {
    Ready(ActionSpec),
    Spawn(LegacyPreparedSpawn<R>),
    ArgsWrite { header: ActionSpec, recipe: R },
}
#[test]
fn boxed_private_spawns_preserve_legacy_arrays_and_have_full_positive_layout() {
    // Requires the separate actual public SpawnSpec/ActionPayload/ActionSpec
    // old-header proof, not merely the current header type on both sides here.
    type ActualRecipe = crate::starlark_rule::args_callbacks::DraftRecipe;
    assert!(size_of::<PendingRegisteredAction>() <= size_of::<LegacyPending>());
    assert!(align_of::<PendingRegisteredAction>() <= align_of::<LegacyPending>());
    assert!(size_of::<PreparedAction<ActualRecipe>>() <= size_of::<LegacyPrepared<ActualRecipe>>());
    assert!(
        align_of::<PreparedAction<ActualRecipe>>() <= align_of::<LegacyPrepared<ActualRecipe>>()
    );
    for (body, pointer, alignment, actual) in [
        (
            size_of::<PendingSpawn>(),
            size_of::<Box<PendingSpawn>>(),
            align_of::<PendingSpawn>(),
            false,
        ),
        (
            size_of::<PreparedSpawn<ActualRecipe>>(),
            size_of::<Box<PreparedSpawn<ActualRecipe>>>(),
            align_of::<PreparedSpawn<ActualRecipe>>(),
            true,
        ),
    ] {
        let bytes = body + pointer + alignment + 8 * size_of::<usize>();
        let facts = bytes / size_of::<usize>() + usize::from(bytes % size_of::<usize>() != 0);
        let mut tape = Vec::new();
        let mut charge = |r, b| {
            tape.push((r, b));
            Ok(())
        };
        if actual {
            prepared::<ActualRecipe>(&mut charge).unwrap()
        } else {
            admit::<PendingSpawn>(&mut charge).unwrap()
        };
        assert_eq!(tape, [(CONTROL_WORK, CONTROL_BYTES), (facts, bytes)]);
    }
}
fn charge(graph: &mut PublishedValueArenaBuilder, r: usize, b: usize) -> Result<(), String> {
    graph
        .charge_storage(DepsetStorageCharge {
            nodes: 0,
            edges: 0,
            bytes: 0,
            visits: r,
        })
        .map_err(|e| e.to_string())?;
    graph.charge_bytes(b).map_err(|e| e.to_string())
}
#[test]
fn full_box_work_and_bytes_exact_or_plus_one_refuse_before_allocation_and_poison() {
    type Payload = PreparedSpawn<crate::starlark_rule::args_callbacks::DraftRecipe>;
    let bytes = size_of::<Payload>()
        + size_of::<Box<Payload>>()
        + align_of::<Payload>()
        + 8 * size_of::<usize>();
    let facts = bytes / size_of::<usize>() + usize::from(bytes % size_of::<usize>() != 0);
    for (work_seed, bytes_seed, ok) in [
        (
            NODE_LIMIT - CONTROL_WORK - facts,
            BYTE_LIMIT - CONTROL_BYTES - bytes,
            true,
        ),
        (NODE_LIMIT - CONTROL_WORK - facts + 1, 0, false),
        (0, BYTE_LIMIT - CONTROL_BYTES - bytes + 1, false),
    ] {
        let mut graph = PublishedValueArenaBuilder::new();
        charge(&mut graph, work_seed, bytes_seed).unwrap();
        let guard = std::sync::Arc::new(());
        let result = prepared::<crate::starlark_rule::args_callbacks::DraftRecipe>(&mut |r, b| {
            charge(&mut graph, r, b)
        });
        assert_eq!(result.is_ok(), ok);
        if ok {
            let copy = guard.clone();
            assert_eq!(std::sync::Arc::strong_count(&guard), 2);
            drop(copy)
        } else {
            assert_eq!(std::sync::Arc::strong_count(&guard), 1);
            assert!(graph.charge_visit().is_err());
        }
    }
}
fn header() -> ActionSpec {
    use slug_configuration_v2::CanonicalStringMap;
    use slug_configuration_v2::HostPathFlavor;
    use slug_configuration_v2::NormalizedBazelPath;
    use slug_configuration_v2::RetainedActionEnvironment;
    ActionSpec::spawn(SpawnSpec::new(
        RetainedSpawnInvocation::Executable(SpawnExecutable::Path(
            NormalizedBazelPath::new(HostPathFlavor::Unix, "tool").unwrap(),
        )),
        RetainedCommandLine::new(Vec::new()),
        ArtifactInputs::new(Vec::new()),
        ArtifactInputs::new(Vec::new()),
        Vec::<ActionOutput>::new(),
        None,
        RetainedActionEnvironment::default().for_action(false, std::iter::empty::<(&str, &str)>()),
        CanonicalStringMap::from_pairs(std::iter::empty::<(&str, &str)>()),
        "Generic",
        None::<&str>,
    ))
}
#[test]
fn pending_none_spawn_uses_actual_module_ledger_and_refuses_before_box_creation() {
    let bytes = size_of::<PendingSpawn>()
        + size_of::<Box<PendingSpawn>>()
        + align_of::<PendingSpawn>()
        + 8 * size_of::<usize>();
    let facts = bytes / size_of::<usize>() + usize::from(bytes % size_of::<usize>() != 0);
    // Initial actual collector:2+16; two added debits each pay root-entry2,
    // then control4 and the independently reconstructed full-body fact count.
    for (seed, ok) in [
        (NODE_LIMIT - 18 - 8 - facts, true),
        (NODE_LIMIT - 18 - 8 - facts + 1, false),
    ] {
        let module = Module::new();
        slug_loading_v2::resource_set::prepay_action_storage(&module, seed, 0).unwrap();
        let result =
            PendingRegisteredAction::spawn(&module, header(), PendingCommandLine::default());
        assert_eq!(result.is_ok(), ok);
        if let Ok(PendingRegisteredAction::Spawn(payload)) = result {
            assert!(payload.resource_set.is_none())
        }
        if !ok {
            assert!(slug_loading_v2::resource_set::prepay_action_storage(&module, 0, 0).is_err());
        }
    }
}
