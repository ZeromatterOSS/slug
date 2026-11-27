use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::values::Heap;
use crate::values::Value;

impl<'v> Value<'v> {
    #[inline]
    pub(crate) fn equals_in_heap(self, other: Value<'v>, heap: Heap<'v>) -> crate::Result<bool> {
        match heap.bounded_operation_budget() {
            Some(budget) => self.equals_bounded(other, budget),
            None => self.equals(other),
        }
    }

    pub(crate) fn equals_bounded(
        self,
        other: Value<'v>,
        budget: &ExecutionBudget,
    ) -> crate::Result<bool> {
        budget.charge(1, 0)?;
        if self.ptr_eq(other) {
            return Ok(true);
        }
        if let Some(left) = self.unpack_str() {
            let Some(right) = other.unpack_str() else {
                return Ok(false);
            };
            if left.len() != right.len() {
                return Ok(false);
            }
            budget.charge_text_work(left.len())?;
            return Ok(left == right);
        }
        if let Some(left) = self.unpack_i32() {
            if let Some(right) = other.unpack_i32() {
                return Ok(left == right);
            }
            if other.is_none() || other.unpack_bool().is_some() || other.is_str() {
                return Ok(false);
            }
            return Err(unsupported(budget, "equality", other));
        }
        // None and bool have only pointer equality in ordinary evaluation too.
        if self.is_none() || self.unpack_bool().is_some() {
            return Ok(false);
        }
        if other.is_none()
            && let Some(equal) = self.get_ref().equals_none_bounded()
        {
            return Ok(equal);
        }
        Err(unsupported(budget, "equality", self))
    }

    #[inline]
    pub(crate) fn truth_in_heap(self, heap: Heap<'v>) -> crate::Result<bool> {
        match heap.bounded_operation_budget() {
            Some(budget) => self.truth_bounded(budget),
            None => Ok(self.to_bool()),
        }
    }

    fn truth_bounded(self, budget: &ExecutionBudget) -> crate::Result<bool> {
        budget.charge(1, 0)?;
        if let Some(value) = self.unpack_bool() {
            return Ok(value);
        }
        if self.is_none() {
            return Ok(false);
        }
        if let Some(value) = self.unpack_i32() {
            return Ok(value != 0);
        }
        if let Some(value) = self.unpack_str() {
            return Ok(!value.is_empty());
        }
        Err(unsupported(budget, "truth testing", self))
    }
}

fn unsupported(
    budget: &ExecutionBudget,
    operation: &'static str,
    value: Value<'_>,
) -> crate::Error {
    budget
        .refuse(ExecutionLimit::UnsupportedOperation {
            operation,
            value_type: value.get_type(),
        })
        .into()
}
