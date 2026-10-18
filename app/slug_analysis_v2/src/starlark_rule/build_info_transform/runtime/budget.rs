//! One monotone poisoned work/controlled-allocation ledger for the entire call.
use std::cell::RefCell;
use std::fmt::Write;
use std::fmt::{self};
use std::mem::size_of;

use slug_build_api_v2::BuildInfoEvaluationContext;
use slug_build_api_v2::BuildInfoTransformError as Error;
use slug_build_api_v2::published_value::BYTE_LIMIT;
use slug_build_api_v2::published_value::NODE_LIMIT;
#[derive(Default, Debug)]
pub(super) struct Budget {
    pub(super) work: usize,
    pub(super) bytes: usize,
    pub(super) failed: bool,
}
impl Budget {
    pub(super) fn pay(&mut self, work: usize, bytes: usize) -> Result<(), Error> {
        if self.failed {
            return Err(Error::Limit("invocation ledger poisoned".into()));
        }
        match self
            .work
            .checked_add(work)
            .zip(self.bytes.checked_add(bytes))
        {
            Some((w, b)) if w <= NODE_LIMIT && b <= BYTE_LIMIT => {
                self.work = w;
                self.bytes = b;
                Ok(())
            }
            _ => {
                self.failed = true;
                Err(Error::Limit("invocation work/controlled bytes".into()))
            }
        }
    }
}
/// One text-work record covers at most 256 examined/copied bytes. Byte storage
/// remains charged byte-for-byte; separate real scans/copies each pay separately.
/// This is a bounded unit, not a credit or omission of long no-match scans.
pub(super) const TEXT_WORK_BYTES: usize = 256;
pub(super) fn text_work(bytes: usize) -> usize {
    bytes.div_ceil(TEXT_WORK_BYTES)
}
pub(super) fn mul(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_mul(b)
        .ok_or_else(|| Error::Limit("invocation size overflow".into()))
}
pub(super) fn add(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_add(b)
        .ok_or_else(|| Error::Limit("invocation size overflow".into()))
}
/// Prepay full old/new/coexisting vector capacity and header before allocation.
pub(super) fn vector<T>(budget: &RefCell<Budget>, count: usize) -> Result<(), Error> {
    budget.borrow_mut().pay(
        2,
        add(mul(count.max(4), 4 * size_of::<T>())?, size_of::<Vec<T>>())?,
    )
}
struct Message<'a> {
    text: String,
    budget: &'a RefCell<Budget>,
    credit: usize,
}
impl Write for Message<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let next = self.text.len().checked_add(s.len()).ok_or(fmt::Error)?;
        let forecast = next.max(8).checked_mul(4).ok_or(fmt::Error)?;
        let extra = forecast.checked_sub(self.credit).ok_or(fmt::Error)?;
        self.budget
            .borrow_mut()
            .pay(1 + text_work(s.len()), extra)
            .map_err(|_| fmt::Error)?;
        self.credit = forecast;
        self.text.push_str(s);
        Ok(())
    }
}
pub(super) fn contextualize(
    budget: &RefCell<Budget>,
    context: &dyn BuildInfoEvaluationContext,
    error: Error,
) -> Error {
    if matches!(error, Error::Cancelled | Error::Limit(_)) {
        return error;
    }
    let paid = budget
        .borrow_mut()
        .pay(2, size_of::<Message<'_>>() + size_of::<String>());
    if let Err(error) = paid {
        return error;
    }
    let mut out = Message {
        text: String::new(),
        budget,
        credit: 0,
    };
    let message = match &error {
        Error::Callback(s) | Error::InvalidReturn(s) | Error::Unsupported(s) => s.as_str(),
        _ => unreachable!(),
    };
    if write!(&mut out, "{} build-info: {}", context.mnemonic(), message).is_err() {
        return Error::Limit("error rendering".into());
    }
    match error {
        Error::Callback(_) => Error::Callback(out.text.into()),
        Error::InvalidReturn(_) => Error::InvalidReturn(out.text.into()),
        _ => Error::Unsupported(out.text.into()),
    }
}
pub(super) fn callback_error(
    budget: &RefCell<Budget>,
    context: &dyn BuildInfoEvaluationContext,
    error: impl fmt::Display,
) -> Error {
    if let Err(error) = budget
        .borrow_mut()
        .pay(2, size_of::<Message<'_>>() + size_of::<String>())
    {
        return error;
    }
    let mut out = Message {
        text: String::new(),
        budget,
        credit: 0,
    };
    if write!(&mut out, "{} build-info: {}", context.mnemonic(), error).is_err() {
        return Error::Limit("callback error rendering".into());
    }
    Error::Callback(out.text.into())
}
