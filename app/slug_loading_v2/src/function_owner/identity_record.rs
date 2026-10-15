//! One complete fixed identity prefix, then independent immutable mapping records.
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;

use super::*;

fn prefix(identity: &BzlModuleIdentity) -> anyhow::Result<PublicationShape> {
    let mapping_id = identity.label.mapping_id();
    let bytes = (std::mem::size_of::<BzlModuleIdentity>() + 2 * std::mem::size_of::<usize>())
        .saturating_add(identity.label.package().repo().as_str().len())
        .saturating_add(identity.label.package().package().as_str().len())
        .saturating_add(identity.label.target().as_str().len())
        .saturating_add(mapping_id.map_or(0, |mapping| mapping.as_str().len()))
        .saturating_add(
            identity
                .source
                .retained_text_bytes()
                .unwrap_or(usize::MAX)
                .saturating_mul(2),
        );
    // Preserve every former field's storage/edge/depth forecast. Only the paid
    // work unit changes: the bounded fixed prefix is inspected as one record.
    let shape = fact(bytes, 5 + usize::from(mapping_id.is_some()), 2);
    check_shape(shape)?;
    Ok(shape)
}

fn append_entry(
    shape: &mut PublicationShape,
    apparent: &ApparentRepoName,
    canonical: &CanonicalRepoName,
) -> anyhow::Result<()> {
    let bytes = std::mem::size_of::<(ApparentRepoName, CanonicalRepoName)>()
        .saturating_add(apparent.as_str().len())
        .saturating_add(canonical.as_str().len());
    // A dynamic entry is separately checked before the next entry is touched.
    // The pair and both strings remain complete; the relative depth is three.
    add_shape(shape, fact(bytes, 2, 3), 0)
}

pub(super) fn shape(identity: &BzlModuleIdentity) -> anyhow::Result<PublicationShape> {
    let mut shape = prefix(identity)?;
    for (apparent, canonical) in &*identity.repository_mapping {
        append_entry(&mut shape, apparent, canonical)?;
    }
    Ok(shape)
}

#[cfg(test)]
#[path = "identity_record_tests.rs"]
mod tests;
