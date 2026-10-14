//! Source authority belongs to the actual original module, not its display path.
use std::sync::Arc;

use slug_build_api_v2::CallableGraphBuilder;
use slug_build_api_v2::CallableModuleIdentity;
use slug_build_api_v2::CallableModuleSource;
use slug_build_api_v2::CallableSourceAuthority;
use slug_build_api_v2::CallableSourceAuthorityData;
use slug_loading_v2::BzlLoadManifest;
use slug_loading_v2::BzlModuleIdentity;
use slug_loading_v2::defining_module_source_manifest;
use starlark::codemap::CodeMap;
use starlark::values::Value;
use starlark::values::ValueIdentity;
use starlark_map::unordered_map::UnorderedMap;

#[derive(Default)]
pub(super) struct SourceAuthorities<'v> {
    modules: UnorderedMap<ValueIdentity<'v>, CallableSourceAuthority>,
    digests: UnorderedMap<(usize, usize), [u8; 32]>,
}
impl<'v> SourceAuthorities<'v> {
    pub(super) fn authority(
        &mut self,
        client_data: Option<Value<'v>>,
        code: &CodeMap,
        builder: &mut CallableGraphBuilder,
        remaining: &mut usize,
    ) -> Result<CallableSourceAuthority, String> {
        let client_data =
            client_data.ok_or("Args callback requires actual original-module source metadata")?;
        let manifest = defining_module_source_manifest(client_data)
            .ok_or("Args callback original-module metadata has no tracked source manifest")?;
        let digest = self.digest(code, builder, remaining)?;
        let mut verified = false;
        for source in &*manifest.reachable {
            step(remaining)?;
            if source.identity == manifest.root && source.source_digest == digest {
                verified = true;
                break;
            }
        }
        if !verified {
            return Err(
                "Args callback source bytes differ from its tracked original-module root".into(),
            );
        }
        if let Some(authority) = self.modules.get(&client_data.identity()) {
            return Ok(authority.clone());
        }
        let authority = convert(manifest, builder, remaining)?;
        self.modules
            .insert(client_data.identity(), authority.clone());
        Ok(authority)
    }
    pub(super) fn verify_source(
        &mut self,
        authority: &CallableSourceAuthority,
        code: &CodeMap,
        builder: &mut CallableGraphBuilder,
        remaining: &mut usize,
    ) -> Result<(), String> {
        let digest = self.digest(code, builder, remaining)?;
        for source in &*authority.data().reachable {
            step(remaining)?;
            if source.digest == digest {
                return Ok(());
            }
        }
        Err("Args callback retained/inlined source lies outside its tracked source closure".into())
    }
    fn digest(
        &mut self,
        code: &CodeMap,
        builder: &mut CallableGraphBuilder,
        remaining: &mut usize,
    ) -> Result<[u8; 32], String> {
        step(remaining)?;
        let key = (code.source().as_ptr() as usize, code.source().len());
        if let Some(digest) = self.digests.get(&key) {
            return Ok(*digest);
        }
        if code.source().len() > slug_build_api_v2::callable_graph::CALLABLE_GRAPH_BYTE_LIMIT {
            return Err("Args callback source byte limit".into());
        }
        builder
            .charge_bytes(code.source().len())
            .map_err(|e| e.to_string())?;
        // Source checksum work shares the aggregate byte budget before hashing.
        // Existing exact crypto owner; no filesystem read or source text copy.
        let encoded = slug_bzlmod_v2::digest_module_file_content(code.source().as_bytes());
        if encoded.len() != 64 {
            return Err("invalid SHA-256 producer length".into());
        }
        let mut digest = [0; 32];
        for (out, pair) in digest.iter_mut().zip(encoded.as_bytes().chunks_exact(2)) {
            let digit = |c: u8| match c {
                b'0'..=b'9' => Some(c - b'0'),
                b'a'..=b'f' => Some(c - b'a' + 10),
                _ => None,
            };
            *out = digit(pair[0]).ok_or("invalid SHA-256 producer")? * 16
                + digit(pair[1]).ok_or("invalid SHA-256 producer")?;
        }
        self.digests.insert(key, digest);
        Ok(digest)
    }
}
fn step(remaining: &mut usize) -> Result<(), String> {
    *remaining = remaining
        .checked_sub(1)
        .ok_or("Args source inspection visit limit")?;
    Ok(())
}
fn identity(source: &BzlModuleIdentity) -> CallableModuleIdentity {
    CallableModuleIdentity {
        label: source.label.clone(),
        workspace_path: source.workspace_path.clone(),
        repository_mapping: source.repository_mapping.clone(),
    }
}
fn convert(
    manifest: &BzlLoadManifest,
    builder: &mut CallableGraphBuilder,
    remaining: &mut usize,
) -> Result<CallableSourceAuthority, String> {
    use slug_build_api_v2::callable_graph::CALLABLE_GRAPH_BYTE_LIMIT;
    use slug_build_api_v2::callable_graph::CALLABLE_GRAPH_NODE_LIMIT;
    if manifest.reachable.len() > CALLABLE_GRAPH_NODE_LIMIT
        || manifest.direct_children.len() > CALLABLE_GRAPH_NODE_LIMIT
    {
        return Err("Args source closure fact limit".into());
    }
    let mut cost = |s: &BzlModuleIdentity| -> Result<usize, String> {
        step(remaining)?;
        let mut bytes = s
            .workspace_path
            .as_os_str()
            .len()
            .saturating_add(s.label.package().repo().as_str().len())
            .saturating_add(s.label.package().package().as_str().len())
            .saturating_add(s.label.target().as_str().len());
        for (a, c) in &*s.repository_mapping {
            step(remaining)?;
            bytes = bytes
                .saturating_add(a.as_str().len())
                .saturating_add(c.as_str().len());
        }
        Ok(bytes)
    };
    let mut bytes = cost(&manifest.root)?;
    for child in &*manifest.direct_children {
        bytes = bytes.saturating_add(cost(child)?);
    }
    for source in &*manifest.reachable {
        bytes = bytes
            .saturating_add(cost(&source.identity)?)
            .saturating_add(std::mem::size_of::<CallableModuleSource>());
    }
    if bytes > CALLABLE_GRAPH_BYTE_LIMIT {
        return Err("Args source closure byte limit".into());
    }
    builder.charge_bytes(bytes).map_err(|e| e.to_string())?;
    CallableSourceAuthority::new(CallableSourceAuthorityData {
        root: identity(&manifest.root),
        direct_children: manifest
            .direct_children
            .iter()
            .map(identity)
            .collect::<Vec<_>>()
            .into(),
        reachable: manifest
            .reachable
            .iter()
            .map(|s| CallableModuleSource {
                identity: identity(&s.identity),
                digest: s.source_digest,
            })
            .collect::<Vec<_>>()
            .into(),
        fingerprint: manifest.fingerprint,
    })
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;
