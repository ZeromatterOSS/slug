use std::fmt;
use std::fmt::Write;

use super::Arguments;
use super::FunctionError;
use crate::collections::Hashed;
use crate::collections::symbol::symbol::Symbol;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::values::BoundedText;
use crate::values::Heap;
use crate::values::StringValue;
use crate::values::StringValueLike;
use crate::values::Value;
use crate::values::ValueLike;
use crate::values::dict::DictRef;

/// Borrowed, validated named arguments. Lookups pay for scanned keys instead of
/// rebuilding an index or depending on the distribution of caller hashes.
pub(crate) struct BoundedFormatNames<'v, 'a> {
    explicit: &'a [(Symbol, StringValue<'v>)],
    values: &'a [Value<'v>],
    kwargs: Option<DictRef<'v>>,
}

impl<'v> BoundedFormatNames<'v, '_> {
    fn explicit(&self) -> impl Iterator<Item = (&'v str, Value<'v>)> + '_ {
        self.explicit
            .iter()
            .zip(self.values)
            .map(|((_, name), value)| (name.as_str(), *value))
    }

    pub(crate) fn get(
        &self,
        name: &str,
        budget: &ExecutionBudget,
    ) -> crate::Result<Option<Value<'v>>> {
        let kwargs = self.kwargs.iter().flat_map(|kwargs| {
            kwargs.iter().map(|(key, value)| {
                // The borrow remains live since validation.
                (key.unpack_str().unwrap(), value)
            })
        });
        Ok(find_name(self.explicit().chain(kwargs), name, budget)?)
    }

    /// Materialize unique validated names in call order, reusing their stored hashes.
    pub(crate) fn alloc_dict(
        self,
        heap: Heap<'v>,
        budget: &ExecutionBudget,
    ) -> crate::Result<Value<'v>> {
        let count = self
            .explicit
            .len()
            .checked_add(self.kwargs.as_ref().map_or(0, |kwargs| kwargs.len()))
            .ok_or_else(|| budget.refuse(ExecutionLimit::AllocationBytes))?;
        let explicit = self
            .explicit
            .iter()
            .zip(self.values)
            .map(|((symbol, name), value)| {
                (
                    Hashed::new_unchecked(symbol.small_hash(), name.to_value()),
                    *value,
                )
            });
        let kwargs = self.kwargs.iter().flat_map(|kwargs| kwargs.iter_hashed());
        heap.alloc_budgeted_dict_from_unique_rows(count, explicit.chain(kwargs), budget)
    }
}

fn find_name<'v>(
    entries: impl Iterator<Item = (&'v str, Value<'v>)>,
    name: &str,
    budget: &ExecutionBudget,
) -> Result<Option<Value<'v>>, ExecutionLimit> {
    for (key, value) in entries {
        if bounded_name_matches(key, name, budget)? {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

pub(crate) fn bounded_name_matches(
    left: &str,
    right: &str,
    budget: &ExecutionBudget,
) -> Result<bool, ExecutionLimit> {
    let compared = if left.len() == right.len() {
        ExecutionBudget::text_work(left.len())
    } else {
        0
    };
    budget.charge(1 + compared, 0)?;
    Ok(left == right)
}

impl FunctionError {
    fn render_bounded(
        budget: &ExecutionBudget,
        render: impl FnOnce(&mut BoundedText<'_>) -> Result<(), ExecutionLimit>,
    ) -> crate::Error {
        let mut message = BoundedText::new(budget, usize::MAX);
        match render(&mut message).and_then(|()| message.finish()) {
            Ok(message) => {
                crate::Error::new_kind(crate::ErrorKind::Function(anyhow::Error::msg(message)))
            }
            Err(limit) => limit.into(),
        }
    }

    pub(crate) fn bounded_message(
        budget: &ExecutionBudget,
        message: fmt::Arguments<'_>,
    ) -> crate::Error {
        Self::render_bounded(budget, |output| {
            output
                .write_fmt(message)
                .map_err(|_| budget.refuse(ExecutionLimit::TextFormatting))
        })
    }

    pub(crate) fn bounded_extra_named<'a>(
        budget: &ExecutionBudget,
        function: &str,
        names: impl Iterator<Item = &'a str>,
    ) -> crate::Error {
        Self::render_bounded(budget, |message| {
            message.push_str("Found `")?;
            for (index, name) in names.enumerate() {
                budget.charge(1, 0)?;
                if index != 0 {
                    message.push_str("` `")?;
                }
                message.push_str(name)?;
            }
            message.push_str("` extra named parameter(s) for call to ")?;
            message.push_str(function)
        })
    }
}

impl<'v, 'a> Arguments<'v, 'a> {
    /// Admit iterator startup and ensure each step has fixed work and storage.
    /// The caller charges steps before advancing the returned iterator.
    pub(crate) fn bounded_positions<'b>(
        &'b self,
        heap: Heap<'v>,
        budget: &ExecutionBudget,
    ) -> crate::Result<impl Iterator<Item = Value<'v>> + 'b> {
        budget.charge(1, 0)?;
        self.positions_with(move |value| {
            heap.bounded_iterator(value, "positional argument expansion")
        })
    }

    pub(super) fn bounded_optional<const REQUIRED: usize, const OPTIONAL: usize>(
        &self,
        heap: Heap<'v>,
        budget: &ExecutionBudget,
    ) -> crate::Result<([Value<'v>; REQUIRED], [Option<Value<'v>>; OPTIONAL])> {
        let mut positions = self.bounded_positions(heap, budget)?;
        let mut required = [Value::new_none(); REQUIRED];
        let mut optional = [None; OPTIONAL];
        let mut got = 0;
        loop {
            budget.charge(1, 0)?;
            let Some(value) = positions.next() else {
                break;
            };
            if got < REQUIRED {
                required[got] = value;
            } else if let Some(slot) = optional.get_mut(got - REQUIRED) {
                *slot = Some(value);
            }
            got += 1;
        }
        if got < REQUIRED || got > REQUIRED + OPTIONAL {
            return Err(FunctionError::WrongNumberOfArgs {
                min: REQUIRED,
                max: REQUIRED + OPTIONAL,
                got,
            }
            .into());
        }
        Ok((required, optional))
    }

    pub(crate) fn bounded_no_named_args(&self, budget: &ExecutionBudget) -> crate::Result<()> {
        budget.check()?;
        let explicit = self.0.names.names();
        let kwargs = self.unpack_kwargs()?;
        if explicit.is_empty() && kwargs.as_ref().is_none_or(|kwargs| kwargs.is_empty()) {
            return Ok(());
        }

        // A malformed ** argument takes precedence over unexpected names. Keep the
        // dictionary borrowed across both passes so validated names cannot change.
        if let Some(kwargs) = &kwargs {
            for key in kwargs.keys() {
                budget.charge(1, 0)?;
                Self::unpack_kwargs_key(key)?;
            }
        }
        let keys = explicit.iter().map(|(name, _)| name.as_str()).chain(
            kwargs
                .iter()
                .flat_map(|kwargs| kwargs.keys().map(|key| key.unpack_str().unwrap())),
        );

        Err(FunctionError::bounded_extra_named(budget, "function", keys))
    }

    pub(crate) fn bounded_format_names(
        &self,
        budget: &ExecutionBudget,
    ) -> crate::Result<BoundedFormatNames<'v, 'a>> {
        budget.check()?;
        let names = BoundedFormatNames {
            explicit: self.0.names.names(),
            values: self.0.named,
            kwargs: self.unpack_kwargs()?,
        };
        if let Some(kwargs) = &names.kwargs {
            for (key, _) in kwargs.iter() {
                budget.charge(1, 0)?;
                let key = Self::unpack_kwargs_key(key)?;
                if find_name(names.explicit(), key, budget)?.is_some() {
                    return Err(FunctionError::bounded_message(
                        budget,
                        format_args!("Argument `{key}` occurs more than once"),
                    ));
                }
            }
        }
        Ok(names)
    }
}
