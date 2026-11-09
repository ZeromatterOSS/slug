//! A single caller-neutral replay stream for actual repository mutations.

use super::*;

pub(super) fn replay_operation(
    state: &RepositoryRuleInvocationState,
    operation: RepositoryFilesystemOperation,
) -> anyhow::Result<RepositoryFilesystemResult> {
    let (owner, ordinal) = state
        .filesystem_owner
        .as_ref()
        .ok_or_else(|| state.fail(RepositoryRuleInvocationError::FilesystemReplay))?;
    let index = *state.filesystem_call_index.borrow();
    if index >= REPOSITORY_FILESYSTEM_MAX_CALLS {
        return Err(state.fail(RepositoryRuleInvocationError::FilesystemLimit));
    }
    let prefix = state
        .effects
        .borrow()
        .as_ref()
        .expect("repository context has not completed")
        .snapshot();
    prefix
        .with_operation(&operation)
        .map_err(|error| state.fail(RepositoryRuleInvocationError::Plan(error)))?;
    let request = Arc::new(RepositoryFilesystemRequest::new(
        RepositoryFilesystemCallId::new(owner.clone(), *ordinal, index),
        state.generated_repository.clone(),
        operation.clone(),
        prefix,
    ));
    let record = state
        .filesystem_transcript
        .matching_record(&request)
        .map_err(|_| state.fail(RepositoryRuleInvocationError::FilesystemReplay))?;
    let Some(record) = record else {
        if index != state.prepared_filesystem_count() {
            return Err(state.fail(RepositoryRuleInvocationError::FilesystemReplay));
        }
        return Err(state.fail(RepositoryRuleInvocationError::FilesystemNeed(request)));
    };
    let valid_result = match (&operation, record.result()) {
        (
            RepositoryFilesystemOperation::Archive(effect),
            RepositoryFilesystemResult::Archive(sha),
        ) => effect.expected_sha256() == Some(*sha),
        (RepositoryFilesystemOperation::Delete(_), RepositoryFilesystemResult::Delete(_)) => true,
        _ => false,
    };
    if state.filesystem_lease_nonce.is_none() || !valid_result {
        return Err(state.fail(RepositoryRuleInvocationError::FilesystemReplay));
    }
    let mut effects = state.effects.borrow_mut();
    let effects = effects
        .as_mut()
        .expect("repository context has not completed");
    match operation {
        RepositoryFilesystemOperation::Archive(effect) => effects.push_archive_effect(effect),
        RepositoryFilesystemOperation::Delete(path) => effects.push_delete(path),
    }
    .map_err(|error| state.fail(RepositoryRuleInvocationError::Plan(error)))?;
    *state.filesystem_call_index.borrow_mut() += 1;
    Ok(record.result().clone())
}
