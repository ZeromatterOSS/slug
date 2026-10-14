//! Exact source certificates for private unit-test module preparation.
//!
//! Call before evaluation. This does not evaluate, recover identities from
//! filenames, issue function owners or repair intentionally invalid provenance.
use super::*;

pub(crate) struct PreparedTestBzlSource {
    manifest: BzlLoadManifest,
    context: BzlEvaluationContext,
}

impl PreparedTestBzlSource {
    pub(crate) fn manifest(&self) -> &BzlLoadManifest {
        &self.manifest
    }

    pub(crate) fn context(&self) -> &BzlEvaluationContext {
        &self.context
    }

    pub(crate) fn sources(&self) -> Arc<[(CompactString, BzlModuleSourceProvenance)]> {
        manifest_starlark_sources(&self.manifest)
    }
}

pub(crate) fn child_manifest(module: &FrozenModule) -> anyhow::Result<&BzlLoadManifest> {
    let data = module
        .client_data()
        .ok_or_else(|| anyhow::anyhow!("test child has no actual original-module metadata"))?;
    crate::starlark_label::defining_module_source_manifest(data.to_value())
        .ok_or_else(|| anyhow::anyhow!("test child has no actual defining manifest"))
}

pub(crate) fn prepare(
    module: &Module,
    ast: &AstModule,
    lexical_identity: BzlModuleIdentity,
    children: &[&BzlLoadManifest],
) -> anyhow::Result<PreparedTestBzlSource> {
    let source = ast.file_span(ast.statement().span).file;
    let source_digest = digest(source.source());
    let mut reachable = vec![BzlModuleSourceProvenance::new(
        lexical_identity.clone(),
        source_digest,
    )];
    let mut direct_children = Vec::new();
    let mut direct_indices = Vec::new();
    for (index, child) in children.iter().enumerate() {
        if !direct_children
            .iter()
            .any(|identity: &BzlModuleIdentity| identity.label == child.root.label)
        {
            direct_children.push(child.root.clone());
            direct_indices.push(index);
        }
        for incoming in child.reachable.iter() {
            if let Some(existing) = reachable
                .iter()
                .find(|row| row.identity.label == incoming.identity.label)
            {
                anyhow::ensure!(
                    existing == incoming,
                    "conflicting actual test child provenance"
                );
            } else {
                reachable.push(incoming.clone());
            }
        }
    }
    let mut hasher = Sha256::new();
    hasher.update(b"slug-v2-bzl-load-manifest\0");
    hasher.update(source_digest);
    for index in direct_indices {
        fingerprint_identity(&mut hasher, &children[index].root);
        hasher.update(children[index].fingerprint);
    }
    let manifest = BzlLoadManifest {
        root: lexical_identity,
        direct_children: direct_children.into(),
        reachable: reachable.into(),
        fingerprint: hasher.finalize().into(),
    };
    crate::starlark_label::attach_defining_module_manifest(module, &manifest)?;
    let context = BzlEvaluationContext::from_manifest(&manifest);
    Ok(PreparedTestBzlSource { manifest, context })
}

#[cfg(test)]
mod tests;
