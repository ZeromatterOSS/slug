//! Repository-root confined deletion; the Boolean comes from Core I/O.

use super::*;

fn deletion_path(
    repository_name: &str,
    generated_present: &Arc<Mutex<GeneratedPathFacts>>,
    value: Value<'_>,
) -> Result<CompactString, RepositoryRuleInvocationError> {
    if let Some(path) = value.unpack_str() {
        let normalized = normalized_generated_path("", path)
            .filter(|normalized| normalized.as_str() == path)
            .ok_or(RepositoryRuleInvocationError::DeleteArgument)?;
        return Ok(normalized);
    }
    match RepositoryStarlarkPath::from_value(value) {
        Some(RepositoryStarlarkPath::Generated {
            repository,
            relative,
            present,
        }) if repository == repository_name && Arc::ptr_eq(present, generated_present) => {
            Ok(relative.clone())
        }
        _ => Err(RepositoryRuleInvocationError::DeleteArgument),
    }
}

pub(super) fn delete(
    state: &RepositoryRuleInvocationState,
    value: Value<'_>,
) -> anyhow::Result<bool> {
    if state.workspace.is_some() {
        let path = workspace::own_relative(state, value).map_err(|error| state.fail(error))?;
        return match workspace::replay(
            state,
            slug_bzlmod_v2::RepositoryWorkspaceOperation::Delete(path),
        )? {
            slug_bzlmod_v2::RepositoryWorkspaceResult::Delete(value) => Ok(value),
            _ => Err(state.fail(RepositoryRuleInvocationError::WorkspaceReplay)),
        };
    }
    let path = deletion_path(&state.generated_repository, &state.generated_present, value)
        .map_err(|error| state.fail(error))?;
    let operation = RepositoryFilesystemOperation::delete(path.clone())
        .map_err(|_| state.fail(RepositoryRuleInvocationError::DeleteArgument))?;
    let deleted = filesystem::replay_operation(state, operation)?
        .deleted()
        .ok_or_else(|| state.fail(RepositoryRuleInvocationError::FilesystemReplay))?;
    // Remove known authored paths, without certifying any unknown archive path.
    // Ancestors remain present even when their last known child is removed.
    let mut facts = state
        .generated_present
        .lock()
        .expect("generated path state mutex poisoned");
    facts.present.retain(|known| {
        known != &path
            && !known
                .strip_prefix(path.as_str())
                .is_some_and(|suffix| suffix.starts_with('/'))
    });
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delete_paths_reject_root_traversal_absolute_and_foreign_paths_before_replay() {
        for path in [
            "",
            ".",
            "..",
            "../escape",
            "/host",
            "a/../b",
            "a//b",
            "C:/host",
            "a\\b",
        ] {
            assert!(
                RepositoryFilesystemOperation::delete(path.into()).is_err(),
                "{path:?}"
            );
        }
        let module = Module::new();
        let facts = Arc::new(Mutex::new(GeneratedPathFacts {
            present: SmallSet::new(),
            archive_seen: false,
            refused: false,
        }));
        for raw_label in ["@@//:file", "@@foreign//:file"] {
            let label = slug_identity_v2::CanonicalLabel::parse(raw_label).unwrap();
            let value = module
                .heap()
                .alloc_simple(StarlarkLabel::new(label.clone()));
            assert!(matches!(
                deletion_path("repo", &facts, value),
                Err(RepositoryRuleInvocationError::DeleteArgument)
            ));
            let external_path = module.heap().alloc_simple(RepositoryStarlarkPath::Label {
                address: RepositoryLabelPathAddress::from_label(&label),
                path: HostRepositoryLabelPathValue::new(
                    NormalizedAbsolutePath::new("/tmp/external/file").unwrap(),
                    PathObservationNamespace::Host,
                )
                .unwrap(),
            });
            assert!(matches!(
                deletion_path("repo", &facts, external_path),
                Err(RepositoryRuleInvocationError::DeleteArgument)
            ));
        }
        let host_path = NormalizedAbsolutePath::new("/tmp/elsewhere").unwrap();
        let host = module.heap().alloc_simple(RepositoryStarlarkPath::Host {
            path: host_path.clone(),
            exists: true,
        });
        assert!(matches!(
            deletion_path("repo", &facts, host),
            Err(RepositoryRuleInvocationError::DeleteArgument)
        ));
        let which = module.heap().alloc_simple(RepositoryStarlarkPath::Which(
            HostRepositoryLabelPathValue::new(host_path, PathObservationNamespace::Host).unwrap(),
        ));
        assert!(matches!(
            deletion_path("repo", &facts, which),
            Err(RepositoryRuleInvocationError::DeleteArgument)
        ));
        for (repository, present) in [
            ("foreign", facts.clone()),
            (
                "repo",
                Arc::new(Mutex::new(GeneratedPathFacts {
                    present: SmallSet::new(),
                    archive_seen: false,
                    refused: false,
                })),
            ),
        ] {
            let foreign = module
                .heap()
                .alloc_simple(RepositoryStarlarkPath::Generated {
                    repository: repository.into(),
                    relative: "file".into(),
                    present,
                });
            assert!(matches!(
                deletion_path("repo", &facts, foreign),
                Err(RepositoryRuleInvocationError::DeleteArgument)
            ));
        }
        let own = module
            .heap()
            .alloc_simple(RepositoryStarlarkPath::Generated {
                repository: "repo".into(),
                relative: "dir/file".into(),
                present: facts.clone(),
            });
        assert_eq!(deletion_path("repo", &facts, own).unwrap(), "dir/file");
        assert_eq!(
            deletion_path("repo", &facts, module.heap().alloc("another")).unwrap(),
            "another"
        );
    }
}
