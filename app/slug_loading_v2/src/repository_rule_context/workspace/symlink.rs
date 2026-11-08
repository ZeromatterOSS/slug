//! Target conversion precedes destination conversion. Source strings name only
//! roots declared by this invocation prefix, never the available snapshot set.
use super::*;

pub(in crate::repository_rule_context) fn create(
    state: &RepositoryRuleInvocationState,
    target: Value<'_>,
    destination: Value<'_>,
) -> anyhow::Result<()> {
    let (target, source) = convert_target(state, target).map_err(|error| state.fail(error))?;
    let path = own_relative(state, destination).map_err(|error| state.fail(error))?;
    match replay(
        state,
        RepositoryWorkspaceOperation::Symlink {
            path,
            target,
            source,
        },
    )? {
        RepositoryWorkspaceResult::Unit => Ok(()),
        _ => Err(state.fail(RepositoryRuleInvocationError::WorkspaceReplay)),
    }
}

fn convert_target(
    state: &RepositoryRuleInvocationState,
    value: Value<'_>,
) -> Result<
    (NormalizedAbsolutePath, Option<RepositoryWorkspaceReadView>),
    RepositoryRuleInvocationError,
> {
    if let Some(text) = value.unpack_str() {
        if text.contains(['\0', '\\']) {
            return Err(RepositoryRuleInvocationError::SymlinkArgument);
        }
        match path(state, text) {
            Ok(RepositoryStarlarkPath::Workspace { absolute, .. }) => return Ok((absolute, None)),
            Err(RepositoryRuleInvocationError::WorkspaceUnsupported) => {}
            Err(error) => return Err(error),
            _ => unreachable!(),
        }
        // No normalization may turn an unsupported external spelling into a
        // declared root, and relative external escapes remain unsupported.
        if !Path::new(text).is_absolute() {
            return Err(RepositoryRuleInvocationError::WorkspaceUnsupported);
        }
        let absolute = NormalizedAbsolutePath::new(text)
            .map_err(|_| RepositoryRuleInvocationError::SymlinkArgument)?;
        if absolute.as_path().as_os_str() != Path::new(text).as_os_str() {
            return Err(RepositoryRuleInvocationError::WorkspaceUnsupported);
        }
        return declared_source(state, absolute, None);
    }
    if let Some(label) = StarlarkLabel::from_value(value) {
        let address = RepositoryLabelPathAddress::from_label(label.canonical());
        let resolved = resolve_label(state, &address)?;
        return resolved_target(state, &resolved);
    }
    match RepositoryStarlarkPath::from_value(value) {
        Some(RepositoryStarlarkPath::Workspace {
            absolute, replay, ..
        }) => {
            if !state
                .workspace
                .as_ref()
                .is_some_and(|ours| Arc::ptr_eq(ours, replay))
            {
                return Err(RepositoryRuleInvocationError::WorkspaceReplay);
            }
            Ok((absolute.clone(), None))
        }
        Some(RepositoryStarlarkPath::Label { address, path }) => {
            let resolved = resolve_label(state, address)?;
            if resolved != *path {
                return Err(RepositoryRuleInvocationError::WorkspaceReplay);
            }
            resolved_target(state, &resolved)
        }
        Some(_) => Err(RepositoryRuleInvocationError::WorkspaceUnsupported),
        None => Err(RepositoryRuleInvocationError::SymlinkArgument),
    }
}
fn resolved_target(
    state: &RepositoryRuleInvocationState,
    resolved: &HostRepositoryLabelPathValue,
) -> Result<
    (NormalizedAbsolutePath, Option<RepositoryWorkspaceReadView>),
    RepositoryRuleInvocationError,
> {
    let replay = state
        .workspace
        .as_ref()
        .ok_or(RepositoryRuleInvocationError::WorkspaceReplay)?
        .lock()
        .expect("workspace replay poisoned");
    if resolved.namespace()
        == PathObservationNamespace::Materialization(replay.reservation().instance())
    {
        if !resolved
            .path()
            .as_path()
            .starts_with(replay.reservation().display_root().as_path())
        {
            return Err(RepositoryRuleInvocationError::WorkspaceReplay);
        }
        return Ok((resolved.path().clone(), None));
    }
    drop(replay);
    declared_source(state, resolved.path().clone(), Some(resolved.namespace()))
}
fn declared_source(
    state: &RepositoryRuleInvocationState,
    absolute: NormalizedAbsolutePath,
    namespace: Option<PathObservationNamespace>,
) -> Result<
    (NormalizedAbsolutePath, Option<RepositoryWorkspaceReadView>),
    RepositoryRuleInvocationError,
> {
    let replay = state
        .workspace
        .as_ref()
        .ok_or(RepositoryRuleInvocationError::WorkspaceReplay)?
        .lock()
        .expect("workspace replay poisoned");
    let source = replay
        .sources
        .values()
        .find(|source| {
            let reservation = source.reservation();
            namespace.is_none_or(|namespace| {
                namespace == PathObservationNamespace::Materialization(reservation.instance())
            }) && absolute
                .as_path()
                .starts_with(reservation.display_root().as_path())
        })
        .ok_or(RepositoryRuleInvocationError::WorkspaceUnsupported)?;
    let view = RepositoryWorkspaceReadView::from_source(source.clone())
        .map_err(RepositoryRuleInvocationError::Workspace)?;
    Ok((absolute, Some(view)))
}
