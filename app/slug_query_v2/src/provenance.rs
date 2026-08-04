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
//! Candidate provenance is preserved until an operation's Bazel label-keyed
//! materialization boundary, while fake candidates remain outside the real
//! evaluation graph.

use std::hash::Hash;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackageIdentifier;
use slug_identity_v2::PackagePath;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::graph::QueryError;
use crate::graph::QueryLabel;

#[derive(Debug, Clone, Allocative, Dupe)]
pub(crate) struct QueryPackageIdentity(Arc<QueryPackageIdentityData>);

#[derive(Debug, Allocative)]
enum QueryPackageIdentityData {
    Root {
        package: PackagePath,
    },
    External {
        canonical_repo: CanonicalRepoName,
        apparent_repo: ApparentRepoName,
        package: PackagePath,
    },
}

impl QueryPackageIdentity {
    pub(crate) fn root(package: PackagePath) -> Self {
        Self(Arc::new(QueryPackageIdentityData::Root { package }))
    }

    pub(crate) fn external(
        canonical_repo: CanonicalRepoName,
        apparent_repo: ApparentRepoName,
        package: PackagePath,
    ) -> Result<Self, QueryError> {
        if canonical_repo.is_root() || apparent_repo.is_root() {
            return Err(QueryError::evaluation(
                "external query package identity requires nonroot canonical and apparent repositories",
            ));
        }
        Ok(Self(Arc::new(QueryPackageIdentityData::External {
            canonical_repo,
            apparent_repo,
            package,
        })))
    }

    pub(crate) fn package(&self) -> &PackagePath {
        match self.0.as_ref() {
            QueryPackageIdentityData::Root { package }
            | QueryPackageIdentityData::External { package, .. } => package,
        }
    }

    pub(crate) fn canonical_repo(&self) -> Option<&CanonicalRepoName> {
        match self.0.as_ref() {
            QueryPackageIdentityData::Root { .. } => None,
            QueryPackageIdentityData::External { canonical_repo, .. } => Some(canonical_repo),
        }
    }

    pub(crate) fn apparent_repo(&self) -> Option<&ApparentRepoName> {
        match self.0.as_ref() {
            QueryPackageIdentityData::Root { .. } => None,
            QueryPackageIdentityData::External { apparent_repo, .. } => Some(apparent_repo),
        }
    }

    pub(crate) fn canonical_package(&self) -> PackageIdentifier {
        PackageIdentifier::new(
            self.canonical_repo()
                .cloned()
                .unwrap_or_else(CanonicalRepoName::root),
            self.package().clone(),
        )
    }

    fn canonical_repo_str(&self) -> &str {
        self.canonical_repo()
            .map(CanonicalRepoName::as_str)
            .unwrap_or("")
    }
}

impl PartialEq for QueryPackageIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.canonical_repo_str() == other.canonical_repo_str() && self.package() == other.package()
    }
}

impl Eq for QueryPackageIdentity {}

impl std::hash::Hash for QueryPackageIdentity {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.canonical_repo_str().hash(state);
        self.package().hash(state);
    }
}

impl Ord for QueryPackageIdentity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.canonical_repo_str(), self.package())
            .cmp(&(other.canonical_repo_str(), other.package()))
    }
}

impl PartialOrd for QueryPackageIdentity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

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
        consuming_owner: QueryPackageIdentity,
    },
}

impl QueryCandidate {
    pub(crate) fn real(label: QueryLabel) -> Self {
        Self::Real(label)
    }

    pub(crate) fn fake(printed_label: QueryLabel, consuming_owner: QueryPackageIdentity) -> Self {
        Self::Fake {
            printed_label,
            consuming_owner,
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

    pub(crate) fn owner_identity(&self) -> Result<QueryPackageIdentity, QueryError> {
        match self {
            Self::Real(label) => label.owner_identity(),
            Self::Fake {
                consuming_owner, ..
            } => Ok(consuming_owner.dupe()),
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

    /// Retain one callback delivery after an in-place candidate filter. The
    /// IDs were already label-materialized when the delivery was created, so
    /// this deliberately performs neither candidate interning nor cross-batch
    /// collapse.
    pub(crate) fn from_delivery_ids(ids: Vec<QueryCandidateId>) -> Self {
        Self {
            batches: QueryCandidateBatch::from_ids(ids).into_iter().collect(),
        }
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
    pub(crate) fn sibling_packages(
        &self,
        arena: &QueryCandidateArena,
    ) -> Result<Arc<[QueryPackageIdentity]>, QueryError> {
        let mut packages = SmallSet::new();
        for batch in &self.batches {
            for id in batch.ids.iter().copied() {
                packages.insert(arena.get(id).owner_identity()?);
            }
        }
        Ok(packages.into_iter().collect::<Vec<_>>().into())
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

    pub(crate) fn from_materialized_ids(ids: Vec<QueryCandidateId>) -> Self {
        Self {
            batches: QueryCandidateBatch::from_ids(ids).into_iter().collect(),
        }
    }

    pub(crate) fn materialized_by_label(
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
        QueryCandidate::fake(label(value), root_owner(consuming_package))
    }

    fn root_owner(package: &str) -> QueryPackageIdentity {
        label(&format!("//{package}:__pkg__"))
            .owner_identity()
            .unwrap()
    }

    fn package_names(batches: &QueryCandidateBatches, arena: &QueryCandidateArena) -> Vec<String> {
        batches
            .sibling_packages(arena)
            .unwrap()
            .iter()
            .map(|owner| owner.package().as_str().to_owned())
            .collect()
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
    fn fake_candidate_owner_identity_is_symmetric_and_route_preserving() {
        let printed = label("//shared:one.bzl");
        let fake_a = QueryCandidate::fake(printed.dupe(), root_owner("a"));
        let fake_b = QueryCandidate::fake(printed.dupe(), root_owner("b"));
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

        let canonical = CanonicalRepoName::new("dep+").unwrap();
        let external = |apparent: &str| {
            QueryLabel::from_apparent_route(
                &slug_identity_v2::ApparentLabel::parse(&format!("@{apparent}//pkg:caller"))
                    .unwrap(),
                &canonical,
            )
            .unwrap()
            .owner_identity()
            .unwrap()
        };
        let routed = QueryCandidate::fake(label("//shared:route.bzl"), external("dep"));
        let aliased = QueryCandidate::fake(label("//shared:route.bzl"), external("alias"));
        assert_eq!(routed, aliased);
        let routed_id = arena.intern(routed);
        assert_eq!(routed_id, arena.intern(aliased));
        assert_eq!(
            arena
                .get(routed_id)
                .owner_identity()
                .unwrap()
                .apparent_repo()
                .unwrap()
                .as_str(),
            "dep"
        );
        assert_eq!(arena.len(), 4);
    }

    #[test]
    fn query_package_identity_canonical_equality_retains_first_apparent_route() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hash;
        use std::hash::Hasher;

        use slug_identity_v2::ApparentLabel;

        let canonical = CanonicalRepoName::new("dep+").unwrap();
        let dep = QueryLabel::from_apparent_route(
            &ApparentLabel::parse("@dep//pkg:target").unwrap(),
            &canonical,
        )
        .unwrap()
        .owner_identity()
        .unwrap();
        let alias = QueryLabel::from_apparent_route(
            &ApparentLabel::parse("@alias//pkg:target").unwrap(),
            &canonical,
        )
        .unwrap()
        .owner_identity()
        .unwrap();
        let hash = |owner: &QueryPackageIdentity| {
            let mut state = DefaultHasher::new();
            owner.hash(&mut state);
            state.finish()
        };

        assert_eq!(dep, alias);
        assert_eq!(dep.cmp(&alias), std::cmp::Ordering::Equal);
        assert_eq!(hash(&dep), hash(&alias));
        let mut owners = SmallSet::new();
        assert!(owners.insert(dep));
        assert!(!owners.insert(alias));
        let first = owners.into_iter().next().unwrap();
        assert_eq!(first.apparent_repo().unwrap().as_str(), "dep");
        assert_eq!(first.canonical_repo().unwrap().as_str(), "dep+");
        assert_eq!(first.package().as_str(), "pkg");
        assert!(
            QueryLabel::from_canonical(
                slug_identity_v2::CanonicalLabel::parse("@@dep+//pkg:target").unwrap()
            )
            .owner_identity()
            .unwrap_err()
            .to_string()
            .contains("lost its apparent repository route")
        );
        assert!(
            QueryPackageIdentity::external(
                CanonicalRepoName::root(),
                ApparentRepoName::root(),
                PackagePath::root(),
            )
            .unwrap_err()
            .to_string()
            .contains("requires nonroot canonical and apparent repositories")
        );
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
        assert_eq!(package_names(&ab, &arena), ["a", "b"]);
        let ba = b.union(a);
        assert_eq!(ba.batches().len(), 2);
        assert_eq!(package_names(&ba, &arena), ["b", "a"]);
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
        assert_eq!(package_names(&union, &arena), ["shared", "consumer"]);
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
    fn delivery_id_filter_keeps_distinct_nonempty_callback_boundaries() {
        let mut arena = QueryCandidateArena::new();
        let first = arena.intern(real("//pkg:first"));
        let dropped = arena.intern(real("//pkg:dropped"));
        let second = arena.intern(real("//pkg:second"));
        let filtered = QueryCandidateBatches::from_delivery_ids(vec![first])
            .union(QueryCandidateBatches::from_delivery_ids(vec![second]));

        assert_eq!(filtered.batches().len(), 2);
        assert_eq!(filtered.batches()[0].ids(), &[first]);
        assert_eq!(filtered.batches()[1].ids(), &[second]);
        assert_ne!(first, dropped);
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
