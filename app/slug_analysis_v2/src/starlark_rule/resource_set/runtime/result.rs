//! Bounded staged validation: all key types, then unknown keys, then values.
use std::cell::RefCell;
use std::fmt;
use std::fmt::Write;

use slug_build_api_v2::actions::ResourceEstimate;
use slug_build_api_v2::actions::ResourceSetError;
use slug_build_api_v2::actions::ResourceSetEvaluationContext;
use slug_build_api_v2::published_value::BYTE_LIMIT;
use slug_build_api_v2::published_value::NODE_LIMIT;
use starlark::values::StarlarkBigInt;
use starlark::values::UnpackValue;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::dict::DictRef;
use starlark::values::float::UnpackFloat;

#[derive(Default)]
pub(super) struct ResultBudget {
    work: usize,
    bytes: usize,
    failed: bool,
}
impl ResultBudget {
    pub(super) fn pay(&mut self, work: usize, bytes: usize) -> Result<(), ResourceSetError> {
        if self.failed {
            return Err(ResourceSetError::Limit("resource result poisoned".into()));
        }
        let next = self
            .work
            .checked_add(work)
            .zip(self.bytes.checked_add(bytes));
        match next {
            Some((w, b)) if w <= NODE_LIMIT && b <= BYTE_LIMIT => {
                self.work = w;
                self.bytes = b;
                Ok(())
            }
            _ => {
                self.failed = true;
                Err(ResourceSetError::Limit("resource result work/bytes".into()))
            }
        }
    }
}
fn numeric(
    value: Option<Value<'_>>,
    default: f64,
    budget: &RefCell<ResultBudget>,
    check: &impl Fn() -> Result<(), ResourceSetError>,
) -> Result<f64, ResourceSetError> {
    check()?;
    budget.borrow_mut().pay(1, 0)?;
    let Some(value) = value else {
        return Ok(default);
    };
    if let Some(big) = value.downcast_ref::<StarlarkBigInt>() {
        let bytes = big
            .magnitude_allocation_upper_bound()
            .ok_or_else(|| ResourceSetError::Limit("resource integer size".into()))?;
        budget.borrow_mut().pay(bytes / 8 + 1, bytes)?;
    }
    UnpackFloat::unpack_value(value)
        .map_err(|_| ResourceSetError::InvalidReturn("expected int or float".into()))?
        .map(|v| v.0)
        .ok_or_else(|| ResourceSetError::InvalidReturn("expected int or float".into()))
}
pub(super) fn parse(
    value: Value<'_>,
    budget: &RefCell<ResultBudget>,
    check: &impl Fn() -> Result<(), ResourceSetError>,
) -> Result<ResourceEstimate, ResourceSetError> {
    budget.borrow_mut().pay(1, 0)?;
    let dictionary = DictRef::from_value(value)
        .ok_or_else(|| ResourceSetError::InvalidReturn("expected dictionary".into()))?;
    budget
        .borrow_mut()
        .pay(2, std::mem::size_of::<[Option<Value<'_>>; 3]>())?;
    let mut keys = dictionary.iter();
    loop {
        check()?;
        budget.borrow_mut().pay(1, 0)?;
        let Some((key, _)) = keys.next() else { break };
        if key.unpack_str().is_none() {
            return Err(ResourceSetError::InvalidReturn(
                "resource keys must be strings".into(),
            ));
        }
    }
    let mut values = [None; 3];
    let mut entries = dictionary.iter();
    loop {
        check()?;
        budget.borrow_mut().pay(1, 0)?;
        let Some((key, value)) = entries.next() else {
            break;
        };
        match key.unpack_str().expect("key types validated") {
            "memory" => values[0] = Some(value),
            "cpu" => values[1] = Some(value),
            "local_test" => values[2] = Some(value),
            _ => {
                return Err(ResourceSetError::InvalidReturn(
                    "unknown resource key".into(),
                ));
            }
        }
    }
    let memory_mb = numeric(values[0], 250.0, budget, check)?;
    let cpu = numeric(values[1], 1.0, budget, check)?;
    let local_test = numeric(values[2], 0.0, budget, check)? as i32;
    check()?;
    // Rust's float-to-int cast truncates, saturates, and maps NaN to zero.
    Ok(ResourceEstimate {
        memory_mb,
        cpu,
        local_test,
    })
}
struct BoundedMessage<'a> {
    text: String,
    budget: &'a RefCell<ResultBudget>,
    credit: usize,
}
impl Write for BoundedMessage<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let next = self.text.len().checked_add(value.len()).ok_or(fmt::Error)?;
        let forecast = next.max(8).checked_mul(4).ok_or(fmt::Error)?;
        let additional = forecast.checked_sub(self.credit).ok_or(fmt::Error)?;
        self.budget
            .borrow_mut()
            .pay(1, additional)
            .map_err(|_| fmt::Error)?;
        self.credit = forecast;
        self.text.push_str(value);
        Ok(())
    }
}
pub(super) fn message(
    budget: &RefCell<ResultBudget>,
    context: &dyn ResourceSetEvaluationContext,
    error: impl fmt::Display,
) -> ResourceSetError {
    match message_text(budget, context, error) {
        Ok(text) => ResourceSetError::Callback(text.into()),
        Err(error) => error,
    }
}

/// Keep numeric/limit/cancellation categories while adding bounded consumer context.
pub(super) fn contextualize(
    budget: &RefCell<ResultBudget>,
    context: &dyn ResourceSetEvaluationContext,
    error: ResourceSetError,
) -> ResourceSetError {
    match error {
        ResourceSetError::Cancelled => ResourceSetError::Cancelled,
        ResourceSetError::Limit(message) => ResourceSetError::Limit(message),
        ResourceSetError::Unsupported(message) => match message_text(budget, context, &message) {
            Ok(text) => ResourceSetError::Unsupported(text.into()),
            Err(error) => error,
        },
        ResourceSetError::InvalidReturn(message) => match message_text(budget, context, &message) {
            Ok(text) => ResourceSetError::InvalidReturn(text.into()),
            Err(error) => error,
        },
        ResourceSetError::Callback(message) => match message_text(budget, context, &message) {
            Ok(text) => ResourceSetError::Callback(text.into()),
            Err(error) => error,
        },
    }
}
fn message_text(
    budget: &RefCell<ResultBudget>,
    context: &dyn ResourceSetEvaluationContext,
    error: impl fmt::Display,
) -> Result<String, ResourceSetError> {
    budget.borrow_mut().pay(
        2,
        std::mem::size_of::<BoundedMessage<'_>>() + std::mem::size_of::<String>(),
    )?;
    let mut result = BoundedMessage {
        text: String::new(),
        budget,
        credit: 0,
    };
    if write!(
        &mut result,
        "{} resource_set: {}",
        context.mnemonic(),
        error
    )
    .is_err()
    {
        return Err(ResourceSetError::Limit("resource error rendering".into()));
    }
    Ok(result.text)
}
