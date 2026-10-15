//! Consuming seal's local certificate loan and bounded failure attribution.
use retained_facts::CertificationPhase;

use super::*;

impl PublishedValueArenaBuilder {
    pub(super) fn certify_local(
        &mut self,
        originals: &dyn PublishedOriginalBinding,
        work: &mut usize,
    ) -> Result<
        (
            retained_facts::LocalRetainedFacts,
            Vec<CallableSourceAuthority>,
        ),
        PublishedValueError,
    > {
        let entry_work = *work;
        let mut phase = CertificationPhase::ViewPrepay;
        let local_result = (|| {
            retained_facts::prepare_checked_draft(
                work,
                &mut self.bytes,
                &mut self.producer_scratch_bytes,
                &mut self.failed_budget,
            )?;
            // append_edges validated every copied index against these exact
            // monotone domains. The consuming seal cannot mutate any of them
            // during this borrowed loan; no detached proof can be constructed.
            let draft = checked_draft::CheckedDraft {
                rows: &self.rows,
                edges: &self.edges,
                imports: &self.imports,
            };
            retained_facts::checked_local_visits(
                draft,
                &self.slots,
                self.owner_roots.len(),
                originals.root_count(),
                work,
                &mut self.bytes,
                &mut self.producer_scratch_bytes,
                &mut self.failed_budget,
                &mut phase,
            )
        })();
        local_result.map_err(|error| match error {
            PublishedValueError::Limit("retained certificate work") => {
                self.retained_certificate_work_refusal(entry_work, *work, phase)
            }
            other => other,
        })
    }
}
