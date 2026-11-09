//! The ordinary result producer observes one current, exact request association.
//! This cutoff grants no native authority and needs no workspace execution policy.
use dice::DiceProjectionComputations;
use dice::ProjectionKey;

use super::*;

#[derive(Clone, PartialEq, Eq, Allocative)]
pub(super) struct RepositoryMaterializationResultEntryProjection {
    request: Arc<RepositoryMaterializationRequest>,
}

impl RepositoryMaterializationResultEntryProjection {
    pub(super) fn new(request: Arc<RepositoryMaterializationRequest>) -> Self {
        Self { request }
    }

    fn select(
        &self,
        epoch: &RepositoryMaterializationResultEpoch,
    ) -> Option<Arc<RepositoryMaterializationEpochEntry>> {
        // get() checks the workspace and CURRENT repository slot. Retained
        // generated output inventory must never resurrect an old association.
        epoch
            .get(&self.request)
            .filter(|entry| entry.request == self.request)
            .map(|entry| Arc::new(entry.clone()))
    }
}

impl Hash for RepositoryMaterializationResultEntryProjection {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.request.id.hash(state);
    }
}

impl fmt::Debug for RepositoryMaterializationResultEntryProjection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RepositoryMaterializationResultEntryProjection")
            .field("workspace", &self.request.id.workspace)
            .field("repository", &self.request.id.canonical_repo)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for RepositoryMaterializationResultEntryProjection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "repository-materialization-result-entry:{}",
            self.request.id.canonical_repo
        )
    }
}

impl ProjectionKey for RepositoryMaterializationResultEntryProjection {
    type DeriveFromKey = RepositoryMaterializationResultEpochKey;
    type Value = Option<Arc<RepositoryMaterializationEpochEntry>>;

    fn compute(
        &self,
        epoch: &RepositoryMaterializationResultEpoch,
        _: &DiceProjectionComputations,
    ) -> Self::Value {
        self.select(epoch)
    }

    fn equality(a: &Self::Value, b: &Self::Value) -> bool {
        a == b
    }

    // Absence is an exact, valid projection value. The parent's resulting Need
    // remains invalid and is retried under its existing acquisition contract.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_preserves_actual_request_arc_and_redacts_full_spec() {
        // Pure metadata only: no native materialization or source authority.
        let workspace = NormalizedAbsolutePath::new("/result-entry").unwrap();
        let mut attributes = SmallMap::new();
        attributes.insert(
            CompactString::new("credential"),
            OverrideAttributeValue::String("private-attribute-value".into()),
        );
        let actual = Arc::new(RepositoryMaterializationRequest {
            id: RepositoryMaterializationRequestId {
                workspace: workspace.clone(),
                canonical_repo: CanonicalRepoName::new("selected").unwrap(),
            },
            repo_spec: RepoSpec {
                rule_id: crate::RepoRuleId {
                    bzl_file: CanonicalLabel::parse("@@//:repository.bzl").unwrap(),
                    rule_name: "repository".into(),
                },
                attributes: Arc::new(attributes),
            },
            kind: RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new("/private-logical-root").unwrap(),
            },
        });
        let expected = Arc::new(actual.as_ref().clone());
        assert!(!Arc::ptr_eq(&actual, &expected));
        let projection = RepositoryMaterializationResultEntryProjection::new(expected.clone());
        let epoch = RepositoryMaterializationResultEpoch::new(
            workspace,
            [RepositoryMaterializationEpochEntry {
                request: actual.clone(),
                result: RepositoryMaterializationResult::Success(
                    RepositoryMaterializationSuccess::Local,
                ),
            }],
        )
        .unwrap();
        let selected = projection.select(&epoch).unwrap();
        assert!(Arc::ptr_eq(&selected.request, &actual));
        assert!(!Arc::ptr_eq(&selected.request, &expected));
        let mut changed = expected.as_ref().clone();
        changed.repo_spec.attributes = Arc::default();
        let changed = RepositoryMaterializationResultEntryProjection::new(Arc::new(changed));
        assert_ne!(projection, changed);
        assert!(changed.select(&epoch).is_none());
        let rendered = format!("{projection:?} {projection}");
        for private in [
            "credential",
            "private-attribute-value",
            "private-logical-root",
        ] {
            assert!(!rendered.contains(private));
        }
    }
}
