//! Existing evaluator-owned text construction over immutable logical bytes.
//! No decoded buffer is retained beside the workspace body.
use slug_bzlmod_v2::RepositoryWorkspaceBody;

pub(in crate::repository_rule_context) fn latin1_text(body: &RepositoryWorkspaceBody) -> String {
    let mut text = String::with_capacity(body.len());
    body.for_each_chunk(&mut |chunk| {
        text.extend(chunk.iter().copied().map(char::from));
    });
    text
}

/// Replaces the template algorithm's existing source `.to_vec()` copy. Its
/// caller retains the original limit and substitution validation order.
pub(in crate::repository_rule_context) fn template_input(
    body: &RepositoryWorkspaceBody,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(body.len());
    body.for_each_chunk(&mut |chunk| bytes.extend_from_slice(chunk));
    bytes
}
