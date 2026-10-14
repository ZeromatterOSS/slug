//! Preallocated complex-object relocation tasks for one garbage collection.
use std::cell::Cell;
use std::cell::RefCell;

use super::arena::MIN_ALLOC;
use super::heap_type::Tracer;
use crate::values::Trace;

#[cfg(test)]
mod tests;

const QUEUE_BYTE_LIMIT: usize = 64 * 1024 * 1024;

struct Task<'v> {
    payload: *mut (),
    trace: unsafe fn(*mut (), &Tracer<'v>),
}

pub(super) struct DeferredTraceQueue<'v> {
    tasks: RefCell<Vec<Task<'v>>>,
    inserted: Cell<usize>,
    bound: usize,
}

impl<'v> DeferredTraceQueue<'v> {
    pub(super) fn new(old_bytes: usize) -> crate::Result<Self> {
        Self::with_limit(old_bytes, QUEUE_BYTE_LIMIT)
    }

    pub(super) fn with_limit(old_bytes: usize, byte_limit: usize) -> crate::Result<Self> {
        // Every old complex allocation is at least MIN_ALLOC, and forwarding
        // prevents a second insertion for that allocation. Unreachable/simple
        // objects and arena padding make this conservative.
        let bound = old_bytes / MIN_ALLOC.bytes() as usize;
        let bytes = bound
            .checked_mul(std::mem::size_of::<Task<'v>>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()));
        if !matches!(bytes, Some(n) if n <= byte_limit) {
            return Err(crate::Error::new_other(anyhow::anyhow!(
                "garbage collection queue storage limit exceeded before moving"
            )));
        }
        let mut tasks = Vec::new();
        tasks.try_reserve_exact(bound).map_err(|_| {
            crate::Error::new_other(anyhow::anyhow!(
                "garbage collection queue allocation failed before moving"
            ))
        })?;
        Ok(Self {
            tasks: RefCell::new(tasks),
            inserted: Cell::new(0),
            bound,
        })
    }

    /// The initialized target payload is exclusive, stable and valid until drain
    /// completes. It came from one old object of the collecting arena exactly once.
    pub(super) unsafe fn push<T: Trace<'v>>(&self, payload: *mut T) {
        unsafe fn trace<'v, T: Trace<'v>>(payload: *mut (), tracer: &Tracer<'v>) {
            unsafe { (&mut *payload.cast::<T>()).trace(tracer) };
        }
        let inserted = self.inserted.get();
        assert!(
            inserted < self.bound,
            "GC same-heap/once-per-field contract violated"
        );
        self.inserted.set(inserted + 1);
        let mut tasks = self.tasks.borrow_mut();
        assert!(
            tasks.len() < tasks.capacity(),
            "GC preallocated queue bound violated"
        );
        tasks.push(Task {
            payload: payload.cast(),
            trace: trace::<T>,
        });
    }

    pub(super) fn drain(&self, tracer: &Tracer<'v>) {
        loop {
            // Never hold the queue borrow while a callback enqueues descendants.
            let task = self.tasks.borrow_mut().pop();
            let Some(task) = task else { break };
            unsafe { (task.trace)(task.payload, tracer) };
        }
    }
}
