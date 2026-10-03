//! Complete normal subtree certificates authorize reuse, never path/digest coincidence.
use slug_bzlmod_v2::RuntimeSourceDeclaration;
use slug_bzlmod_v2::RuntimeSourceGuestMappingRef;
use slug_bzlmod_v2::RuntimeSourceSelectorRef;
use slug_workspace_v2::InputImageAliasPolicy;
use slug_workspace_v2::InputImageEntryKind;

use super::*;

fn same_origin(a: &ObservedSelectedLabelSource, b: &ObservedSelectedLabelSource) -> bool {
    a.recipe().workspace() == b.recipe().workspace()
        && a.recipe().label() == b.recipe().label()
        && a.recipe().source() == b.recipe().source()
        && a.repository() == b.repository()
        && a.image_key().namespace() == b.image_key().namespace()
        && a.image_key().anchor_root() == b.image_key().anchor_root()
}

pub(super) fn candidate(
    declaration: &RuntimeSourceDeclaration,
    selected: Option<&ObservedSelectedLabelSource>,
    tool: Option<&ObservedSelectedLabelSource>,
) -> bool {
    matches!(
        declaration.guest_mapping(),
        RuntimeSourceGuestMappingRef::OriginalSource
    ) && matches!(
        declaration.selector(),
        RuntimeSourceSelectorRef::SelectedToolOriginLabelParent { .. }
    ) && selected.zip(tool).is_some_and(|(a, b)| same_origin(a, b))
}

pub(super) fn certify(
    row: &RuntimeSourceRequest,
    image: &ObservedInputImage,
    tools: &ObservedSelectedExtensionToolSources,
    active: &dyn Fn() -> bool,
) -> Result<RuntimeSourceImageBinding, RuntimeSourcesError> {
    let declaration = row
        .policy
        .declarations()
        .nth(row.index)
        .ok_or(RuntimeSourcesError::Operational)?;
    let mut binding = None;
    for (role, source) in tools.sources().enumerate() {
        require_active(active)?;
        let Some(metadata) = source.label_metadata() else {
            continue;
        };
        // Relocated declarations keep their independently captured source. Physical
        // overlap alone grants no tool authority and does not imply guest overlap.
        if !candidate(declaration, row.selected.as_ref(), source.label_selection()) {
            continue;
        }
        let tool = metadata.image();
        let root = image.source_root().as_path();
        let tool_root = tool.source_root().as_path();
        if !root.starts_with(tool_root) && !tool_root.starts_with(root) {
            continue;
        }
        if row.guest_prefix().as_path() != root
            || image.namespace() != tool.namespace()
            || image.anchor_root() != tool.anchor_root()
            || image.policy().aliases() != InputImageAliasPolicy::Deny
            || tool.policy().aliases() != InputImageAliasPolicy::Deny
            || image.is_regular_file_root()
            || tool.is_regular_file_root()
        {
            return Err(RuntimeSourcesError::InvalidScope);
        }
        let start = tool
            .entries()
            .binary_search_by(|e| e.path().as_path().cmp(root))
            .map_err(|_| RuntimeSourcesError::InvalidScope)?;
        let count = image.entries().len();
        let end = start
            .checked_add(count)
            .ok_or(RuntimeSourcesError::InvalidScope)?;
        let interval = tool
            .entries()
            .get(start..end)
            .ok_or(RuntimeSourcesError::InvalidScope)?;
        if interval
            .first()
            .is_none_or(|e| e.kind() != InputImageEntryKind::Directory)
            || image
                .entries()
                .first()
                .is_none_or(|e| e.kind() != InputImageEntryKind::Directory)
            || tool
                .entries()
                .get(end)
                .is_some_and(|e| e.path().as_path().starts_with(root))
            || (start > 0 && tool.entries()[start - 1].path().as_path().starts_with(root))
        {
            return Err(RuntimeSourcesError::InvalidScope);
        }
        for (actual, expected) in interval.iter().zip(image.entries()) {
            require_active(active)?;
            if !actual.path().as_path().starts_with(root)
                || actual.path() != expected.path()
                || actual.kind() != expected.kind()
                || actual.mode() != expected.mode()
                || actual.link_target().is_some()
                || expected.link_target().is_some()
                || actual.resolved_alias_path().is_some()
                || expected.resolved_alias_path().is_some()
            {
                return Err(RuntimeSourcesError::InvalidScope);
            }
        }
        // A duplicated role may reference the same full tool tree. Each keeps its
        // semantic occurrence; native storage already physically shares equal trees.
        binding.get_or_insert(RuntimeSourceImageBinding::CoveredTool {
            role,
            root_entry: start,
            entry_count: count,
        });
    }
    Ok(binding.unwrap_or(RuntimeSourceImageBinding::Captured))
}
