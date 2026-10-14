//! Successful proofs over exact guarded allocations, scoped to one publication.
use starlark::codemap::CodeMapId;

use super::*;

struct RootProof<'v> {
    _client_data: Value<'v>,
    _code: CodeMap,
    authority: CallableSourceAuthority,
}
struct MemberProof {
    _authority: CallableSourceAuthority,
    _code: CodeMap,
}
struct DigestProof {
    _code: CodeMap,
    digest: [u8; 32],
}

#[derive(Default)]
pub(super) struct SourceMemo<'v> {
    roots: UnorderedMap<(ValueIdentity<'v>, CodeMapId), RootProof<'v>>,
    members: UnorderedMap<(usize, CodeMapId), MemberProof>,
    digests: UnorderedMap<CodeMapId, DigestProof>,
    reserved_entries: usize,
}
fn member_key(authority: &CallableSourceAuthority, code: &CodeMap) -> (usize, CodeMapId) {
    (
        authority.data() as *const CallableSourceAuthorityData as usize,
        code.id(),
    )
}
impl<'v> SourceMemo<'v> {
    pub(super) fn root(
        &self,
        client_data: Value<'v>,
        code: &CodeMap,
    ) -> Option<&CallableSourceAuthority> {
        self.roots
            .get(&(client_data.identity(), code.id()))
            .map(|proof| &proof.authority)
    }
    pub(super) fn member(&self, authority: &CallableSourceAuthority, code: &CodeMap) -> bool {
        self.members.contains_key(&member_key(authority, code))
    }
    pub(super) fn digest(&self, code: &CodeMap) -> Option<[u8; 32]> {
        self.digests.get(&code.id()).map(|proof| proof.digest)
    }
    /// Credit covers the fixed small key/guard types, the first multi-slot
    /// HashTable allocation, subsequent capacity growth and control bytes.
    /// Credit accumulates per inserted entry; no guard is cloned before this.
    pub(super) fn preflight(
        &mut self,
        count: usize,
        builder: &mut PublishedValueArenaBuilder,
        remaining: &mut usize,
    ) -> Result<(), String> {
        let entries = self
            .reserved_entries
            .checked_add(count)
            .ok_or("Args source cache entry overflow")?;
        if entries > slug_build_api_v2::callable_graph::CALLABLE_GRAPH_NODE_LIMIT {
            // Native budget failure poisons subsequent use of this builder.
            let _ = builder.charge_bytes(usize::MAX);
            return Err("Args source cache entry limit".into());
        }
        for _ in 0..count {
            step(builder, remaining)?;
        }
        builder
            .producer_scratch()
            .charge_bytes(count.saturating_mul(1024))
            .map_err(|error| error.to_string())?;
        self.reserved_entries = entries;
        Ok(())
    }
    pub(super) fn record_root(
        &mut self,
        client_data: Value<'v>,
        code: &CodeMap,
        authority: &CallableSourceAuthority,
    ) {
        self.roots.insert(
            (client_data.identity(), code.id()),
            RootProof {
                _client_data: client_data,
                _code: code.clone(),
                authority: authority.clone(),
            },
        );
    }
    pub(super) fn record_member(&mut self, authority: &CallableSourceAuthority, code: &CodeMap) {
        self.members.insert(
            member_key(authority, code),
            MemberProof {
                _authority: authority.clone(),
                _code: code.clone(),
            },
        );
    }
    pub(super) fn record_digest(&mut self, code: &CodeMap, digest: [u8; 32]) {
        self.digests.insert(
            code.id(),
            DigestProof {
                _code: code.clone(),
                digest,
            },
        );
    }
}

#[cfg(test)]
mod tests;
