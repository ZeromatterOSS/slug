//! Closed declarations for the exact environments held by these native rows.
use starlark_map::small_set::SmallSet;

use super::*;
use crate::bzl_environment::BzlEnvironmentInputs;
use crate::bzl_environment::publication_shape;
use crate::bzl_environment::references::EnvironmentReferencePlan;
use crate::bzl_environment::references::descriptor_memo_scratch_bytes;
use crate::bzl_module::semantic_evaluation::budget::Budget;

impl ImmutablePublicationInput for BzlEnvironmentInputs {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn retained_shape(&self) -> PublicationShape {
        self.publication().full()
    }
    fn intrinsic_shape(&self) -> PublicationShape {
        self.publication().intrinsic()
    }
    fn shared_inputs(&self) -> &[SharedPublicationInput] {
        self.publication().children()
    }
}

pub(super) fn uses(
    rows: &[Arc<LocalBzlInput>],
) -> impl Iterator<Item = &Arc<BzlEnvironmentInputs>> + Clone {
    rows.iter().flat_map(|row| {
        std::iter::once(&row.key.environment).chain(row.children.iter().map(|key| &key.environment))
    })
}
fn operations(work: &mut PublicationShape, count: usize, bytes: usize) -> anyhow::Result<()> {
    add_shape(
        work,
        PublicationShape {
            visits: count,
            bytes,
            ..zero_shape()
        },
        0,
    )
}
pub(super) fn table_shape(
    references: &EnvironmentReferencePlan,
    work: &mut PublicationShape,
) -> anyhow::Result<(PublicationShape, usize)> {
    // The cached length and fixed table arithmetic are new work, not free facts.
    operations(
        work,
        1,
        std::mem::size_of::<Budget>() + std::mem::size_of::<usize>(),
    )?;
    let (table, count) = references
        .owner_table_budget()
        .map_err(anyhow::Error::msg)?;
    Ok((publication_shape(table), count))
}
fn peak(retained: PublicationShape, work: PublicationShape) -> anyhow::Result<()> {
    crate::bzl_module::semantic_evaluation::budget::admit_peak(
        PeakContext::new(
            PeakOwner::FunctionOwner,
            PeakPhase::NativeTransfer,
            FirstLedger::NativeRetained,
        ),
        Budget {
            records: retained.visits,
            bytes: retained.bytes,
            depth: retained.depth,
        },
        Budget {
            records: work.visits,
            bytes: work.bytes,
            depth: work.depth,
        },
        0,
    )
    .map_err(anyhow::Error::msg)
}
fn overflow() -> anyhow::Error {
    anyhow::anyhow!("environment shared reference overflow")
}

/// No guard is cloned here. Rows retain every borrowed env/catalog until this
/// scratch set drops; actual addresses are only a no-ABA physical admission key.
pub(super) fn admit(
    rows: &[Arc<LocalBzlInput>],
    count: usize,
    old_shared_count: usize,
    retained: &mut PublicationShape,
    work: &mut PublicationShape,
) -> anyhow::Result<usize> {
    // Three paid field/iterator operations per original occurrence, before walk.
    operations(work, count.checked_mul(3).ok_or_else(overflow)?, 0)?;
    let mut catalogs = 0usize;
    let mut encountered = 0usize;
    for environment in uses(rows) {
        encountered = encountered.checked_add(1).ok_or_else(overflow)?;
        if environment.own_catalog().is_some() {
            catalogs = catalogs.checked_add(1).ok_or_else(overflow)?;
        }
    }
    anyhow::ensure!(
        encountered == count,
        "environment declaration count mismatch"
    );
    let extra = count.checked_add(catalogs).ok_or_else(overflow)?;
    let total = old_shared_count.checked_add(extra).ok_or_else(overflow)?;
    let handles = PublicationShape {
        visits: extra,
        edges: extra,
        bytes: extra
            .checked_mul(std::mem::size_of::<SharedPublicationInput>())
            .ok_or_else(overflow)?,
        depth: 1,
        nodes: 0,
    };
    // These legacy handles are no longer stored in validity. Their full
    // physical preflight stays conservative; only local intrinsic omits them.
    add_shape(retained, handles, 0)?;
    // Five slots cover Vec old/new/final Arc transfer; clone/coercion/append and
    // the later iterator/field reads all precede the corresponding copies.
    operations(
        work,
        extra.checked_mul(5).ok_or_else(overflow)?,
        handles.bytes.checked_mul(5).ok_or_else(overflow)?,
    )?;
    operations(
        work,
        1,
        std::mem::size_of::<Budget>() + 8 * std::mem::size_of::<usize>(),
    )?;
    let scratch = descriptor_memo_scratch_bytes(count).map_err(anyhow::Error::msg)?;
    operations(work, 1, scratch)?;
    peak(*retained, *work)?;
    // SmallSet<usize> is SmallMap<usize,()>: the prepaid usize/usize bound
    // dominates its entries/alignment/hash array/index and minimum capacity.
    let mut admitted = SmallSet::<usize>::with_capacity(count);
    // Prepay the complete second borrowed walk, including own-catalog reads.
    operations(work, count.checked_mul(3).ok_or_else(overflow)?, 0)?;
    for environment in uses(rows) {
        if let Some(catalog) = environment.own_catalog() {
            // Both actual pointer read and lookup precede any cached body read.
            operations(work, 2, 0)?;
            let pointer = catalog.pointer();
            if !admitted.contains(&pointer.1) {
                operations(work, 2, std::mem::size_of::<Budget>())?;
                add_shape(retained, publication_shape(catalog.physical_budget()), 1)?;
                peak(*retained, *work)?;
                admitted.insert(pointer.1);
            }
        }
    }
    peak(*retained, *work)?;
    Ok(total)
}

/// Reconstruct the former direct handle array for independent storage oracles.
/// Production retains these complete bodies through its declared row DAG.
#[cfg(test)]
pub(super) fn append(rows: &[Arc<LocalBzlInput>], out: &mut Vec<SharedPublicationInput>) {
    for environment in uses(rows) {
        out.push(SharedPublicationInput::new(environment.clone()));
        if let Some(catalog) = environment.own_catalog() {
            out.push(catalog.shared());
        }
    }
}

#[cfg(test)]
#[path = "environment_publication/tests.rs"]
mod tests;
