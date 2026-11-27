use super::Arguments;
use super::FunctionError;
use crate::collections::symbol::symbol::Symbol;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::values::BoundedText;
use crate::values::Heap;
use crate::values::StringValue;
use crate::values::StringValueLike;
use crate::values::Value;
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
