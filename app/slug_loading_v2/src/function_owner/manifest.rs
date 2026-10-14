//! Validate expanded provenance once; retain its original heap footprint as scalars.
use slug_identity_v2::CanonicalLabel;

use super::*;

/// Original module client data owns both an identity and an Arc manifest. The
/// expanded arrays remain in the original heap even though native semantic
/// rows no longer retain another full manifest. Preserve ALL bytes/edges here;
/// only their semantic traversal is replaced by validated local facts.
pub(super) fn original_storage(
    manifest: &BzlLoadManifest,
) -> anyhow::Result<(PublicationShape, PublicationShape)> {
    let mut shape = fact(
        std::mem::size_of::<BzlLoadManifest>()
            + std::mem::size_of::<Option<Arc<BzlLoadManifest>>>()
            + std::mem::size_of::<Option<FunctionEvaluationOwner>>()
            + 8 * std::mem::size_of::<usize>(),
        4,
        1,
    );
    add_shape(&mut shape, identity_shape(&manifest.root)?, 1)?;
    // DefiningBzlModuleContext.identity is separate from source_manifest.root.
    add_shape(&mut shape, identity_shape(&manifest.root)?, 1)?;
    for child in &*manifest.direct_children {
        add_shape(&mut shape, fact(0, 1, 0), 0)?;
        add_shape(&mut shape, identity_shape(child)?, 2)?;
    }
    for source in &*manifest.reachable {
        add_shape(&mut shape, fact(32, 1, 0), 0)?;
        add_shape(&mut shape, identity_shape(&source.identity)?, 2)?;
    }
    // This is one cached scalar retention certificate, not an expanded native
    // semantic value. Full producer validation work is independently bounded.
    let work = shape;
    shape.visits = 0;
    Ok((shape, work))
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
