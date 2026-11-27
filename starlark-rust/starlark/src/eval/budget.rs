//! Request-owned credits for native operations that admit work and allocation.

use std::cmp::Ordering;
use std::sync::Mutex;

/// First refusal of an opt-in execution budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ExecutionLimit {
    /// The request exhausted its computational work allowance.
    #[error("Starlark execution work limit exceeded")]
    Work,
    /// The request exhausted its cumulative allocation allowance.
    #[error("Starlark execution allocation limit exceeded")]
    AllocationBytes,
    /// The requested buffer would exceed its text limit.
    #[error("Starlark text byte limit exceeded")]
    TextBytes,
    /// Rendering would exceed the admitted value nesting depth.
    #[error("Starlark text depth limit exceeded")]
    TextDepth,
    /// Structural identity would exceed the admitted value nesting depth.
    #[error("Starlark structural identity depth limit exceeded")]
    StructuralDepth,
    /// Structural identity encountered an active value cycle.
    #[error("Starlark structural identity encountered a value cycle")]
    StructuralCycle,
    /// A fallible allocation refused an admitted request.
    #[error("Starlark execution allocation failed")]
    AllocationFailed,
    /// No admitted renderer exists for this value type.
    #[error("bounded Starlark text is unsupported for type {0}")]
    UnsupportedText(&'static str),
    /// No bounded implementation is admitted for this operation and value.
    #[error("bounded Starlark {operation} is unsupported for type {value_type}")]
    UnsupportedOperation {
        /// The attempted operation.
        operation: &'static str,
        /// The runtime type that needs an admitted implementation.
        value_type: &'static str,
    },
    /// An admitted writer failed without a more specific refusal.
    #[error("bounded Starlark text formatting failed")]
    TextFormatting,
}

impl From<ExecutionLimit> for crate::Error {
    fn from(limit: ExecutionLimit) -> Self {
        crate::Error::new_other(limit)
    }
}

#[derive(Clone, Copy, Debug)]
struct Credits {
    work: usize,
    allocation_bytes: usize,
}

/// Shared monotone credits for one request, including its separate evaluator
/// heaps and terminal rendering. The first refusal is returned by every later
/// charge. Only cooperating operations are charged by this ledger.
#[derive(Debug)]
pub struct ExecutionBudget {
    state: Mutex<Result<Credits, ExecutionLimit>>,
    text_depth: usize,
}

impl ExecutionBudget {
    /// Set the total work, cumulative controlled allocation bytes, and maximum
    /// value nesting accepted by text rendering and structural identity.
    pub fn new(work: usize, allocation_bytes: usize, text_depth: usize) -> Self {
        Self {
            state: Mutex::new(Ok(Credits {
                work,
                allocation_bytes,
            })),
            text_depth,
        }
    }

    /// Admit work and the full requested allocation before either occurs.
    /// Dropping scratch or a heap does not refund credits.
    pub fn charge(&self, work: usize, allocation_bytes: usize) -> Result<(), ExecutionLimit> {
        let mut state = self.state.lock().unwrap();
        let current = (*state)?;
        *state = match (
            current.work.checked_sub(work),
            current.allocation_bytes.checked_sub(allocation_bytes),
        ) {
            (None, _) => Err(ExecutionLimit::Work),
            (_, None) => Err(ExecutionLimit::AllocationBytes),
            (Some(work), Some(allocation_bytes)) => Ok(Credits {
                work,
                allocation_bytes,
            }),
        };
        (*state).map(|_| ())
    }

    /// Admit a pass over text, with each work unit covering up to 256 bytes.
    pub fn charge_text_work(&self, bytes: usize) -> Result<(), ExecutionLimit> {
        self.charge(Self::text_work(bytes), 0)
    }

    pub(crate) fn text_work(bytes: usize) -> usize {
        bytes.div_ceil(256)
    }

    /// Admit a scratch vector's requested capacity and possible relocation.
    /// The caller pays for work that fills the admitted storage.
    pub fn reserve_vec<T>(
        &self,
        values: &mut Vec<T>,
        required_len: usize,
    ) -> Result<(), ExecutionLimit> {
        let capacity = values.capacity();
        if required_len <= capacity {
            return self.check();
        }
        let next = capacity
            .checked_mul(2)
            .map(|next| next.max(required_len))
            .ok_or_else(|| self.refuse(ExecutionLimit::AllocationBytes))?;
        let element_bytes = std::mem::size_of::<T>();
        let bytes = next
            .checked_mul(element_bytes)
            .ok_or_else(|| self.refuse(ExecutionLimit::AllocationBytes))?;
        self.charge(
            capacity.max(Self::text_work(capacity * element_bytes)),
            bytes,
        )?;
        values
            .try_reserve_exact(next - values.len())
            .map_err(|_| self.refuse(ExecutionLimit::AllocationFailed))
    }

    /// Admit and append one element to a scratch vector.
    pub fn push_vec<T>(&self, values: &mut Vec<T>, item: T) -> Result<(), ExecutionLimit> {
        self.charge(1, 0)?;
        let len = values
            .len()
            .checked_add(1)
            .ok_or_else(|| self.refuse(ExecutionLimit::AllocationBytes))?;
        self.reserve_vec(values, len)?;
        values.push(item);
        Ok(())
    }

    /// Heap-sort in place, charging each comparison and swap before it occurs.
    /// The comparison can charge any additional work required by its operands.
    pub fn sort_unstable_by<T>(
        &self,
        values: &mut [T],
        mut compare: impl FnMut(&T, &T) -> Result<Ordering, ExecutionLimit>,
    ) -> Result<(), ExecutionLimit> {
        self.check()?;
        let mut sift_down = |values: &mut [T], mut parent: usize| -> Result<(), ExecutionLimit> {
            while parent < values.len() / 2 {
                let left = 2 * parent + 1;
                let right = left + 1;
                let mut child = left;
                if right < values.len() {
                    self.charge(1, 0)?;
                    if compare(&values[right], &values[left])?.is_gt() {
                        child = right;
                    }
                }
                self.charge(1, 0)?;
                if !compare(&values[parent], &values[child])?.is_lt() {
                    break;
                }
                self.charge(1, 0)?;
                values.swap(parent, child);
                parent = child;
            }
            Ok(())
        };
        for parent in (0..values.len() / 2).rev() {
            sift_down(values, parent)?;
        }
        for end in (1..values.len()).rev() {
            self.charge(1, 0)?;
            values.swap(0, end);
            sift_down(&mut values[..end], 0)?;
        }
        Ok(())
    }

    /// Return the configured value nesting limit shared with text rendering.
    pub fn text_depth(&self) -> usize {
        self.text_depth
    }

    /// Observe a previous refusal without consuming any credits.
    pub fn check(&self) -> Result<(), ExecutionLimit> {
        (*self.state.lock().unwrap()).map(|_| ())
    }

    /// Latch a resource refusal and return the first reason on this request.
    pub fn refuse(&self, reason: ExecutionLimit) -> ExecutionLimit {
        let mut state = self.state.lock().unwrap();
        match *state {
            Err(existing) => existing,
            Ok(_) => {
                *state = Err(reason);
                reason
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admission_is_exact_monotone_and_sticky() {
        let budget = ExecutionBudget::new(2, 8, 16);
        assert_eq!(budget.charge(1, 3), Ok(()));
        assert_eq!(budget.charge(1, 5), Ok(()));
        assert_eq!(budget.charge(0, 1), Err(ExecutionLimit::AllocationBytes));
        assert_eq!(budget.charge(1, 0), Err(ExecutionLimit::AllocationBytes));
        assert_eq!(
            budget.refuse(ExecutionLimit::TextDepth),
            ExecutionLimit::AllocationBytes
        );
        assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));

        let fresh = ExecutionBudget::new(2, 8, 16);
        assert_eq!(fresh.charge_text_work(257), Ok(()));
        assert_eq!(fresh.charge_text_work(1), Err(ExecutionLimit::Work));
    }
}
