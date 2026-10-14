//! Certified hidden native containment composed with explicit graph edges.
use super::*;
pub(super) fn base(row: &PublishedNode) -> Result<usize, PublishedValueError> {
    let depth = match row {
        PublishedNode::Native(value) => value.ordinary_depth(),
        PublishedNode::Provider(provider) => provider
            .files_to_run
            .as_ref()
            .map_or(0, |value| value.ordinary_depth().saturating_add(1)),
        PublishedNode::ConfiguredTarget { providers, .. } => providers
            .iter()
            .map(|p| match p {
                PublishedTargetProvider::Native { provider, .. } => {
                    provider.ordinary_depth().saturating_add(1)
                }
                PublishedTargetProvider::Occurrence(_) => 0,
            })
            .max()
            .unwrap_or(0),
        _ => 0,
    };
    if depth > DEPTH_LIMIT {
        return Err(PublishedValueError::Limit("projection depth"));
    }
    Ok(depth)
}
