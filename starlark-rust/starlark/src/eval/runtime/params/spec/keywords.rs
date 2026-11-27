use starlark_map::Hashed;
use starlark_map::small_map::SmallMap;

use crate::__macro_refs::coerce;
use crate::eval::ExecutionBudget;
use crate::eval::runtime::arguments::FunctionError;
use crate::eval::runtime::arguments::bounded_name_matches;
use crate::values::Heap;
use crate::values::StringValue;
use crate::values::Value;
use crate::values::ValueLike;
use crate::values::dict::Dict;

pub(super) enum CapturedKeywords<'v, 'a> {
    Ordinary(Option<SmallMap<StringValue<'v>, Value<'v>>>),
    Budgeted {
        rows: Vec<(Hashed<Value<'v>>, Value<'v>)>,
        budget: &'a ExecutionBudget,
    },
}

impl<'v, 'a> CapturedKeywords<'v, 'a> {
    pub(super) fn new(budget: Option<&'a ExecutionBudget>) -> Self {
        match budget {
            Some(budget) => Self::Budgeted {
                rows: Vec::new(),
                budget,
            },
            None => Self::Ordinary(None),
        }
    }

    pub(super) fn push_explicit(
        &mut self,
        key: Hashed<StringValue<'v>>,
        value: Value<'v>,
    ) -> crate::Result<()> {
        match self {
            Self::Ordinary(map) => {
                map.get_or_insert_with(|| SmallMap::with_capacity(12))
                    .insert_hashed_unique_unchecked(key, value);
            }
            Self::Budgeted { rows, budget } => {
                let key = Hashed::new_unchecked(key.hash(), key.key().to_value());
                budget.push_vec(rows, (key, value))?;
            }
        }
        Ok(())
    }

    pub(super) fn len(&self) -> usize {
        match self {
            Self::Ordinary(map) => map.as_ref().map_or(0, SmallMap::len),
            Self::Budgeted { rows, .. } => rows.len(),
        }
    }

    /// The caller's dictionary already has unique keys. Check only the explicit
    /// prefix for conflicts with a dictionary key.
    pub(super) fn push_unpacked(
        &mut self,
        key: Hashed<StringValue<'v>>,
        value: Value<'v>,
        explicit_len: usize,
    ) -> crate::Result<bool> {
        match self {
            Self::Ordinary(map) => Ok(map
                .get_or_insert_with(|| SmallMap::with_capacity(12))
                .insert_hashed(key, value)
                .is_some()),
            Self::Budgeted { rows, budget } => {
                for (candidate, _) in &rows[..explicit_len] {
                    if candidate.hash() == key.hash() {
                        if bounded_name_matches(
                            candidate.key().unpack_str().unwrap(),
                            key.key().as_str(),
                            budget,
                        )? {
                            return Ok(true);
                        }
                    } else {
                        budget.charge(1, 0)?;
                    }
                }
                let key = Hashed::new_unchecked(key.hash(), key.key().to_value());
                budget.push_vec(rows, (key, value))?;
                Ok(false)
            }
        }
    }

    pub(super) fn alloc(self, heap: Heap<'v>) -> crate::Result<Value<'v>> {
        match self {
            Self::Ordinary(map) => Ok(heap.alloc(match map {
                Some(map) => Dict::new(coerce(map)),
                None => Dict::default(),
            })),
            Self::Budgeted { rows, budget } => {
                heap.alloc_budgeted_dict_from_unique_rows(rows.len(), rows, budget)
            }
        }
    }

    pub(super) fn check_empty(self, function: &str) -> crate::Result<()> {
        match self {
            Self::Ordinary(Some(map)) => Err(FunctionError::ExtraNamedArg {
                names: map.keys().map(|name| name.as_str().to_owned()).collect(),
                function: function.to_owned(),
            }
            .into()),
            Self::Budgeted { rows, budget } if !rows.is_empty() => {
                Err(FunctionError::bounded_extra_named(
                    budget,
                    function,
                    rows.iter()
                        .map(|(name, _)| name.key().unpack_str().unwrap()),
                ))
            }
            _ => Ok(()),
        }
    }
}
