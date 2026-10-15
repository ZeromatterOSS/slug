//! Stable semantic selection over the same complete shared root row.
use super::*;
use crate::bzl_module::semantic_evaluation::budget::peak_context::FirstLedger;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakContext;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakOwner;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakPhase;

#[derive(Debug, Allocative)]
pub(super) struct BzlStableOwner {
    root: Arc<LocalBzlInput>,
    shared: [SharedPublicationInput; 1],
}
impl BzlStableOwner {
    pub(super) fn root(&self) -> &Arc<LocalBzlInput> {
        &self.root
    }
    pub(super) fn shape() -> PublicationShape {
        PublicationShape {
            visits: 3, // One wrapper record and both stored reference facts.
            ..fact(std::mem::size_of::<Self>(), 2, 1)
        }
    }
    pub(super) fn construction_shape() -> PublicationShape {
        let mut shape = Self::shape();
        // The inline singleton is already in sizeof(Self). The allocation's
        // Arc controls and both actual handle-clone operations are extra work.
        shape.bytes += 2 * std::mem::size_of::<usize>();
        shape.visits += 2;
        shape
    }
    pub(super) fn new(
        root: &Arc<LocalBzlInput>,
        retained: PublicationShape,
        work: &mut PublicationShape,
    ) -> anyhow::Result<Arc<Self>> {
        let mut complete = retained;
        add_shape(&mut complete, Self::shape(), 0)
            .map_err(|error| anyhow::anyhow!("combined stable owner and flat validity: {error}"))?;
        add_shape(work, Self::construction_shape(), 0)?;
        // Both final retained representations and all live constructor work
        // are admitted before either root guard clone or wrapper allocation.
        crate::bzl_module::semantic_evaluation::budget::admit_peak(
            PeakContext::new(
                PeakOwner::StableOwner,
                PeakPhase::StablePrecopy,
                FirstLedger::NativeRetained,
            ),
            crate::bzl_module::semantic_evaluation::budget::Budget {
                records: complete.visits,
                bytes: complete.bytes,
                depth: complete.depth,
            },
            crate::bzl_module::semantic_evaluation::budget::Budget {
                records: work.visits,
                bytes: work.bytes,
                depth: work.depth,
            },
            0,
        )
        .map_err(anyhow::Error::msg)?;
        Ok(Arc::new(Self {
            root: root.clone(),
            shared: [SharedPublicationInput::new(root.clone())],
        }))
    }
}
impl StructuralFunctionOwnerPart for BzlStableOwner {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|other| self.root.key == other.root.key)
    }
    fn hash_part(&self, mut state: &mut dyn Hasher) {
        self.root.key.hash(&mut state);
    }
    fn retained_bytes(&self) -> usize {
        Self::shape().bytes
    }
    fn retained_shape(&self) -> PublicationShape {
        Self::shape()
    }
    fn shared_inputs(&self) -> &[SharedPublicationInput] {
        &self.shared
    }
}
