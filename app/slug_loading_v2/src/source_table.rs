//! One original ordered raw table and its complete immutable publication guard.
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use slug_build_api_v2::published_value::ImmutablePublicationInput;
use slug_build_api_v2::published_value::PublicationShape;
use slug_build_api_v2::published_value::SharedPublicationInput;

use crate::BzlModuleSourceProvenance;
use crate::bzl_module::semantic_evaluation::budget::Budget;

pub(crate) mod inherited;
mod mapping;
#[path = "source_table/prepare.rs"]
pub(crate) mod prepare;
pub(crate) type Rows = Arc<[(CompactString, BzlModuleSourceProvenance)]>;

#[derive(Debug, Allocative)]
struct SourceIdentityTable {
    raw: Rows,
    publication: crate::bzl_environment::publication::PublicationMetadata,
    mappings: Arc<[mapping::MappingPair]>,
}
impl ImmutablePublicationInput for SourceIdentityTable {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn retained_shape(&self) -> PublicationShape {
        self.publication.full()
    }
    fn intrinsic_shape(&self) -> PublicationShape {
        self.publication.intrinsic()
    }
    fn shared_inputs(&self) -> &[SharedPublicationInput] {
        self.publication.children()
    }
}

/// Private fields bind the exact raw allocation to ONE successful descriptor.
#[derive(Debug, Clone, Allocative)]
pub(crate) struct CertifiedSourceTable {
    table: Arc<SourceIdentityTable>,
    declarations: [SharedPublicationInput; 1],
}
impl CertifiedSourceTable {
    pub(crate) fn raw(&self) -> &Rows {
        &self.table.raw
    }
    #[cfg(test)]
    pub(crate) fn descriptor_strong_count(&self) -> usize {
        Arc::strong_count(&self.table)
    }
    pub(crate) fn mapping_pairs(&self) -> &[mapping::MappingPair] {
        &self.table.mappings
    }
    pub(crate) fn declarations(&self) -> &[SharedPublicationInput] {
        &self.declarations
    }
    pub(crate) fn capture(&self, work: &mut Budget) -> Result<Self, String> {
        work.charge(4, std::mem::size_of::<Self>())?;
        Ok(self.clone())
    }
}
impl PartialEq for CertifiedSourceTable {
    fn eq(&self, other: &Self) -> bool {
        self.raw() == other.raw()
    }
}
impl Eq for CertifiedSourceTable {}
impl std::ops::Deref for CertifiedSourceTable {
    type Target = [(CompactString, BzlModuleSourceProvenance)];
    fn deref(&self) -> &Self::Target {
        self.raw()
    }
}

/// Read-only provenance remains available after a refused original preparation.
#[derive(Debug)]
pub(crate) enum SourceTableCapture {
    Complete(CertifiedSourceTable),
    Refused { raw: Rows, error: String },
}
impl SourceTableCapture {
    pub(crate) fn from_raw(raw: Rows, work: &mut Budget) -> Self {
        let result = (|| {
            work.charge(1, std::mem::size_of::<Rows>())?;
            let mut plan = prepare::Plan::new(raw.len(), work)?;
            for (name, source) in &*raw {
                plan.prefix(source, work)?;
                for (a, c) in &*source.identity.repository_mapping {
                    plan.mapping(a, c, work)?;
                }
                plan.filename(name, work)?;
            }
            work.charge(1, plan.shape().bytes)?;
            plan.prepay_finish(work)?;
            plan.finish(&raw)
        })();
        match result {
            Ok(table) => Self::Complete(table),
            Err(error) => Self::Refused { raw, error },
        }
    }
    pub(crate) fn raw(&self) -> &Rows {
        match self {
            Self::Complete(table) => table.raw(),
            Self::Refused { raw, .. } => raw,
        }
    }
    pub(crate) fn certified(&self, work: &mut Budget) -> Result<CertifiedSourceTable, String> {
        // Pay BEFORE reading the stored result, even on a refused preparation.
        work.charge(1, std::mem::size_of::<&Self>())?;
        match self {
            Self::Complete(table) => table.capture(work),
            Self::Refused { error, .. } => {
                work.charge(1, error.len())?;
                Err(error.clone())
            }
        }
    }
}

#[cfg(test)]
#[path = "source_table/tests.rs"]
mod tests;
