//! Native ownership moves preserve non-Clone rows, backing and drop lifetime.
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use super::*;
use crate::ActionKind;
use crate::ActionOutputKind;
use crate::CtxActions;
use crate::DerivedArtifactRoot;

#[path = "../../../tests/root_support/mod.rs"]
mod root_support;

struct Witness(Arc<AtomicUsize>);
impl Drop for Witness {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
// Intentionally no Clone implementation: consuming APIs must not require it.
struct MoveOnlyAction {
    spec: ActionSpec,
    witness: Option<Witness>,
}
impl From<ActionSpec> for MoveOnlyAction {
    fn from(spec: ActionSpec) -> Self {
        Self {
            spec,
            witness: None,
        }
    }
}
impl RegisterableAction for MoveOnlyAction {
    fn outputs(&self) -> &[ActionOutput] {
        self.spec.outputs()
    }
    fn mnemonic(&self) -> &str {
        self.spec.mnemonic()
    }
    fn validate(&self) -> Result<(), ActionError> {
        RegisterableAction::validate(&self.spec)
    }
}

#[test]
fn consuming_owners_move_non_clone_rows_backing_and_original_root_until_last_drop() {
    let root = root_support::root();
    let mut owner = CtxActions::<MoveOnlyAction>::new_typed(root.clone());
    let drops = Arc::new(AtomicUsize::new(0));
    let weak = Arc::downgrade(&drops);
    let mut paths = Vec::new();
    for relative in ["first", "second"] {
        let output = owner.declare_file(relative).unwrap();
        paths.push(output.path().to_owned());
        let spec = ActionSpec::new(
            ActionKind::Write {
                content: relative.to_owned(),
                is_executable: false,
            },
            relative,
            vec![output],
        );
        owner
            .register_pending(MoveOnlyAction {
                spec,
                witness: Some(Witness(drops.clone())),
            })
            .unwrap();
    }
    let backing = owner.registry().actions().as_ptr();
    let capacity = owner.registry().actions.capacity();
    for (index, path) in paths.iter().enumerate() {
        assert_eq!(owner.registry().output_owner(path), Some(index));
    }
    let (moved_root, registry) = owner.into_parts();
    assert!(std::ptr::eq(
        root.configuration(),
        moved_root.configuration()
    ));
    assert_eq!(registry.actions().as_ptr(), backing);
    for (index, path) in paths.iter().enumerate() {
        assert_eq!(registry.output_owner(path), Some(index));
    }
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    let rows = registry.into_actions();
    assert_eq!(rows.as_ptr(), backing);
    assert_eq!(rows.capacity(), capacity);
    assert_eq!(
        rows.iter()
            .map(RegisterableAction::mnemonic)
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    assert!(rows.iter().all(|row| row.witness.is_some()));
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(rows);
    assert_eq!(drops.load(Ordering::SeqCst), 2);
    drop(drops);
    assert!(weak.upgrade().is_none());

    let empty = CtxActions::<MoveOnlyAction>::new_typed(moved_root);
    let (empty_root, empty_registry) = empty.into_parts();
    assert!(std::ptr::eq(
        root.configuration(),
        empty_root.configuration()
    ));
    assert!(empty_registry.into_actions().is_empty());
    assert_eq!(
        ActionOutput::new(empty_root, "unchanged", ActionOutputKind::File).root(),
        &root
    );
}
