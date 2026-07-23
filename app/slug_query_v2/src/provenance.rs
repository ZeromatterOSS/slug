/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Request-local real/fake query candidate identity and set algebra.
//!
//! This module deliberately does not activate any query function. It preserves
//! candidate provenance until an operation's Bazel label-keyed materialization
//! boundary, while keeping fake candidates out of the real evaluation graph.

#![allow(dead_code)] // Gate A substrate remains disconnected until Gate B activation.

use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::graph::QueryLabel;

/// Full request-local candidate identity.
///
/// Equality and hashing are ordinary, symmetric, and include real/fake kind
/// plus the fake candidate's consuming package. Printed-label uniqueness is a
/// separate operation performed only at the required materialization boundary.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub(crate) enum QueryCandidate {
    Real(QueryLabel),
    Fake {
        printed_label: QueryLabel,
        consuming_package: CompactString,
    },
}

impl QueryCandidate {
    pub(crate) fn real(label: QueryLabel) -> Self {
        Self::Real(label)
    }

    pub(crate) fn fake(
        printed_label: QueryLabel,
        consuming_package: impl Into<CompactString>,
    ) -> Self {
        Self::Fake {
            printed_label,
            consuming_package: consuming_package.into(),
        }
    }

    pub(crate) fn printed_label(&self) -> &QueryLabel {
        match self {
            Self::Real(label)
            | Self::Fake {
                printed_label: label,
                ..
            } => label,
        }
    }

    /// Return the label eligible for request-local evaluation-edge recording.
    ///
    /// `None` for a fake candidate means "record no graph node or edge", not
    /// "do not print it". Fake candidates remain selected/renderable after
    /// downstream consumption and will have an empty dependency set.
    pub(crate) fn evaluation_graph_label(&self) -> Option<&QueryLabel> {
        match self {
            Self::Real(label) => Some(label),
            Self::Fake { .. } => None,
        }
    }

    fn sibling_package(&self) -> CompactString {
        match self {
            Self::Real(label) => CompactString::new(label.package()),
            Self::Fake {
                consuming_package, ..
            } => consuming_package.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Allocative)]
pub(crate) struct QueryCandidateId(u32);

/// Compact request-local arena. Candidates are stored once without one `Arc`
/// allocation per identity.
#[derive(Debug, Default, Allocative)]
pub(crate) struct QueryCandidateArena {
    candidates: Vec<QueryCandidate>,
    candidate_to_id: SmallMap<QueryCandidate, QueryCandidateId>,
}

impl QueryCandidateArena {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn intern(&mut self, candidate: QueryCandidate) -> QueryCandidateId {
        if let Some(id) = self.candidate_to_id.get(&candidate) {
            return *id;
        }
        let id = QueryCandidateId(
            self.candidates
                .len()
                .try_into()
                .expect("query candidate arena exceeds u32 candidate capacity"),
        );
        self.candidate_to_id.insert(candidate.clone(), id);
        self.candidates.push(candidate);
        id
    }

    pub(crate) fn get(&self, id: QueryCandidateId) -> &QueryCandidate {
        &self.candidates[id.0 as usize]
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.candidates.len()
    }
}

/// One non-empty callback delivery, label-materialized within that delivery.
#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub(crate) struct QueryCandidateBatch {
    ids: Arc<[QueryCandidateId]>,
}

impl QueryCandidateBatch {
    fn from_candidates(
        arena: &mut QueryCandidateArena,
        candidates: impl IntoIterator<Item = QueryCandidate>,
    ) -> Option<Self> {
        let mut seen_labels = SmallSet::new();
        let mut ids = Vec::new();
        for candidate in candidates {
            if seen_labels.insert(candidate.printed_label().dupe()) {
                ids.push(arena.intern(candidate));
            }
        }
        Self::from_ids(ids)
    }

    fn from_ids(ids: Vec<QueryCandidateId>) -> Option<Self> {
        (!ids.is_empty()).then(|| Self { ids: ids.into() })
    }

    pub(crate) fn ids(&self) -> &[QueryCandidateId] {
        &self.ids
    }
}

/// Ordered callback deliveries. Distinct batches retain distinct provenance
/// until a downstream operation explicitly materializes by printed label.
#[derive(Debug, Clone, Default, Eq, PartialEq, Allocative)]
pub(crate) struct QueryCandidateBatches {
    batches: Vec<QueryCandidateBatch>,
}

impl QueryCandidateBatches {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn from_delivery(
        arena: &mut QueryCandidateArena,
        candidates: impl IntoIterator<Item = QueryCandidate>,
    ) -> Self {
        let batches = QueryCandidateBatch::from_candidates(arena, candidates)
            .into_iter()
            .collect();
        Self { batches }
    }

    pub(crate) fn batches(&self) -> &[QueryCandidateBatch] {
        &self.batches
    }

    /// Concatenate callback deliveries without cross-batch label collapse.
    pub(crate) fn union(mut self, other: Self) -> Self {
        self.batches.extend(other.batches);
        self
    }

    /// Materialize all deliveries by printed label, retaining the first full
    /// candidate representative.
    pub(crate) fn eval_all(&self, arena: &QueryCandidateArena) -> Option<QueryCandidateBatch> {
        QueryCandidateBatch::from_ids(
            self.materialized_by_label(arena)
                .into_iter()
                .map(|(_, id)| id)
                .collect(),
        )
    }

    /// Materialize both sides and retain each left representative whose
    /// printed label occurs on the right.
    pub(crate) fn intersection(&self, arena: &QueryCandidateArena, other: &Self) -> Self {
        let right = other
            .materialized_by_label(arena)
            .into_iter()
            .map(|(label, _)| label)
            .collect::<SmallSet<_>>();
        Self::from_materialized_ids(
            self.materialized_by_label(arena)
                .into_iter()
                .filter_map(|(label, id)| right.contains(&label).then_some(id))
                .collect(),
        )
    }

    /// Materialize both sides and remove all left representatives with a
    /// matching right printed label, independent of real/fake provenance.
    pub(crate) fn except(&self, arena: &QueryCandidateArena, other: &Self) -> Self {
        let right = other
            .materialized_by_label(arena)
            .into_iter()
            .map(|(label, _)| label)
            .collect::<SmallSet<_>>();
        Self::from_materialized_ids(
            self.materialized_by_label(arena)
                .into_iter()
                .filter_map(|(label, id)| (!right.contains(&label)).then_some(id))
                .collect(),
        )
    }

    /// Visit every candidate in every delivery before package deduplication.
    pub(crate) fn sibling_packages(&self, arena: &QueryCandidateArena) -> Arc<[CompactString]> {
        let mut packages = SmallSet::new();
        for batch in &self.batches {
            for id in batch.ids.iter().copied() {
                packages.insert(arena.get(id).sibling_package());
            }
        }
        packages.into_iter().collect::<Vec<_>>().into()
    }

    /// Collapse to printable labels only after downstream provenance-sensitive
    /// consumers have run. The first representative for each label wins.
    pub(crate) fn unique_output_labels(&self, arena: &QueryCandidateArena) -> Arc<[QueryLabel]> {
        self.materialized_by_label(arena)
            .into_iter()
            .map(|(label, _)| label)
            .collect::<Vec<_>>()
            .into()
    }

    fn from_materialized_ids(ids: Vec<QueryCandidateId>) -> Self {
        Self {
            batches: QueryCandidateBatch::from_ids(ids).into_iter().collect(),
        }
    }

    fn materialized_by_label(
        &self,
        arena: &QueryCandidateArena,
    ) -> SmallMap<QueryLabel, QueryCandidateId> {
        let mut materialized = SmallMap::new();
        for batch in &self.batches {
            for id in batch.ids.iter().copied() {
                let label = arena.get(id).printed_label();
                if materialized.get(label).is_none() {
                    materialized.insert(label.dupe(), id);
                }
            }
        }
        materialized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(value: &str) -> QueryLabel {
        QueryLabel::parse_root(value).unwrap()
    }

    fn real(value: &str) -> QueryCandidate {
        QueryCandidate::real(label(value))
    }

    fn fake(value: &str, consuming_package: &str) -> QueryCandidate {
        QueryCandidate::fake(label(value), consuming_package)
    }

    fn only_candidate<'a>(
        batches: &QueryCandidateBatches,
        arena: &'a QueryCandidateArena,
    ) -> &'a QueryCandidate {
        assert_eq!(batches.batches().len(), 1);
        assert_eq!(batches.batches()[0].ids().len(), 1);
        arena.get(batches.batches()[0].ids()[0])
    }

    #[test]
    fn full_identity_and_arena_interning_are_stable_and_symmetric() {
        let printed = label("//shared:one.bzl");
        let fake_a = QueryCandidate::fake(printed.dupe(), "a");
        let fake_b = QueryCandidate::fake(printed.dupe(), "b");
        let real = QueryCandidate::real(printed);
        assert_eq!(fake_a, fake_a.clone());
        assert_ne!(fake_a, fake_b);
        assert_ne!(fake_b, fake_a);
        assert_ne!(fake_a, real);
        assert_ne!(real, fake_a);

        let mut arena = QueryCandidateArena::new();
        let fake_a_id = arena.intern(fake_a.clone());
        assert_eq!(fake_a_id, arena.intern(fake_a));
        assert_ne!(fake_a_id, arena.intern(fake_b));
        assert_ne!(fake_a_id, arena.intern(real));
        assert_eq!(arena.len(), 3);
    }

    #[test]
    fn one_delivery_keeps_the_first_representative_per_printed_label() {
        let mut arena = QueryCandidateArena::new();
        let batches = QueryCandidateBatches::from_delivery(
            &mut arena,
            [fake("//shared:one.bzl", "a"), fake("//shared:one.bzl", "b")],
        );
        assert_eq!(
            only_candidate(&batches, &arena),
            &fake("//shared:one.bzl", "a")
        );
    }

    #[test]
    fn fake_union_preserves_batches_and_both_consuming_packages_in_both_orders() {
        let mut arena = QueryCandidateArena::new();
        let a = QueryCandidateBatches::from_delivery(&mut arena, [fake("//shared:one.bzl", "a")]);
        let b = QueryCandidateBatches::from_delivery(&mut arena, [fake("//shared:one.bzl", "b")]);

        let ab = a.clone().union(b.clone());
        assert_eq!(ab.batches().len(), 2);
        assert_eq!(
            ab.sibling_packages(&arena).as_ref(),
            &[CompactString::new("a"), CompactString::new("b")]
        );
        let ba = b.union(a);
        assert_eq!(ba.batches().len(), 2);
        assert_eq!(
            ba.sibling_packages(&arena).as_ref(),
            &[CompactString::new("b"), CompactString::new("a")]
        );
    }

    #[test]
    fn real_fake_union_retains_both_packages_then_one_output_label() {
        let mut arena = QueryCandidateArena::new();
        let real_batches =
            QueryCandidateBatches::from_delivery(&mut arena, [real("//shared:one.bzl")]);
        let fake = QueryCandidateBatches::from_delivery(
            &mut arena,
            [fake("//shared:one.bzl", "consumer")],
        );
        let union = real_batches.union(fake);
        assert_eq!(union.batches().len(), 2);
        assert_eq!(
            union.sibling_packages(&arena).as_ref(),
            &[CompactString::new("shared"), CompactString::new("consumer")]
        );
        assert_eq!(
            union.unique_output_labels(&arena).as_ref(),
            &[label("//shared:one.bzl")]
        );
    }

    #[test]
    fn intersection_preserves_the_left_representative_in_all_provenance_orders() {
        let mut arena = QueryCandidateArena::new();
        let real_batches =
            QueryCandidateBatches::from_delivery(&mut arena, [real("//shared:one.bzl")]);
        let fake_a =
            QueryCandidateBatches::from_delivery(&mut arena, [fake("//shared:one.bzl", "a")]);
        let fake_b =
            QueryCandidateBatches::from_delivery(&mut arena, [fake("//shared:one.bzl", "b")]);

        assert_eq!(
            only_candidate(&fake_a.intersection(&arena, &real_batches), &arena),
            &fake("//shared:one.bzl", "a")
        );
        assert_eq!(
            only_candidate(&real_batches.intersection(&arena, &fake_a), &arena),
            &real("//shared:one.bzl")
        );
        assert_eq!(
            only_candidate(&fake_a.intersection(&arena, &fake_b), &arena),
            &fake("//shared:one.bzl", "a")
        );
        assert_eq!(
            only_candidate(&fake_b.intersection(&arena, &fake_a), &arena),
            &fake("//shared:one.bzl", "b")
        );
    }

    #[test]
    fn except_removes_equal_printed_labels_symmetrically() {
        let mut arena = QueryCandidateArena::new();
        let real = QueryCandidateBatches::from_delivery(&mut arena, [real("//shared:one.bzl")]);
        let fake_a =
            QueryCandidateBatches::from_delivery(&mut arena, [fake("//shared:one.bzl", "a")]);
        let fake_b =
            QueryCandidateBatches::from_delivery(&mut arena, [fake("//shared:one.bzl", "b")]);

        for result in [
            fake_a.except(&arena, &real),
            real.except(&arena, &fake_a),
            fake_a.except(&arena, &fake_b),
            fake_b.except(&arena, &fake_a),
        ] {
            assert!(result.batches().is_empty());
        }
    }

    #[test]
    fn except_removes_only_matching_labels_from_a_multi_label_fake_set() {
        let mut arena = QueryCandidateArena::new();
        let fake_set = QueryCandidateBatches::from_delivery(
            &mut arena,
            [
                fake("//shared:one.bzl", "consumer"),
                fake("//shared:two.bzl", "consumer"),
            ],
        );
        let real_one = QueryCandidateBatches::from_delivery(&mut arena, [real("//shared:one.bzl")]);
        assert_eq!(
            only_candidate(&fake_set.except(&arena, &real_one), &arena),
            &fake("//shared:two.bzl", "consumer")
        );
    }

    #[test]
    fn union_does_not_materialize_until_eval_all_is_requested() {
        let mut arena = QueryCandidateArena::new();
        let a = QueryCandidateBatches::from_delivery(&mut arena, [fake("//shared:one.bzl", "a")]);
        let b = QueryCandidateBatches::from_delivery(&mut arena, [fake("//shared:one.bzl", "b")]);
        let union = a.union(b);
        assert_eq!(union.batches().len(), 2);
        let materialized = union.eval_all(&arena).unwrap();
        assert_eq!(materialized.ids().len(), 1);
        assert_eq!(
            arena.get(materialized.ids()[0]),
            &fake("//shared:one.bzl", "a")
        );
    }

    #[test]
    fn empty_results_never_contain_an_empty_batch() {
        let mut arena = QueryCandidateArena::new();
        let empty = QueryCandidateBatches::from_delivery(&mut arena, []);
        assert!(empty.batches().is_empty());
        assert!(empty.eval_all(&arena).is_none());
        assert!(QueryCandidateBatches::empty().batches().is_empty());

        let one = QueryCandidateBatches::from_delivery(&mut arena, [real("//shared:one.bzl")]);
        let two = QueryCandidateBatches::from_delivery(&mut arena, [real("//shared:two.bzl")]);
        assert!(one.intersection(&arena, &two).batches().is_empty());
        assert!(one.except(&arena, &one).batches().is_empty());
    }

    #[test]
    fn graph_eligibility_is_separate_from_output_visibility() {
        let real = real("//shared:one.bzl");
        let fake = fake("//shared:one.bzl", "consumer");
        assert_eq!(
            real.evaluation_graph_label(),
            Some(&label("//shared:one.bzl"))
        );
        assert!(fake.evaluation_graph_label().is_none());
        assert_eq!(fake.printed_label(), &label("//shared:one.bzl"));
    }
}
