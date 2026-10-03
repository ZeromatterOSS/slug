//! New external Host authority; the materializer's workspace-only arm is unchanged.
use slug_bzlmod_v2::RuntimeSourceSelectorRef;

use super::*;

#[cfg(test)]
thread_local! {
    static ROOT_OPENS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)};
    static OPENED_DECLARATIONS: std::cell::Cell<u64> = const {std::cell::Cell::new(0)};
}
#[cfg(test)]
pub(in crate::runtime::dice) fn reset_opens() {
    ROOT_OPENS.set(0);
    OPENED_DECLARATIONS.set(0);
}
#[cfg(test)]
pub(in crate::runtime::dice) fn opens() -> usize {
    ROOT_OPENS.get()
}
#[cfg(test)]
pub(in crate::runtime::dice) fn opened_declarations() -> u64 {
    OPENED_DECLARATIONS.get()
}

pub(in crate::runtime::dice) fn source(
    command: &NativeDemandCommand<'_>,
    plan: &ObservedRuntimeSourceRequests,
    row: &RuntimeSourceRequest,
    active: &dyn Fn() -> bool,
) -> Result<input_image::ScopedImageSource, NativeDemandSessionError> {
    preflight(command, plan, row)?;
    if !active() {
        return Err(refusal("runtime source preparation cancelled"));
    }
    #[cfg(test)]
    ROOT_OPENS.set(ROOT_OPENS.get() + 1);
    #[cfg(test)]
    OPENED_DECLARATIONS.set(OPENED_DECLARATIONS.get() | (1u64 << row.declaration_index()));
    let declaration = plan
        .policy()
        .declarations()
        .nth(row.declaration_index())
        .expect("validated index");
    let source = match declaration.selector() {
        RuntimeSourceSelectorRef::ExplicitHostDirectory { anchor } => {
            // No ambient fallback: every ancestor is nofollow, and the selected
            // exact anchor must remain the same named filesystem object.
            let fd = input_image::open_image_anchor(anchor.normalized_ref())
                .map_err(NativeDemandSessionError::InputImage)?;
            input_image::ScopedImageSource::new(
                PathObservationNamespace::Host,
                anchor.normalized_ref().dupe(),
                fd,
                None,
            )
            .map_err(NativeDemandSessionError::InputImage)?
        }
        _ => command
            .runtime
            .repository_materializer
            .image_source(
                command.repository_session,
                row.image_key().namespace(),
                row.image_key().anchor_root(),
                row.selected_label_source().and_then(|s| s.repository()),
            )
            .map_err(NativeDemandSessionError::Repository)?,
    };
    preflight(command, plan, row)?;
    if !active() {
        return Err(refusal("runtime source preparation cancelled"));
    }
    Ok(source)
}

pub(in crate::runtime::dice) fn preflight(
    command: &NativeDemandCommand<'_>,
    plan: &ObservedRuntimeSourceRequests,
    row: &RuntimeSourceRequest,
) -> Result<(), NativeDemandSessionError> {
    let budget = command.runtime.image_resources.descriptor_budget();
    if command.inputs.request.runtime_sources.as_ref() != Some(plan.policy())
        || plan.policy().workspace().as_path() != command.runtime.workspace
        || !plan.same_resources(&command.runtime.scratch_resources)
        || !plan.policy().same_budget(budget)
        || !command
            .runtime
            .repository_materializer
            .session_is_active(command.repository_session)
        || !plan.requests().any(|current| current == row)
    {
        return Err(refusal("foreign runtime source authority"));
    }
    let declaration = plan
        .policy()
        .declarations()
        .nth(row.declaration_index())
        .ok_or_else(|| refusal("foreign runtime declaration index"))?;
    let key = row.image_key();
    if key.policy() != &declaration.image_policy() || key.source_root().as_path() == Path::new("/")
    {
        return Err(refusal("foreign runtime source scope"));
    }
    match declaration.guest_mapping() {
        slug_bzlmod_v2::RuntimeSourceGuestMappingRef::OriginalSource => {
            if row.guest_prefix().as_path() != key.source_root().as_path() {
                return Err(refusal("foreign original runtime mapping"));
            }
        }
        slug_bzlmod_v2::RuntimeSourceGuestMappingRef::ExplicitPrefix(prefix) => {
            if row.guest_prefix() != prefix {
                return Err(refusal("foreign explicit runtime mapping"));
            }
        }
    }
    if !row.guest_prefix().same_budget(budget) {
        return Err(refusal("foreign resolved runtime mapping budget"));
    }
    match declaration.selector() {
        RuntimeSourceSelectorRef::ExplicitHostDirectory { anchor } => {
            let path = anchor.as_path();
            let workspace = command.runtime.workspace.as_path();
            if !anchor.same_budget(budget)
                || path == Path::new("/")
                || path.starts_with(workspace)
                || workspace.starts_with(path)
                || key.namespace() != PathObservationNamespace::Host
                || key.anchor_root() != anchor.normalized_ref()
                || key.source_root() != anchor.normalized_ref()
                || row.selected_label_source().is_some()
            {
                return Err(refusal("foreign external runtime root"));
            }
        }
        RuntimeSourceSelectorRef::WorkspaceRelativeDirectory { components } => {
            let relative = key
                .source_root()
                .as_path()
                .strip_prefix(&command.runtime.workspace)
                .map_err(|_| refusal("foreign workspace runtime subtree"))?;
            if key.namespace() != PathObservationNamespace::Host
                || key.anchor_root().as_path() != command.runtime.workspace
                || row.selected_label_source().is_some()
                || relative.components().count() != components.len()
                || relative
                    .components()
                    .zip(components)
                    .any(|(actual, expected)| actual.as_os_str() != expected.as_str())
            {
                return Err(refusal("foreign workspace runtime subtree"));
            }
        }
        RuntimeSourceSelectorRef::SelectedToolOriginLabelParent { role, .. } => {
            let selected = row
                .selected_label_source()
                .ok_or_else(|| refusal("missing runtime Label origin"))?;
            let tool = plan
                .tool_requests()
                .requests()
                .find(|r| r.operand().role == role)
                .and_then(|r| r.label_selection())
                .ok_or_else(|| refusal("runtime role has no Label origin"))?;
            if selected.recipe().workspace() != tool.recipe().workspace()
                || key.namespace() != selected.image_key().namespace()
                || key.anchor_root() != selected.image_key().anchor_root()
                || key.source_root() != selected.selected_file().normalized_ref()
                || !key
                    .source_root()
                    .as_path()
                    .starts_with(selected.image_key().source_root().as_path())
            {
                return Err(refusal("foreign runtime Label subtree"));
            }
            super::super::input_image_bridge::label_source::check_selection(
                command.runtime,
                selected,
            )?;
        }
    }
    Ok(())
}
