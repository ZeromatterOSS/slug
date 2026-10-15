//! Native-only immutable closure metadata; semantic/event budgets stay separate.
use slug_build_api_v2::published_value::PublicationShape;
use slug_build_api_v2::published_value::SharedPublicationInput;

use super::*;

#[derive(Debug, Clone, Allocative)]
pub(crate) struct PublicationMetadata {
    intrinsic: PublicationShape,
    full: PublicationShape,
    children: Arc<[SharedPublicationInput]>,
}
/// Borrowed preflight result, consumed only after its caller's joint admission.
pub(crate) struct PreparedPublication {
    intrinsic: PublicationShape,
    full: PublicationShape,
    storage: PublicationShape,
    count: usize,
}
fn overflow() -> String {
    "native publication metadata overflow".into()
}
fn checked_add(
    a: &mut PublicationShape,
    b: PublicationShape,
    nesting: usize,
) -> Result<(), String> {
    a.nodes = a.nodes.checked_add(b.nodes).ok_or_else(overflow)?;
    a.edges = a.edges.checked_add(b.edges).ok_or_else(overflow)?;
    a.bytes = a.bytes.checked_add(b.bytes).ok_or_else(overflow)?;
    a.visits = a.visits.checked_add(b.visits).ok_or_else(overflow)?;
    a.depth = a
        .depth
        .max(b.depth.checked_add(nesting).ok_or_else(overflow)?);
    if a.depth > crate::bzl_module::semantic_evaluation::budget::DEPTH_LIMIT {
        return Err(overflow());
    }
    Ok(())
}
pub(crate) fn declaration_storage(count: usize) -> Result<PublicationShape, String> {
    Ok(PublicationShape {
        nodes: 0,
        edges: count,
        visits: count.checked_add(1).ok_or_else(overflow)?,
        bytes: count
            .checked_mul(std::mem::size_of::<SharedPublicationInput>())
            .and_then(|n| n.checked_add(2 * std::mem::size_of::<usize>()))
            .ok_or_else(overflow)?,
        depth: usize::from(count != 0),
    })
}
impl PreparedPublication {
    /// Every next()/cached-shape read is prepaid; expanded full facts are not
    /// replayed into producer work or its actual physical-retention budget.
    pub(crate) fn prepare(
        mut intrinsic: PublicationShape,
        count: usize,
        mut shapes: impl Iterator<Item = PublicationShape>,
        mut charge: impl FnMut(usize, usize) -> Result<(), String>,
    ) -> Result<Self, String> {
        charge(1, std::mem::size_of::<Self>())?;
        let storage = declaration_storage(count)?;
        checked_add(&mut intrinsic, storage, 0)?;
        // Final Arc plus old/new Vec transfer and all later guard copies.
        charge(
            count
                .checked_mul(4)
                .ok_or_else(overflow)?
                .checked_add(1)
                .ok_or_else(overflow)?,
            storage.bytes.checked_mul(5).ok_or_else(overflow)?,
        )?;
        let mut full = intrinsic;
        for _ in 0..count {
            charge(2, std::mem::size_of::<PublicationShape>())?;
            let child = shapes.next().ok_or_else(overflow)?;
            checked_add(&mut full, child, 1)?;
        }
        charge(1, 0)?;
        if shapes.next().is_some() {
            return Err(overflow());
        }
        Ok(Self {
            intrinsic,
            full,
            storage,
            count,
        })
    }
    pub(crate) fn storage(&self) -> PublicationShape {
        self.storage
    }
    pub(crate) fn full(&self) -> PublicationShape {
        self.full
    }
    pub(crate) fn finish(
        self,
        children: Vec<SharedPublicationInput>,
    ) -> Result<PublicationMetadata, String> {
        if children.len() != self.count {
            return Err(overflow());
        }
        Ok(PublicationMetadata {
            intrinsic: self.intrinsic,
            full: self.full,
            children: children.into(),
        })
    }
}
impl PublicationMetadata {
    pub(crate) fn intrinsic(&self) -> PublicationShape {
        self.intrinsic
    }
    pub(crate) fn full(&self) -> PublicationShape {
        self.full
    }
    pub(crate) fn children(&self) -> &[SharedPublicationInput] {
        &self.children
    }
}

#[cfg(test)]
impl PublicationMetadata {
    // Isolated candidate-index fixtures replace actual key guards while keeping
    // every semantic field/body width. This is not a producer admission API.
    pub(crate) fn with_test_children(&self, children: Vec<SharedPublicationInput>) -> Self {
        assert_eq!(children.len(), self.children.len());
        let mut full = self.intrinsic;
        for child in &children {
            checked_add(&mut full, child.retained_shape(), 1).unwrap();
        }
        Self {
            intrinsic: self.intrinsic,
            full,
            children: children.into(),
        }
    }
}

#[cfg(test)]
#[path = "publication/tests.rs"]
mod tests;
