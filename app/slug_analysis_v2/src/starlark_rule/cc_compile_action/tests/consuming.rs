//! Same production ordered merge, not a surrogate native binder/queue proof.
//! The separately required registration integration tests exercise real Cc loading.
use std::sync::Arc;

use slug_build_api_v2::*;
use slug_configuration_v2::CanonicalStringMap;
use slug_configuration_v2::HostPathFlavor;
use slug_configuration_v2::NormalizedBazelPath;
use slug_configuration_v2::RetainedActionEnvironment;
use slug_loading_v2::subrule_invocation::EvaluatorArgsSnapshot;
use slug_loading_v2::subrule_invocation::register_args_snapshot;
use starlark::environment::Module;

use super::super::merge_pending_actions;
use crate::starlark_rule::args_callbacks::pending::PendingCommandLine;
use crate::starlark_rule::args_callbacks::pending::PendingRegisteredAction;

#[path = "../../../../../slug_build_api_v2/tests/cpp_compile_support/mod.rs"]
mod fixture;

// Deliberately non-Clone. The real phase inside is non-Clone as well. Witnesses
// observe ownership only and never replace an action or publication capability.
#[derive(Debug)]
struct MovedAction {
    phase: PendingRegisteredAction,
    witness: Option<Arc<()>>,
}
impl From<ActionSpec> for MovedAction {
    fn from(action: ActionSpec) -> Self {
        Self {
            phase: PendingRegisteredAction::from(action),
            witness: None,
        }
    }
}
impl RegisterableAction for MovedAction {
    fn outputs(&self) -> &[ActionOutput] {
        self.phase.outputs()
    }
    fn mnemonic(&self) -> &str {
        self.phase.mnemonic()
    }
    fn validate(&self) -> Result<(), ActionError> {
        self.phase.validate()
    }
    fn as_action_spec(&self) -> Option<&ActionSpec> {
        self.phase.as_action_spec()
    }
}
fn root() -> DerivedArtifactRoot {
    crate::analysis_value::test_artifact_root()
}
fn output(name: &str) -> ActionOutput {
    ActionOutput::new(root(), name, ActionOutputKind::File)
}
fn ordinary(module: &Module, guard: &Arc<()>) -> CtxActions<MovedAction> {
    let mut actions = CtxActions::new_typed(root());
    let ready = ActionSpec::new(
        ActionKind::Write {
            content: "ready value".to_owned(),
            is_executable: false,
        },
        "FileWrite",
        vec![output("ready")],
    );
    let header = ActionSpec::spawn(SpawnSpec::new(
        RetainedSpawnInvocation::Executable(SpawnExecutable::Path(
            NormalizedBazelPath::new(HostPathFlavor::Unix, "tools/cc-consumer").unwrap(),
        )),
        RetainedCommandLine::new(Vec::new()),
        ArtifactInputs::new(Vec::new()),
        ArtifactInputs::new(Vec::new()),
        vec![output("spawn")],
        None,
        RetainedActionEnvironment::default(),
        CanonicalStringMap::default(),
        "UnrelatedPendingSpawn",
        Some("original progress"),
    ))
    .with_exec_group("ordinary");
    let spawn =
        PendingRegisteredAction::spawn(module, header, PendingCommandLine::default()).unwrap();
    let snapshot = register_args_snapshot(
        module,
        EvaluatorArgsSnapshot {
            calls: Vec::new(),
            format: RetainedParamFileFormat::Multiline,
            param_file: None,
        },
    )
    .unwrap();
    let args = PendingRegisteredAction::args_write(output("args"), snapshot, false);
    for phase in [PendingRegisteredAction::from(ready), spawn, args] {
        actions
            .register_pending(MovedAction {
                phase,
                witness: Some(guard.clone()),
            })
            .unwrap();
    }
    actions
}
fn cpp(prefix: &str) -> ActionSpec {
    let base = fixture::action(root());
    let mut data = base.cpp_compile_spec().unwrap().data().clone();
    data.construction.outputs = vec![
        output(&format!("{prefix}.o")),
        output(&format!("{prefix}.d")),
    ]
    .into_boxed_slice();
    ActionSpec::cpp_compile(CppCompileSpec::new(data).unwrap())
        .with_exec_group("compiler")
        .with_exec_properties(std::collections::BTreeMap::from([(
            "cpu".to_owned(),
            "native".to_owned(),
        )]))
}
fn box_address(action: &MovedAction) -> usize {
    let PendingRegisteredAction::Spawn(payload) = &action.phase else {
        panic!("genuine boxed pending Spawn")
    };
    std::ptr::from_ref(payload.as_ref()) as usize
}

#[test]
fn consuming_cc_merge_moves_mixed_real_phase_rows_in_exact_order_and_preserves_guards() {
    let module = Module::new();
    let guard = Arc::new(());
    let weak = Arc::downgrade(&guard);
    let actions = ordinary(&module, &guard);
    let original_box = box_address(&actions.registry().actions()[1]);
    assert_eq!(Arc::strong_count(&guard), 4);
    drop(guard);
    let first = cpp("first");
    let second = cpp("second");
    let pending = vec![(1, first.clone()), (2, second.clone())].into_iter();
    let mut calls = Vec::new();
    let result = merge_pending_actions(actions, pending, |index, action| {
        calls.push(index);
        Ok(action)
    })
    .unwrap();
    assert_eq!(calls, [0, 1]);
    let rows = result.registry().actions();
    assert_eq!(rows.len(), 5);
    assert_eq!(
        rows.iter()
            .map(|row| row.outputs()[0].root_relative_path())
            .collect::<Vec<_>>(),
        ["ready", "first.o", "spawn", "second.o", "args"]
    );
    assert_eq!(rows[1].as_action_spec(), Some(&first));
    assert_eq!(rows[3].as_action_spec(), Some(&second));
    assert!(matches!(rows[0].phase, PendingRegisteredAction::Ready(_)));
    assert!(matches!(
        rows[4].phase,
        PendingRegisteredAction::ArgsWrite(_)
    ));
    assert_eq!(
        box_address(&rows[2]),
        original_box,
        "the actual Box allocation must move, not clone"
    );
    assert_eq!(weak.strong_count(), 3);
    for (name, index) in [
        ("ready", 0),
        ("first.o", 1),
        ("first.d", 1),
        ("spawn", 2),
        ("second.o", 3),
        ("second.d", 3),
        ("args", 4),
    ] {
        assert_eq!(
            result.registry().output_owner(output(name).path()),
            Some(index)
        );
    }
    assert!(
        rows.iter()
            .flat_map(|row| row.outputs())
            .all(|value| value.root() == &root())
    );
    assert!(rows[1].witness.is_none() && rows[3].witness.is_none());
    drop(result);
    assert!(weak.upgrade().is_none());
}

#[test]
fn consuming_cc_empty_queue_returns_exact_original_phase_owner_and_registration_index() {
    let module = Module::new();
    let guard = Arc::new(());
    let weak = Arc::downgrade(&guard);
    let actions = ordinary(&module, &guard);
    let original = actions.registry().actions().as_ptr();
    let original_box = box_address(&actions.registry().actions()[1]);
    drop(guard);
    let result = merge_pending_actions(
        actions,
        std::iter::empty::<(usize, ActionSpec)>(),
        |_, _| -> anyhow::Result<ActionSpec> {
            panic!("empty Cc queue must not build, clone or re-register any action")
        },
    )
    .unwrap();
    assert_eq!(result.registry().actions().as_ptr(), original);
    assert_eq!(box_address(&result.registry().actions()[1]), original_box);
    assert_eq!(
        result.registry().output_owner(output("spawn").path()),
        Some(1)
    );
    assert_eq!(weak.strong_count(), 3);
    drop(result);
    assert!(weak.upgrade().is_none());
}

#[test]
fn consuming_cc_invalid_boundaries_build_failure_and_final_conflict_drop_entire_owner() {
    for (boundaries, builds) in [(vec![2, 1], 1), (vec![4], 0)] {
        let module = Module::new();
        let guard = Arc::new(());
        let weak = Arc::downgrade(&guard);
        let actions = ordinary(&module, &guard);
        drop(guard);
        let mut calls = 0;
        let pending = boundaries
            .into_iter()
            .enumerate()
            .map(|(n, index)| (index, cpp(&format!("cc{n}"))));
        let error = merge_pending_actions(actions, pending, |_, action| {
            calls += 1;
            Ok(action)
        })
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("pending action order is invalid"),
            "{error}"
        );
        assert_eq!(
            calls, builds,
            "invalid boundary must refuse before its Cc construction"
        );
        assert!(
            weak.upgrade().is_none(),
            "merged prefix and unconsumed tail must both drop"
        );
    }
    let module = Module::new();
    let guard = Arc::new(());
    let weak = Arc::downgrade(&guard);
    let actions = ordinary(&module, &guard);
    drop(guard);
    let error = merge_pending_actions(
        actions,
        std::iter::once((1, cpp("lower"))),
        |_, _| -> anyhow::Result<ActionSpec> {
            anyhow::bail!("actual construction closure refused")
        },
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "actual construction closure refused");
    assert!(weak.upgrade().is_none());
    let module = Module::new();
    let guard = Arc::new(());
    let weak = Arc::downgrade(&guard);
    let actions = ordinary(&module, &guard);
    drop(guard);
    let mut data = cpp("conflict").cpp_compile_spec().unwrap().data().clone();
    data.construction.outputs[0] = output("ready"); // valid Cc spec, conflicting completed batch
    let conflict = ActionSpec::cpp_compile(CppCompileSpec::new(data).unwrap());
    let error = merge_pending_actions(actions, std::iter::once((0, conflict)), |_, action| {
        Ok(action)
    })
    .unwrap_err();
    assert!(
        error.to_string().contains("declared more than once"),
        "{error}"
    );
    assert!(
        weak.upgrade().is_none(),
        "no partially validated target owner may escape"
    );
}
