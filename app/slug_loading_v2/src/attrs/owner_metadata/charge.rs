//! Typed accounting preserves retained forecasts while batching one entry walk.
use slug_build_api_v2::DepsetStorageCharge;

use super::Visitor;

// Prior consumer charged an edge slot plus a conservative allowance for every
// scalar callback. Batching retains every one of those byte and edge forecasts.
const RETAINED_ROW_ALLOWANCE: usize = 64 + std::mem::size_of::<usize>();

fn emit(raw: usize, rows: usize, visit: &mut Visitor<'_>) -> Result<(), String> {
    let allowance = RETAINED_ROW_ALLOWANCE
        .checked_mul(rows)
        .ok_or("rule owner metadata byte overflow")?;
    let bytes = raw
        .checked_add(allowance)
        .ok_or("rule owner metadata byte overflow")?;
    visit(DepsetStorageCharge {
        nodes: 0,
        edges: rows,
        bytes,
        visits: 2,
    })
}

pub(crate) fn normal(raw: usize, visit: &mut Visitor<'_>) -> Result<(), String> {
    emit(raw, 1, visit)
}

pub(crate) fn source_prefix(
    value: &crate::bzl_module::BzlModuleIdentity,
    visit: &mut Visitor<'_>,
) -> Result<(), String> {
    // One bounded record inspection covers precisely the former six scalar
    // rows, plus the optional label mapping provenance. Mapping entries follow
    // separately, preserving their complete source order and retention costs.
    let raw = [
        std::mem::size_of::<crate::bzl_module::BzlModuleIdentity>(),
        std::mem::size_of::<slug_identity_v2::CanonicalLabel>(),
        value.label.package().repo().as_str().len(),
        value.label.package().package().as_str().len(),
        value.label.target().as_str().len(),
        value
            .source
            .retained_text_bytes()
            .ok_or("rule owner metadata byte overflow")?,
    ]
    .into_iter()
    .try_fold(0usize, usize::checked_add)
    .ok_or("rule owner metadata byte overflow")?;
    let (raw, rows) = match value.label.mapping_id() {
        Some(mapping) => (
            raw.checked_add(mapping.as_str().len())
                .ok_or("rule owner metadata byte overflow")?,
            7,
        ),
        None => (raw, 6),
    };
    emit(raw, rows, visit)
}

pub(crate) fn mapping_entry(
    apparent: &str,
    canonical: &str,
    visit: &mut Visitor<'_>,
) -> Result<(), String> {
    // Inspect the complete actual entry in place. No memo, retained source
    // omission, name cloning or temporary serialized representation is needed.
    let raw = std::mem::size_of::<(
        slug_identity_v2::ApparentRepoName,
        slug_identity_v2::CanonicalRepoName,
    )>()
    .checked_add(apparent.len())
    .and_then(|raw| raw.checked_add(canonical.len()))
    .ok_or("rule owner metadata byte overflow")?;
    emit(raw, 3, visit)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forecast_overflow_refuses_before_the_callback() {
        let mut called = false;
        assert!(
            normal(usize::MAX, &mut |_| {
                called = true;
                Ok(())
            })
            .is_err()
        );
        assert!(!called);
    }
}
