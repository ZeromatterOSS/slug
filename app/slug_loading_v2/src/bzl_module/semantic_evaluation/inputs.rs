//! Borrowed preflight precedes semantic-key storage and lifetime guard copies.
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;

use starlark_map::small_map::SmallMap;

use super::*;

#[derive(Debug, Allocative)]
pub(super) struct ActualChild {
    pub(super) raw: Arc<str>,
    pub(super) module: FrozenBzlModule,
}
#[derive(Debug, Allocative)]
pub(in crate::bzl_module) struct PreparedSemanticBzlEvaluation {
    pub(super) rows: Arc<[Arc<facts::LocalFacts>]>,
    pub(super) children: Box<[ActualChild]>,
    pub(super) budget: budget::Budget,
}
impl PartialEq for PreparedSemanticBzlEvaluation {
    fn eq(&self, other: &Self) -> bool {
        self.rows == other.rows
    }
}
impl Eq for PreparedSemanticBzlEvaluation {}
impl Hash for PreparedSemanticBzlEvaluation {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.rows.hash(h);
    }
}

pub(in crate::bzl_module) fn prepare<'a>(
    identity: &BzlModuleIdentity,
    source_name: &str,
    source: &Arc<[u8]>,
    context: BzlModuleContext,
    children: impl Iterator<Item = (&'a str, &'a FrozenBzlModule)> + Clone,
) -> Result<SemanticBzlEvaluationKey, String> {
    std::str::from_utf8(source).map_err(|_| "semantic .bzl source is not valid UTF-8")?;
    // Validate borrowed producer certificates before projecting full child
    // stable keys for the deterministic repeated-binding work forecast.
    let mut validation = budget::Budget::default();
    for (_, child) in children.clone() {
        validation.charge(1, 0)?;
        if child
            .semantic_inputs
            .as_ref()
            .is_none_or(|inputs| inputs.rows.is_empty())
        {
            return Err("semantic .bzl child lacks actual semantic evaluation ownership".into());
        }
    }
    let local = facts::row_budget(
        identity,
        source_name,
        source,
        children.clone().map(|(raw, child)| {
            let inputs = child.semantic_inputs.as_ref().expect("checked child");
            (raw, inputs.rows[0].key.as_ref())
        }),
    )?;
    let mut logical = local;
    logical.charge(
        1,
        std::mem::size_of::<PreparedSemanticBzlEvaluation>() + 4 * std::mem::size_of::<usize>(),
    )?;
    let mut scratch = validation;
    // No owned identity/source/child guards exist until all borrowed checks pass.
    let mut borrowed = Vec::<&Arc<facts::LocalFacts>>::new();
    let mut index = SmallMap::<u64, Vec<usize>>::new();
    let mut direct_count = 0usize;
    let mut depth = 0usize;
    for (_, child) in children.clone() {
        direct_count = direct_count.checked_add(1).ok_or_else(budget::limit)?;
        let inputs = child
            .semantic_inputs
            .as_ref()
            .ok_or("semantic .bzl child lacks actual semantic evaluation ownership")?;
        depth = depth.max(inputs.budget.depth.saturating_add(1));
        logical.depth(depth)?;
        for row in &*inputs.rows {
            scratch.charge(1, 0)?;
            scratch.identity(&row.key.identity)?;
            scratch.charge(1, row.key.source_name.len())?;
            budget::admit_peak(logical, scratch, 0)?;
            let mut h = DefaultHasher::new();
            row.key.hash(&mut h);
            let hash = h.finish();
            if row.key.identity == *identity && row.key.context == context {
                return Err("semantic .bzl input closure contains its own module".into());
            }
            let mut duplicate = false;
            if let Some(candidates) = index.get(&hash) {
                for candidate in candidates {
                    let prior = borrowed[*candidate];
                    if Arc::ptr_eq(prior, row) {
                        duplicate = true;
                        break;
                    }
                    scratch.charge(prior.budget.records, prior.budget.bytes)?;
                    budget::admit_peak(logical, scratch, 0)?;
                    if prior.key == row.key {
                        if **prior != **row {
                            return Err(
                                "semantic .bzl input has conflicting complete local facts".into()
                            );
                        }
                        duplicate = true;
                        break;
                    }
                }
            }
            if duplicate {
                continue;
            }
            logical.charge(row.budget.records, row.budget.bytes)?;
            budget::admit_peak(logical, scratch, 0)?;
            // Both candidate list and outer row list are forecast before growth.
            let candidates = index.get(&hash).map_or(0, Vec::len);
            budget::admit_index_growth::<&Arc<facts::LocalFacts>>(
                logical,
                &mut scratch,
                borrowed.len(),
                candidates,
            )?;
            index.entry(hash).or_default().push(borrowed.len());
            borrowed.push(row);
        }
    }
    logical.array::<Arc<facts::LocalFacts>>(borrowed.len().saturating_add(1))?;
    budget::admit_peak(logical, scratch, 0)?;
    logical.array::<ActualChild>(direct_count)?;
    budget::admit_peak(logical, scratch, 0)?;
    // Facade clones have deterministic payloads; include them in both key and
    // event admission rather than depending on the caller's actual Arc sharing.
    for (_, child) in children.clone() {
        logical.charge(1, child.path.as_os_str().len())?;
        budget::admit_peak(logical, scratch, 0)?;
        logical.array::<String>(child.loads.len())?;
        budget::admit_peak(logical, scratch, 0)?;
        for load in &child.loads {
            logical.charge(1, load.len())?;
            budget::admit_peak(logical, scratch, 0)?;
        }
        logical.identity(&child.manifest.root)?;
        budget::admit_peak(logical, scratch, 0)?;
    }
    let mut physical = logical;
    physical.charge(0, scratch.bytes)?;
    // Retained arrays plus their construction/Vec-to-Arc transfer coexist.
    physical.array::<facts::Binding>(direct_count)?;
    physical.array::<Arc<facts::LocalFacts>>(borrowed.len().saturating_add(1))?;
    physical.array::<ActualChild>(direct_count)?;
    physical.charge(
        0,
        borrowed
            .capacity()
            .saturating_mul(std::mem::size_of::<&Arc<facts::LocalFacts>>()),
    )?;
    let key = Arc::new(facts::StableKey {
        identity: identity.clone(),
        source_name: source_name.into(),
        context,
    });
    let bindings = children
        .clone()
        .map(|(raw, child)| facts::Binding {
            raw: Arc::from(raw),
            child: child.semantic_inputs.as_ref().expect("checked child").rows[0]
                .key
                .clone(),
        })
        .collect::<Vec<_>>();
    let root = Arc::new(facts::LocalFacts {
        key,
        source: source.clone(),
        bindings: bindings.into(),
        budget: local,
    });
    let actual = children
        .enumerate()
        .map(|(index, (_raw, child))| ActualChild {
            raw: root.bindings[index].raw.clone(),
            module: child.clone(),
        })
        .collect::<Box<[_]>>();
    let mut rows = Vec::with_capacity(borrowed.len() + 1);
    rows.push(root);
    rows.extend(borrowed.into_iter().cloned());
    Ok(SemanticBzlEvaluationKey(Arc::new(
        PreparedSemanticBzlEvaluation {
            rows: rows.into(),
            children: actual,
            budget: logical,
        },
    )))
}
