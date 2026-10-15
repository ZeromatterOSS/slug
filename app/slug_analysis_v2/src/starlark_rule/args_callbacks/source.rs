//! Source authority belongs to the actual original module, not its display path.
use std::sync::Arc;

use slug_build_api_v2::CallableEnvironmentBinding;
use slug_build_api_v2::CallableModuleIdentity;
use slug_build_api_v2::CallableModuleSource;
use slug_build_api_v2::CallableSourceAuthority;
use slug_build_api_v2::CallableSourceAuthorityData;
use slug_build_api_v2::published_value::PublishedValueArenaBuilder;
use slug_loading_v2::BzlLoadManifest;
use slug_loading_v2::BzlModuleIdentity;
use slug_loading_v2::defining_module_source_context;
use slug_loading_v2::defining_module_source_manifest;
use starlark::codemap::CodeMap;
use starlark::values::Value;
use starlark::values::ValueIdentity;
use starlark_map::unordered_map::UnorderedMap;

// Private production owner: graph.rs constructs this only in
// PublicationBuilder::new, beside that publisher's single graph, and calls it
// only with that graph. Never transfer proof caches to another builder/session.
#[derive(Default)]
pub(super) struct SourceAuthorities<'v> {
    modules: UnorderedMap<ValueIdentity<'v>, CallableSourceAuthority>,
    memo: memo::SourceMemo<'v>,
    mappings: conversion::MappingAudits,
}
mod manifest;
mod memo;
use manifest::SourceManifest;
impl<'v> SourceAuthorities<'v> {
    fn complete<T>(
        &mut self,
        builder: &mut PublishedValueArenaBuilder,
        result: Result<T, String>,
    ) -> Result<T, String> {
        if result.is_err() {
            self.mappings.fail();
            // Preserve the original error while refusing all later reuse.
            let _ = builder.charge_bytes(usize::MAX);
        }
        result
    }
    pub(super) fn authority(
        &mut self,
        client_data: Option<Value<'v>>,
        code: &CodeMap,
        builder: &mut PublishedValueArenaBuilder,
        remaining: &mut usize,
    ) -> Result<CallableSourceAuthority, String> {
        let result = self
            .mappings
            .admit_header(builder)
            .and_then(|()| self.authority_inner(client_data, code, builder, remaining));
        self.complete(builder, result)
    }
    pub(super) fn verify_source(
        &mut self,
        authority: &CallableSourceAuthority,
        code: &CodeMap,
        builder: &mut PublishedValueArenaBuilder,
        remaining: &mut usize,
    ) -> Result<(), String> {
        let result = self
            .mappings
            .admit_header(builder)
            .and_then(|()| self.verify_source_inner(authority, code, builder, remaining));
        self.complete(builder, result)
    }
    fn digest(
        &mut self,
        code: &CodeMap,
        builder: &mut PublishedValueArenaBuilder,
        remaining: &mut usize,
    ) -> Result<[u8; 32], String> {
        let result = self
            .mappings
            .admit_header(builder)
            .and_then(|()| self.digest_inner(code, builder, remaining));
        self.complete(builder, result)
    }

    fn authority_inner(
        &mut self,
        client_data: Option<Value<'v>>,
        code: &CodeMap,
        builder: &mut PublishedValueArenaBuilder,
        remaining: &mut usize,
    ) -> Result<CallableSourceAuthority, String> {
        let client_data =
            client_data.ok_or("Args callback requires actual original-module source metadata")?;
        let manifest =
            if let Some(context) = defining_module_source_context(client_data) {
                SourceManifest::Effective(context)
            } else {
                SourceManifest::Raw(defining_module_source_manifest(client_data).ok_or(
                    "Args callback original-module metadata has no tracked source manifest",
                )?)
            };
        // Each incoming root proof lookup is charged even on a memo hit.
        step(builder, remaining)?;
        if let Some(authority) = self.memo.root(client_data, code) {
            return Ok(authority.clone());
        }
        let digest = self.digest(code, builder, remaining)?;
        let mut verified = false;
        for source in manifest.reachable() {
            step(builder, remaining)?;
            if source.identity == *manifest.root() && source.source_digest == digest {
                verified = true;
                break;
            }
        }
        if !verified {
            return Err(
                "Args callback source bytes differ from its tracked original-module root".into(),
            );
        }
        let cached = self.modules.get(&client_data.identity()).cloned();
        let new_module = cached.is_none();
        let authority = match cached {
            Some(authority) => authority,
            None => conversion::convert_manifest(manifest, &mut self.mappings, builder, remaining)?,
        };
        let new_member = !self.memo.member(&authority, code);
        // Root success implies membership, but not vice versa. Preflight all
        // new tables/guards together before inserting any successful proof.
        self.memo.preflight(
            1 + usize::from(new_member) + usize::from(new_module),
            builder,
            remaining,
        )?;
        if new_module {
            self.modules
                .insert(client_data.identity(), authority.clone());
        }
        self.memo.record_root(client_data, code, &authority);
        if new_member {
            self.memo.record_member(&authority, code);
        }
        Ok(authority)
    }
    fn verify_source_inner(
        &mut self,
        authority: &CallableSourceAuthority,
        code: &CodeMap,
        builder: &mut PublishedValueArenaBuilder,
        remaining: &mut usize,
    ) -> Result<(), String> {
        // One paid incoming lookup replaces repeated digest+closure scans
        // only after this exact immutable authority/CodeMap pair succeeded.
        step(builder, remaining)?;
        if self.memo.member(authority, code) {
            return Ok(());
        }
        let digest = self.digest(code, builder, remaining)?;
        for source in &*authority.data().reachable {
            step(builder, remaining)?;
            if source.digest == digest {
                self.memo.preflight(1, builder, remaining)?;
                self.memo.record_member(authority, code);
                return Ok(());
            }
        }
        Err("Args callback retained/inlined source lies outside its tracked source closure".into())
    }
    fn digest_inner(
        &mut self,
        code: &CodeMap,
        builder: &mut PublishedValueArenaBuilder,
        remaining: &mut usize,
    ) -> Result<[u8; 32], String> {
        step(builder, remaining)?;
        if let Some(digest) = self.memo.digest(code) {
            return Ok(digest);
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
        self.memo.preflight(1, builder, remaining)?;
        self.memo.record_digest(code, digest);
        Ok(digest)
    }
}
fn step(builder: &mut PublishedValueArenaBuilder, remaining: &mut usize) -> Result<(), String> {
    builder.charge_visit().map_err(|e| e.to_string())?;
    *remaining = match remaining.checked_sub(1) {
        Some(remaining) => remaining,
        None => {
            // An exhausted host inspection budget must poison native reuse too.
            let _ = builder.charge_bytes(usize::MAX);
            return Err("Args source inspection visit limit".into());
        }
    };
    Ok(())
}
fn identity(source: &BzlModuleIdentity) -> CallableModuleIdentity {
    CallableModuleIdentity {
        label: source.label.clone(),
        source: source.source.clone(),
        repository_mapping: source.repository_mapping.clone(),
    }
}
mod conversion;
#[cfg(test)]
use conversion::convert;

#[cfg(test)]
mod tests;
