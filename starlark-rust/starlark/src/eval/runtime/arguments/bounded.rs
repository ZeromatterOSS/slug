use starlark_map::small_map::BoundedMapError;

use super::Arguments;
use super::FunctionError;
use crate::collections::Hashed;
use crate::collections::SmallMap;
use crate::collections::symbol::symbol::Symbol;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::values::BoundedText;
use crate::values::Heap;
use crate::values::StringValue;
use crate::values::StringValueLike;
use crate::values::Value;
use crate::values::ValueLike;
use crate::values::dict::Dict;
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
        let rows =
            SmallMap::try_from_unique_hashed_rows(count, explicit.chain(kwargs), |work, bytes| {
                budget.charge(work, bytes)
            });
        let rows = match rows {
            Ok(rows) => rows,
            Err(BoundedMapError::Operation(limit)) => return Err(limit.into()),
            Err(BoundedMapError::SizeOverflow) => {
                return Err(budget.refuse(ExecutionLimit::AllocationBytes).into());
            }
            Err(BoundedMapError::AllocationFailed) => {
                return Err(budget.refuse(ExecutionLimit::AllocationFailed).into());
            }
            Err(BoundedMapError::RowCountMismatch) => {
                return Err(crate::Error::new_kind(crate::ErrorKind::Internal(
                    anyhow::anyhow!("validated kwargs row count changed",),
                )));
            }
        };
        Ok(heap.alloc_budgeted_dict(Dict::new(rows))?)
    }
}

fn find_name<'v>(
    entries: impl Iterator<Item = (&'v str, Value<'v>)>,
    name: &str,
    budget: &ExecutionBudget,
) -> Result<Option<Value<'v>>, ExecutionLimit> {
    for (key, value) in entries {
        // Length-mismatched strings do not compare bytes.
        let compared = if key.len() == name.len() {
            ExecutionBudget::text_work(name.len())
        } else {
            0
        };
        budget.charge(1 + compared, 0)?;
        if key == name {
            return Ok(Some(value));
        }
    }
    Ok(None)
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

        // Write the diagnostic once so displaying it never joins owned key copies.
        let mut message = BoundedText::new(budget, usize::MAX);
        message.push_str("Found `")?;
        for (index, name) in keys.enumerate() {
            budget.charge(1, 0)?;
            if index != 0 {
                message.push_str("` `")?;
            }
            message.push_str(name)?;
        }
        message.push_str("` extra named parameter(s) for call to function")?;
        Err(crate::Error::new_kind(crate::ErrorKind::Function(
            anyhow::Error::msg(message.finish()?),
        )))
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
                    let mut name = BoundedText::new(budget, usize::MAX);
                    name.push_str(key)?;
                    return Err(FunctionError::RepeatedArg {
                        name: name.finish()?,
                    }
                    .into());
                }
            }
        }
        Ok(names)
    }
}
