//! Publication-only context work is cumulative, and never enters semantic data.
use super::*;
const CONTEXT_WORK: usize = 4;
fn prepay(work: &mut Budget) -> Result<(), String> {
    work.charge(
        CONTEXT_WORK,
        std::mem::size_of::<BzlEvaluationContext>()
            + std::mem::size_of::<SourceTableCapture>()
            + std::mem::size_of::<RefCell<Budget>>(),
    )
}
fn source_label(identity: &BzlModuleIdentity) -> CompactString {
    let canonical = identity.label.to_string();
    if identity.label.package().repo().is_root() {
        canonical
            .strip_prefix("@@")
            .expect("canonical root label")
            .into()
    } else {
        canonical.into()
    }
}
impl BzlEvaluationContext {
    pub(super) fn from_raw_sources(
        identity: BzlModuleIdentity,
        raw: crate::source_table::Rows,
    ) -> Self {
        let mut work = Budget::default();
        let source_table = match prepay(&mut work) {
            Ok(()) => SourceTableCapture::from_raw(raw, &mut work),
            Err(error) => SourceTableCapture::Refused { raw, error },
        };
        Self {
            source_label: source_label(&identity),
            source_identity: identity,
            source_table,
            publication_work: RefCell::new(work),
            bzl_load_visibility: RefCell::new(None),
        }
    }
    pub(super) fn from_effective_context(
        context: &crate::BzlModuleSourceContext,
    ) -> Result<Self, String> {
        let mut work = context.producer_work();
        prepay(&mut work)?;
        let table = context.certified_sources().capture(&mut work)?;
        let identity = context.raw_load_manifest().root.clone();
        Ok(Self {
            source_label: source_label(&identity),
            source_identity: identity,
            source_table: SourceTableCapture::Complete(table),
            publication_work: RefCell::new(work),
            bzl_load_visibility: RefCell::new(None),
        })
    }
    pub(crate) fn macro_runtime_context_with_table(
        identity: BzlModuleIdentity,
        table: &CertifiedSourceTable,
    ) -> Result<Self, String> {
        // A new invocation-local control phase, not a recertification or replay
        // of the original producer's work. The exact existing descriptor survives.
        let mut work = Budget::default();
        prepay(&mut work)?;
        let table = table.capture(&mut work)?;
        Ok(Self {
            source_label: source_label(&identity),
            source_identity: identity,
            source_table: SourceTableCapture::Complete(table),
            publication_work: RefCell::new(work),
            bzl_load_visibility: RefCell::new(None),
        })
    }
    pub(crate) fn source_table(&self) -> Result<CertifiedSourceTable, String> {
        let mut work = self.publication_work.borrow_mut();
        self.source_table.certified(&mut work)
        // RefMut is dropped before any Starlark/callback can reenter the context.
    }
}

#[cfg(test)]
#[path = "source_table_context/tests.rs"]
mod tests;
