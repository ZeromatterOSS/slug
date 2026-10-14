use std::fmt;

use allocative::Allocative;
use starlark_derive::NoSerialize;
use starlark_derive::Trace;
use starlark_derive::starlark_value;

use super::*;
use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::environment::Module;
use crate::values::Freeze;
use crate::values::FreezeError;
use crate::values::StarlarkValue;

fn limited(heap: &FrozenHeap, work: usize, scratch: usize) -> Freezer<'_> {
    Freezer {
        heap,
        frozen_defs: RefCell::new(Vec::new()),
        traversal: FreezeTraversalState::new(work, scratch),
    }
}

struct Plan(usize);
impl Freeze for Plan {
    type Frozen = ();
    fn freeze(self, freezer: &Freezer) -> FreezeResult<()> {
        freezer.with_traversal(|| {
            freezer.charge_traversal_work(1)?;
            freezer.charge_traversal_scratch(8)?;
            if self.0 > 0 {
                Plan(self.0 - 1).freeze(freezer)?;
            }
            Ok(())
        })
    }
}

#[test]
fn nested_traversals_share_fixed_work_and_scratch_without_refunds() {
    let heap = FrozenHeap::new();
    let freezer = limited(&heap, 2, 24);
    assert!(Plan(2).freeze(&freezer).is_err());
    assert!(freezer.charge_traversal_work(0).is_err());
    let freezer = limited(&heap, 8, 16);
    Plan(1).freeze(&freezer).unwrap();
    assert!(Plan(0).freeze(&freezer).is_err());
    let freezer = limited(&heap, 8, 24);
    Plan(2).freeze(&freezer).unwrap();
}

#[test]
fn forecast_and_counter_errors_poison_before_frozen_shortcuts() {
    let heap = FrozenHeap::new();
    let freezer = limited(&heap, usize::MAX, 0);
    freezer.charge_traversal_work(usize::MAX).unwrap();
    assert!(freezer.charge_traversal_work(1).is_err());
    assert!(freezer.freeze(Value::new_none()).is_err());
    let freezer = limited(&heap, 1, 1);
    let error: FreezeResult<()> =
        freezer.with_traversal(|| Err(FreezeError::new("checked forecast overflow".into())));
    assert!(error.is_err());
    assert!(freezer.freeze(Value::new_none()).is_err());
    let freezer = limited(&heap, 1, 0);
    let mut scratch = Vec::<u8>::new();
    let error = freezer.with_traversal(|| {
        freezer.charge_traversal_scratch(4)?;
        scratch.reserve_exact(4);
        Ok(())
    });
    assert!(error.is_err());
    assert_eq!(scratch.capacity(), 0);
    let freezer = limited(&heap, 0, 0);
    assert!(
        freezer
            .with_traversal(|| {
                let _ = freezer.charge_traversal_work(1);
                Ok(())
            })
            .is_err()
    );
}

#[derive(Debug, Trace, ProvidesStaticType, NoSerialize, Allocative)]
struct Fails;
impl fmt::Display for Fails {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("fails")
    }
}
#[starlark_value(type = "freeze_failure")]
impl<'v> StarlarkValue<'v> for Fails {}
impl Freeze for Fails {
    type Frozen = Self;
    fn freeze(self, _freezer: &Freezer) -> FreezeResult<Self> {
        Err(FreezeError::new("original native freeze failure".into()))
    }
}

#[test]
fn failed_original_freeze_cannot_succeed_via_installed_forwarding() {
    let module = Module::new();
    let heap = module.heap();
    let frozen = FrozenHeap::new();
    let freezer = Freezer::new(&frozen);
    let value = heap.alloc_complex(Fails);
    assert!(!freezer.is_frozen_or_forwarded(value));
    assert_eq!(
        freezer.freeze(value).unwrap_err().err_msg,
        "original native freeze failure"
    );
    // The target reservation is unfilled. Only its header may be inspected.
    assert!(freezer.is_frozen_or_forwarded(value));
    assert!(freezer.freeze(value).is_err());
    assert!(freezer.freeze(Value::new_none()).is_err());
}
