//! One cooperative traversal budget for the lifetime of an actual freezer.
use std::cell::Cell;

use crate::values::FreezeError;
use crate::values::FreezeResult;

pub(super) struct FreezeTraversalState {
    work: Cell<usize>,
    scratch: Cell<usize>,
    failed: Cell<bool>,
    work_limit: usize,
    scratch_limit: usize,
}

impl Default for FreezeTraversalState {
    fn default() -> Self {
        Self::new(65536, 64 * 1024 * 1024)
    }
}

impl FreezeTraversalState {
    pub(super) fn new(work_limit: usize, scratch_limit: usize) -> Self {
        Self {
            work: Cell::new(0),
            scratch: Cell::new(0),
            failed: Cell::new(false),
            work_limit,
            scratch_limit,
        }
    }

    pub(super) fn check(&self) -> FreezeResult<()> {
        if self.failed.get() {
            Err(FreezeError::new(
                "freeze traversal previously failed".into(),
            ))
        } else {
            Ok(())
        }
    }

    pub(super) fn poison(&self) {
        self.failed.set(true);
    }

    fn charge(&self, used: &Cell<usize>, limit: usize, amount: usize) -> FreezeResult<()> {
        self.check()?;
        match used.get().checked_add(amount) {
            Some(next) if next <= limit => {
                used.set(next);
                Ok(())
            }
            _ => {
                self.poison();
                Err(FreezeError::new(
                    "freeze traversal resource limit exceeded".into(),
                ))
            }
        }
    }

    pub(super) fn charge_work(&self, amount: usize) -> FreezeResult<()> {
        self.charge(&self.work, self.work_limit, amount)
    }

    pub(super) fn charge_scratch(&self, amount: usize) -> FreezeResult<()> {
        self.charge(&self.scratch, self.scratch_limit, amount)
    }
}
