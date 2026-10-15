//! Fixed scalar context for an existing owner-edge admission refusal.
use super::*;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PublicationEdgeContext {
    Local,
    ImportBody,
    OwnerBody,
    SharedBody,
}

/// Admission ledger values, not a physical-graph decomposition. Body fields
/// are explanatory subsets of owner edges, never extra terms in joint totals.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PublicationEdgeRefusal {
    context: PublicationEdgeContext,
    used_owner_edges: usize,
    requested_edges: usize,
    limit_edges: usize,
    checked_owner_total: Option<usize>,
    direct_edges: usize,
    inline_edges: usize,
    imported_edges: usize,
    checked_joint_before: Option<usize>,
    checked_joint_total: Option<usize>,
    prior_poisoned: bool,
    owner_body_edges: usize,
    shared_body_edges: usize,
}
impl PublicationEdgeRefusal {
    pub fn context(&self) -> PublicationEdgeContext {
        self.context
    }
    pub fn used_owner_edges(&self) -> usize {
        self.used_owner_edges
    }
    pub fn requested_edges(&self) -> usize {
        self.requested_edges
    }
    pub fn limit_edges(&self) -> usize {
        self.limit_edges
    }
    pub fn checked_owner_total(&self) -> Option<usize> {
        self.checked_owner_total
    }
    pub fn direct_edges(&self) -> usize {
        self.direct_edges
    }
    pub fn inline_edges(&self) -> usize {
        self.inline_edges
    }
    pub fn imported_edges(&self) -> usize {
        self.imported_edges
    }
    pub fn checked_joint_before(&self) -> Option<usize> {
        self.checked_joint_before
    }
    pub fn checked_joint_total(&self) -> Option<usize> {
        self.checked_joint_total
    }
    pub fn prior_poisoned(&self) -> bool {
        self.prior_poisoned
    }
    pub fn owner_body_edges(&self) -> usize {
        self.owner_body_edges
    }
    pub fn shared_body_edges(&self) -> usize {
        self.shared_body_edges
    }
}
impl PublishedValueArenaBuilder {
    // Only called after the unchanged refusal arm has poisoned this builder.
    pub(super) fn edge_refusal(
        &self,
        requested: usize,
        prior_poisoned: bool,
    ) -> PublishedValueError {
        let joint = |owner: usize| {
            owner
                .checked_add(self.edges.len())
                .and_then(|n| n.checked_add(self.inline_edges))
                .and_then(|n| n.checked_add(self.imported_edges))
        };
        let checked_owner_total = self.owner_edges.checked_add(requested);
        PublishedValueError::EdgeBudget(Box::new(PublicationEdgeRefusal {
            context: PublicationEdgeContext::Local,
            used_owner_edges: self.owner_edges,
            requested_edges: requested,
            limit_edges: EDGE_LIMIT,
            checked_owner_total,
            direct_edges: self.edges.len(),
            inline_edges: self.inline_edges,
            imported_edges: self.imported_edges,
            checked_joint_before: joint(self.owner_edges),
            checked_joint_total: checked_owner_total.and_then(joint),
            prior_poisoned,
            owner_body_edges: self.owner_payload.edges,
            shared_body_edges: self.shared_payload.edges,
        }))
    }
}
impl PublishedValueError {
    pub(super) fn with_edge_context(mut self, context: PublicationEdgeContext) -> Self {
        if let Self::EdgeBudget(refusal) = &mut self {
            if refusal.context == PublicationEdgeContext::Local {
                refusal.context = context;
            }
        }
        self
    }
}
#[cfg(test)]
#[path = "edge_refusal/tests.rs"]
mod tests;
