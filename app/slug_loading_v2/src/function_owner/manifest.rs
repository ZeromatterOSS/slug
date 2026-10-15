//! Validate expanded provenance once; retain its original heap footprint as scalars.
use slug_identity_v2::CanonicalLabel;

use super::*;

/// Positively forecast the actual attachment, not the sum of two alternatives.
/// Fixed inline overlap is conservative; every original typed payload stays full.
pub(super) fn original_storage(
    metadata: crate::starlark_label::PreparedDefiningModuleMetadata<'_>,
) -> anyhow::Result<(PublicationShape, PublicationShape)> {
    use crate::starlark_label::PreparedDefiningModuleMetadata;
    let manifest = metadata.raw_manifest();
    let mut shape = fact(PreparedDefiningModuleMetadata::header_bytes(), 4, 1);
    // Actual defining identity exists separately in both client-data layouts.
    add_shape(&mut shape, identity_shape(&manifest.root)?, 1)?;
    let mut work = shape;
    match metadata {
        PreparedDefiningModuleMetadata::RawManifest { .. } => {
            // One Arc manifest and BOTH Arc array control headers. Identity
            // records conservatively include inline element storage as before.
            let mut raw = fact(
                std::mem::size_of::<BzlLoadManifest>() + 6 * std::mem::size_of::<usize>(),
                0,
                1,
            );
            add_shape(&mut raw, identity_shape(&manifest.root)?, 1)?;
            for child in &*manifest.direct_children {
                add_shape(&mut raw, fact(0, 1, 0), 0)?;
                add_shape(&mut raw, identity_shape(child)?, 2)?;
            }
            for source in &*manifest.reachable {
                add_shape(&mut raw, fact(32, 1, 0), 0)?;
                add_shape(&mut raw, identity_shape(&source.identity)?, 2)?;
            }
            add_shape(&mut shape, raw, 0)?;
            add_shape(&mut work, raw, 0)?;
        }
        PreparedDefiningModuleMetadata::EffectiveContext(context) => {
            // The successful private producer certified the COMPLETE context,
            // including raw manifest, bindings, effective rows and source array.
            // Read that scalar certificate once; do not replay a raw tree walk.
            let context = context.retention();
            add_shape(
                &mut shape,
                PublicationShape {
                    nodes: 0,
                    edges: context.records,
                    bytes: context.bytes,
                    visits: 0,
                    depth: context.depth,
                },
                1,
            )?;
            add_shape(&mut work, fact(0, 0, 0), 0)?;
            prepay_effective_attachment_copy(&mut work)?;
        }
    }
    shape.visits = 0;
    Ok((shape, work))
}

/// One additional effective attachment clones the complete sealed source pair
/// and copies its producer Budget. Pay on the original owner ledger, before the
/// later metadata.attach/context.clone; the producer's finish/capture pays other
/// operations. No payload is read or guard cloned here.
pub(super) fn prepay_effective_attachment_copy(work: &mut PublicationShape) -> anyhow::Result<()> {
    // Two Arc increments, two inline handle transfers, three Budget scalar
    // copies and one enclosing Option/control transfer. Paying the whole pair
    // conservatively includes its already-existing raw-table handle too.
    add_shape(
        work,
        PublicationShape {
            visits: 8,
            bytes: std::mem::size_of::<crate::source_table::CertifiedSourceTable>()
                + std::mem::size_of::<crate::bzl_module::semantic_evaluation::budget::Budget>()
                + 2 * std::mem::size_of::<usize>(),
            ..zero_shape()
        },
        0,
    )
}

/// A legacy Context remains simultaneously live during owner construction even
/// though its payload is absent from the actual attached client data. Carry its
/// existing record/byte peak on the SAME constructor ledger before any clones.
pub(super) fn transient_peak(
    metadata: crate::starlark_label::PreparedDefiningModuleMetadata<'_>,
    work: &mut PublicationShape,
) -> anyhow::Result<()> {
    if let Some(context) = metadata.transient_context() {
        let context = context.retention();
        add_shape(
            work,
            PublicationShape {
                nodes: 0,
                edges: context.records,
                bytes: context.bytes,
                visits: 0,
                depth: context.depth,
            },
            1,
        )?;
    }
    Ok(())
}

/// Conservative insertion credit for the fixed BTree node types in this
/// module and the flat-row union. The first insertion allocates a multi-slot
/// node; key/value size alone is not an allocation forecast. Candidate Vec
/// growth also covers its complete next allocation before entry/push mutates.
pub(super) fn scratch_insertion(
    work: &mut PublicationShape,
    candidate_vec: Option<(usize, usize, usize)>, // len, capacity, element bytes
) -> anyhow::Result<()> {
    let candidate_bytes = candidate_vec.map_or(0, |(len, capacity, element)| {
        if len < capacity {
            0
        } else {
            len.saturating_add(1)
                .max(capacity.saturating_mul(2))
                .max(4)
                .saturating_mul(element)
        }
    });
    add_shape(
        work,
        fact(1024usize.saturating_add(candidate_bytes), 1, 1),
        0,
    )
}

fn label_shape(label: &CanonicalLabel) -> anyhow::Result<PublicationShape> {
    let mut shape = fact(std::mem::size_of::<CanonicalLabel>(), 0, 0);
    for text in [
        label.package().repo().as_str(),
        label.package().package().as_str(),
        label.target().as_str(),
    ] {
        add_shape(&mut shape, fact(text.len(), 0, 0), 0)?;
    }
    if let Some(mapping) = label.mapping_id() {
        add_shape(&mut shape, fact(mapping.as_str().len(), 1, 0), 0)?;
    }
    Ok(shape)
}
/// Hashes only index candidates. Complete label equality is charged explicitly
/// before every collision comparison; borrowed guards prevent allocation reuse.
#[derive(Default)]
struct Labels<'a>(BTreeMap<u64, Vec<&'a CanonicalLabel>>);
impl<'a> Labels<'a> {
    fn insert(
        &mut self,
        label: &'a CanonicalLabel,
        work: &mut PublicationShape,
    ) -> anyhow::Result<bool> {
        add_shape(work, label_shape(label)?, 0)?;
        let mut hash = DefaultHasher::new();
        label.hash(&mut hash);
        let hash = hash.finish();
        if let Some(candidates) = self.0.get(&hash) {
            for previous in candidates {
                add_shape(work, label_shape(previous)?, 0)?;
                add_shape(work, label_shape(label)?, 0)?;
                if *previous == label {
                    return Ok(false);
                }
            }
        }
        let (len, capacity) = self
            .0
            .get(&hash)
            .map_or((0, 0), |values| (values.len(), values.capacity()));
        scratch_insertion(
            work,
            Some((len, capacity, std::mem::size_of::<&CanonicalLabel>())),
        )?;
        self.0.entry(hash).or_default().push(label);
        Ok(true)
    }
}

fn identity_equal(
    actual: &BzlModuleIdentity,
    expected: &BzlModuleIdentity,
    work: &mut PublicationShape,
) -> anyhow::Result<bool> {
    add_shape(work, identity_shape(actual)?, 0)?;
    add_shape(work, identity_shape(expected)?, 0)?;
    Ok(actual == expected)
}

pub(super) fn validate(
    manifest: &BzlLoadManifest,
    children: &[Arc<[Arc<LocalBzlInput>]>],
    work: &mut PublicationShape,
) -> anyhow::Result<[u8; 32]> {
    let root = manifest
        .reachable
        .first()
        .ok_or_else(|| anyhow::anyhow!("Bzl manifest has no root provenance"))?;
    if !identity_equal(&root.identity, &manifest.root, work)? {
        anyhow::bail!("Bzl manifest root provenance identity differs from its root");
    }
    let mut direct_seen = Labels::default();
    let mut direct = manifest.direct_children.iter();
    for child in children {
        let row = child
            .first()
            .ok_or_else(|| anyhow::anyhow!("empty validated Bzl child closure"))?;
        if direct_seen.insert(&row.key.identity.label, work)? {
            let expected = direct
                .next()
                .ok_or_else(|| anyhow::anyhow!("Bzl manifest direct children are incomplete"))?;
            if !identity_equal(expected, &row.key.identity, work)? {
                anyhow::bail!("Bzl manifest direct child identity/order differs from actual loads");
            }
        }
    }
    if direct.next().is_some() {
        anyhow::bail!("Bzl manifest has extra direct children");
    }
    let mut reachable_seen = Labels::default();
    reachable_seen.insert(&manifest.root.label, work)?;
    let mut reachable = manifest.reachable.iter().skip(1);
    // Visit ALL raw direct child closures. Repeated labels may have additional
    // descendants; only individual provenance labels follow first-seen policy.
    for child in children {
        for row in child.iter() {
            add_shape(work, fact(std::mem::size_of::<usize>(), 1, 0), 0)?;
            if reachable_seen.insert(&row.key.identity.label, work)? {
                let expected = reachable.next().ok_or_else(|| {
                    anyhow::anyhow!("Bzl manifest reachable provenance is incomplete")
                })?;
                add_shape(work, fact(32, 0, 0), 0)?;
                if !identity_equal(&expected.identity, &row.key.identity, work)?
                    || expected.source_digest != row.source_digest
                {
                    anyhow::bail!(
                        "Bzl manifest reachable identity/digest/order differs from actual loads"
                    );
                }
            }
        }
    }
    if reachable.next().is_some() {
        anyhow::bail!("Bzl manifest has extra reachable provenance");
    }
    Ok(root.source_digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_label_index_refuses_first_node_allocation_before_growth() {
        let label = CanonicalLabel::parse("@@source+//package:file.bzl").unwrap();
        let mut labels = Labels::default();
        for available_after_hash in [1023, 1024] {
            let mut work = PublicationShape {
                bytes: 64 * 1024 * 1024 - label_shape(&label).unwrap().bytes - available_after_hash,
                ..zero_shape()
            };
            // The second case has enough for the tree credit alone, but not
            // the initial candidate Vec's multi-slot allocation.
            assert!(labels.insert(&label, &mut work).is_err());
            assert!(labels.0.is_empty());
        }
    }
}
