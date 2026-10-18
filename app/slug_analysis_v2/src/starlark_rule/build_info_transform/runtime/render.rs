//! Borrowed status fields and returned dictionary; no owned substitution pipeline.
use std::cell::RefCell;
use std::mem::size_of;

use slug_build_api_v2::BUILD_INFO_MAX_OUTPUT_BYTES;
use slug_build_api_v2::BuildInfoTransformError as Error;
use starlark::environment::Module;
use starlark::values::FrozenValue;
use starlark::values::Value;
use starlark::values::dict::AllocDict;
use starlark::values::dict::DictRef;

use super::budget::Budget;
use super::budget::add;
use super::budget::mul;
use super::budget::text_work;
use super::budget::{self};

pub(super) fn latin1(
    bytes: &[u8],
    ledger: &RefCell<Budget>,
    check: &impl Fn() -> Result<(), Error>,
) -> Result<String, Error> {
    ledger.borrow_mut().pay(
        add(1, text_work(bytes.len()))?,
        add(mul(bytes.len(), 2)?, size_of::<String>())?,
    )?;
    let mut text = String::with_capacity(mul(bytes.len(), 2)?);
    for chunk in bytes.chunks(256) {
        check()?;
        for byte in chunk {
            text.push(char::from(*byte));
        }
    }
    Ok(text)
}
/// Sorted borrowed pairs make last-duplicate wins explicit before frozen allocation.
fn status<'a>(
    raw: &'a [u8],
    ledger: &RefCell<Budget>,
    check: &impl Fn() -> Result<(), Error>,
) -> Result<Vec<(&'a [u8], &'a [u8])>, Error> {
    ledger.borrow_mut().pay(
        add(2, text_work(raw.len()))?,
        size_of::<Vec<(&[u8], &[u8])>>(),
    )?;
    let mut rows: Vec<(&[u8], &[u8])> = Vec::new();
    let mut credit = 0;
    let mut lines = raw.split(|c| *c == b'\n');
    loop {
        check()?;
        ledger.borrow_mut().pay(1, 0)?;
        let Some(line) = lines.next() else { break };
        ledger.borrow_mut().pay(add(1, text_work(line.len()))?, 0)?;
        let Some(space) = line.iter().position(|c| *c == b' ') else {
            continue;
        };
        let (key, value) = (&line[..space], &line[space + 1..]);
        let mut low = 0;
        let mut high = rows.len();
        let mut equal = None;
        while low < high {
            check()?;
            let mid = low + (high - low) / 2;
            ledger.borrow_mut().pay(
                add(1, add(text_work(key.len()), text_work(rows[mid].0.len()))?)?,
                0,
            )?;
            match rows[mid].0.cmp(key) {
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Greater => high = mid,
                std::cmp::Ordering::Equal => {
                    equal = Some(mid);
                    break;
                }
            }
        }
        if let Some(index) = equal {
            rows[index].1 = value;
        } else {
            let next = add(rows.len(), 1)?;
            let forecast = add(
                mul(next.max(4), 4 * size_of::<(&[u8], &[u8])>())?,
                size_of::<Vec<(&[u8], &[u8])>>(),
            )?;
            ledger
                .borrow_mut()
                .pay(add(1, rows.len() - low)?, forecast - credit)?;
            credit = forecast;
            rows.insert(low, (key, value));
        }
    }
    Ok(rows)
}
pub(super) fn input_dict<'v>(
    raw: &[u8],
    module: &'v Module,
    ledger: &RefCell<Budget>,
    check: &impl Fn() -> Result<(), Error>,
) -> Result<Value<'v>, Error> {
    let rows = status(raw, ledger, check)?;
    budget::vector::<(FrozenValue, FrozenValue)>(ledger, rows.len())?;
    // AllocDict's exact-size reserve, bucket/control/alignment and frozen header.
    ledger.borrow_mut().pay(
        add(2, rows.len())?,
        add(
            mul(rows.len().max(4), 8 * (2 * size_of::<FrozenValue>() + 32))?,
            128,
        )?,
    )?;
    let mut values = Vec::with_capacity(rows.len());
    for (key, value) in rows {
        check()?;
        let key = latin1(key, ledger, check)?;
        let value = latin1(value, ledger, check)?;
        ledger.borrow_mut().pay(
            add(
                2,
                mul(2, add(text_work(key.len()), text_work(value.len()))?)?,
            )?,
            add(add(key.len(), value.len())?, 128)?,
        )?;
        values.push((
            module.frozen_heap().alloc(key.as_str()),
            module.frozen_heap().alloc(value.as_str()),
        ));
    }
    Ok(module.frozen_heap().alloc(AllocDict(values)).to_value())
}
pub(super) fn output(
    value: Value<'_>,
    template: &[u8],
    ledger: &RefCell<Budget>,
    check: &impl Fn() -> Result<(), Error>,
) -> Result<Vec<u8>, Error> {
    check()?;
    ledger.borrow_mut().pay(1, 0)?;
    let dict = DictRef::from_value(value)
        .ok_or_else(|| Error::InvalidReturn("expected dictionary".into()))?;
    let mut pairs = dict.iter();
    loop {
        check()?;
        ledger.borrow_mut().pay(1, 0)?;
        let Some((k, v)) = pairs.next() else { break };
        ledger.borrow_mut().pay(2, 0)?;
        if k.unpack_str().is_none() {
            return Err(Error::InvalidReturn(
                "substitution key must be a string".into(),
            ));
        }
        if v.unpack_str().is_none() {
            return Err(Error::InvalidReturn(
                "substitution value must be a string".into(),
            ));
        }
    }
    let mut text = latin1(template, ledger, check)?;
    let mut pairs = dict.iter();
    loop {
        check()?;
        ledger.borrow_mut().pay(1, 0)?;
        let Some((key, value)) = pairs.next() else {
            break;
        };
        let key = key.unpack_str().expect("validated key");
        let value = value.unpack_str().expect("validated value");
        if key.is_empty() {
            continue;
        }
        // Two real searches: exact size forecast, then copy nonoverlapping matches.
        ledger.borrow_mut().pay(
            add(2, add(text_work(text.len()), text_work(key.len()))?)?,
            0,
        )?;
        let mut count = 0;
        for _ in text.match_indices(key) {
            check()?;
            count = add(count, 1)?;
        }
        check()?;
        if count == 0 {
            continue;
        }
        let removed = mul(count, key.len())?;
        let inserted = mul(count, value.len())?;
        let next = add(
            text.len()
                .checked_sub(removed)
                .ok_or_else(|| Error::Limit("replacement size".into()))?,
            inserted,
        )?;
        ledger.borrow_mut().pay(
            add(
                add(2, mul(count, 2)?)?,
                add(
                    add(text_work(text.len()), text_work(key.len()))?,
                    text_work(next),
                )?,
            )?,
            add(next, size_of::<String>())?,
        )?;
        let mut replaced = String::with_capacity(next);
        let mut previous = 0;
        for (offset, _) in text.match_indices(key) {
            check()?;
            replaced.push_str(&text[previous..offset]);
            replaced.push_str(value);
            previous = offset + key.len();
        }
        replaced.push_str(&text[previous..]);
        text = replaced;
    }
    check()?;
    ledger.borrow_mut().pay(add(1, text_work(text.len()))?, 0)?;
    let count = text.chars().count();
    if count > BUILD_INFO_MAX_OUTPUT_BYTES {
        return Err(Error::Limit("final output exceeds 2MiB".into()));
    }
    ledger
        .borrow_mut()
        .pay(add(1, text_work(count))?, add(count, size_of::<Vec<u8>>())?)?;
    let mut bytes = Vec::with_capacity(count);
    for ch in text.chars() {
        check()?;
        bytes.push(u8::try_from(ch as u32).unwrap_or(b'?'));
    }
    Ok(bytes)
}
