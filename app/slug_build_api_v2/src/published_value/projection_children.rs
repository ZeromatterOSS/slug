//! Correspondence of native dense child models with original arena rows.
use super::*;
use crate::AnalysisDepset;
use crate::AnalysisDepsetSuccessor;
use crate::cc_variables::CcToolchainVariables;
use crate::cc_variables::CcVariableCacheEntry;

pub(super) fn validate_depset(
    d: &AnalysisDepset,
    o: &PublishedValueRef,
    depth: usize,
) -> Result<(), PublishedValueError> {
    if depth > DEPTH_LIMIT {
        return Err(PublishedValueError::Limit("projection depth"));
    }
    let mut visits = 0usize;
    let mut scratch = 0usize;
    let mut pending = Vec::new();
    // Values guard both physical-node and arena identities against address reuse.
    let mut seen = fxhash::FxHashMap::default();
    charge_depset_validation(
        &mut visits,
        &mut scratch,
        std::mem::size_of::<(AnalysisDepset, PublishedValueRef)>() * 2,
    )?;
    pending.push((d.clone(), o.clone()));
    while let Some((actual, original)) = pending.pop() {
        charge_depset_validation(&mut visits, &mut scratch, 0)?;
        let PublishedNode::Depset(row) = original.node() else {
            return Err(PublishedValueError::Invalid("depset row"));
        };
        if row.depth > crate::depset::MAX_DEPTH
            || actual.order() != row.order
            || actual.element_type() != row.element_type
            || actual.depth() != row.depth
            || !actual.occurrence().shares_with(&row.occurrence)
        {
            return Err(PublishedValueError::Invalid("depset metadata"));
        }
        let canonical = canonical_depset_origin(&original, &mut visits)?;
        let key = (actual.pointer(), original.identity());
        if seen.contains_key(&key) {
            continue;
        }
        charge_depset_validation(
            &mut visits,
            &mut scratch,
            std::mem::size_of_val(&key)
                .saturating_add(std::mem::size_of::<(AnalysisDepset, PublishedValueRef)>())
                .saturating_mul(3),
        )?;
        seen.insert(key, (actual.clone(), original));
        let PublishedNode::Depset(row) = canonical.node() else {
            return Err(PublishedValueError::Invalid("depset row"));
        };
        let mut expected = row.successors.iter();
        for successor in actual.successors() {
            charge_depset_validation(&mut visits, &mut scratch, 0)?;
            match (successor, expected.next()) {
                (
                    AnalysisDepsetSuccessor::Direct(value),
                    Some(PublishedDepsetInput::Direct(edge)),
                ) => {
                    child(value, &canonical, *edge)?;
                }
                (
                    AnalysisDepsetSuccessor::Transitive(value),
                    Some(PublishedDepsetInput::Transitive(edge)),
                ) => {
                    charge_depset_validation(
                        &mut visits,
                        &mut scratch,
                        std::mem::size_of::<(AnalysisDepset, PublishedValueRef)>() * 2,
                    )?;
                    pending.push((value, canonical.arena().resolve(*edge)?));
                }
                _ => return Err(PublishedValueError::Invalid("depset successors")),
            }
        }
        if expected.next().is_some() {
            return Err(PublishedValueError::Invalid("depset successors"));
        }
    }
    Ok(())
}

fn charge_depset_validation(
    visits: &mut usize,
    bytes: &mut usize,
    additional_bytes: usize,
) -> Result<(), PublishedValueError> {
    *visits = visits
        .checked_add(1)
        .filter(|n| *n <= NODE_LIMIT)
        .ok_or(PublishedValueError::Limit("depset validation visits"))?;
    *bytes = bytes
        .checked_add(additional_bytes)
        .filter(|n| *n <= BYTE_LIMIT)
        .ok_or(PublishedValueError::Limit("depset validation scratch"))?;
    Ok(())
}

/// Peel exact canonical-row rewraps while keeping every child arena context.
fn canonical_depset_origin(
    origin: &PublishedValueRef,
    visits: &mut usize,
) -> Result<PublishedValueRef, PublishedValueError> {
    let mut current = origin.clone();
    loop {
        *visits = visits
            .checked_add(1)
            .filter(|n| *n <= NODE_LIMIT)
            .ok_or(PublishedValueError::Limit("depset validation visits"))?;
        let PublishedNode::Depset(row) = current.node() else {
            return Err(PublishedValueError::Invalid("depset row"));
        };
        let [PublishedDepsetInput::Transitive(edge)] = &*row.successors else {
            return Ok(current);
        };
        let child = current.arena().resolve(*edge)?;
        let PublishedNode::Depset(child_row) = child.node() else {
            return Err(PublishedValueError::Invalid("depset row"));
        };
        if row.order == child_row.order {
            return Ok(current);
        }
        if !row.order.compatible_with(child_row.order)
            || row.depth > crate::depset::MAX_DEPTH
            || row.depth != child_row.depth
            || row.element_type != child_row.element_type
        {
            return Err(PublishedValueError::Invalid("dereferenced depset metadata"));
        }
        current = child;
    }
}

pub(super) fn validate_variables(
    v: &CcToolchainVariables,
    o: &PublishedValueRef,
    depth: usize,
) -> Result<(), PublishedValueError> {
    if depth > DEPTH_LIMIT {
        return Err(PublishedValueError::Limit("projection depth"));
    }
    let PublishedNode::CcVariables(r) = o.node() else {
        return Err(PublishedValueError::Invalid("variables row"));
    };
    let m = v.model();
    if m.kind != r.kind
        || &m.occurrence != &r.occurrence
        || m.locals.len() != r.locals.len()
        || m.cache.len() != r.seed.len()
    {
        return Err(PublishedValueError::Invalid("variables metadata"));
    }
    match (&m.parent, r.parent) {
        (None, None) => {}
        (Some(p), Some(e)) => validate_variables(p, &o.arena().resolve(e)?, depth + 1)?,
        _ => return Err(PublishedValueError::Invalid("variables parent")),
    }
    for ((n, v), (rn, e)) in m.locals.iter().zip(r.locals.iter()) {
        if n != rn {
            return Err(PublishedValueError::Invalid("variables local"));
        }
        child(v, o, *e)?;
    }
    for ((n, v), (rn, s)) in m.cache.iter().zip(r.seed.iter()) {
        if n != rn {
            return Err(PublishedValueError::Invalid("variables cache"));
        }
        match (v, s) {
            (CcVariableCacheEntry::Value(v), PublishedVariableSeed::Value(e)) => child(v, o, *e)?,
            (CcVariableCacheEntry::Sequence(v), PublishedVariableSeed::Sequence(r)) => {
                children(v, o, *r)?
            }
            (CcVariableCacheEntry::Missing, PublishedVariableSeed::Missing) => {}
            (CcVariableCacheEntry::Error(a), PublishedVariableSeed::Error(b)) if a == b => {}
            _ => return Err(PublishedValueError::Invalid("variables cache entry")),
        }
    }
    Ok(())
}
