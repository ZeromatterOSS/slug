//! Raw source loads and explicit environment bindings remain distinct provenance.
use sha2::Digest;
use sha2::Sha256;

use super::*;
mod output;
mod plan;
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct BzlEnvironmentSourceBinding {
    pub global: CompactString,
    pub export: CompactString,
    pub module: BzlModuleIdentity,
}
#[derive(Debug, Clone, Allocative)]
pub struct BzlModuleSourceContext {
    retention: Budget,
    raw: Arc<BzlLoadManifest>,
    bindings: Arc<[BzlEnvironmentSourceBinding]>,
    reachable: Arc<[BzlModuleSourceProvenance]>,
    fingerprint: [u8; 32],
    sources: crate::source_table::CertifiedSourceTable,
    producer_work: Budget,
}
impl PartialEq for BzlModuleSourceContext {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
            && self.bindings == other.bindings
            && self.reachable == other.reachable
            && self.fingerprint == other.fingerprint
            && self.sources == other.sources
    }
}
impl Eq for BzlModuleSourceContext {}
fn visit_sources<'a>(
    raw: &'a BzlLoadManifest,
    env: &'a PreparedBzlEnvironment,
    children: impl Iterator<Item = &'a BzlModuleSourceContext>,
    mut visit: impl FnMut(&'a BzlModuleSourceProvenance) -> Result<(), String>,
) -> Result<(), String> {
    for source in &*raw.reachable {
        visit(source)?;
    }
    for child in children {
        for source in &*child.reachable {
            visit(source)?;
        }
    }
    for module in env.modules() {
        let context = module
            .module
            .client_data()
            .and_then(|data| crate::starlark_label::defining_module_source_context(data.to_value()))
            .ok_or("original environment module has no effective source context")?;
        for source in &*context.reachable {
            visit(source)?;
        }
    }
    Ok(())
}
impl BzlModuleSourceContext {
    pub fn raw_load_manifest(&self) -> &BzlLoadManifest {
        &self.raw
    }
    pub fn environment_bindings(&self) -> &[BzlEnvironmentSourceBinding] {
        &self.bindings
    }
    pub fn effective_reachable(&self) -> &Arc<[BzlModuleSourceProvenance]> {
        &self.reachable
    }
    pub fn fingerprint(&self) -> &[u8; 32] {
        &self.fingerprint
    }
    pub(crate) fn sources(&self) -> &Arc<[(CompactString, BzlModuleSourceProvenance)]> {
        self.sources.raw()
    }
    pub(crate) fn certified_sources(&self) -> &crate::source_table::CertifiedSourceTable {
        &self.sources
    }
    pub(crate) fn producer_work(&self) -> Budget {
        self.producer_work
    }
    pub(crate) fn retention(&self) -> Budget {
        self.retention
    }
    pub(crate) fn new<'a>(
        raw: &'a BzlLoadManifest,
        env: &'a PreparedBzlEnvironment,
        children: impl Iterator<Item = &'a Self> + Clone,
    ) -> Result<Self, String> {
        let mut work = Budget::default();
        let mut count = 0usize;
        visit_sources(raw, env, children.clone(), |_| {
            work.charge(1, 0)?;
            count = count
                .checked_add(1)
                .ok_or("environment source count overflow")?;
            Ok(())
        })
        .map_err(|error| work.context("effective source count", error))?;
        let mut plan = plan::Plan::new(count, work)
            .map_err(|error| work.context("effective source plan peak", error))?;
        // New borrowed traversal handle, prepaid before its Clone/read.
        plan.work.charge(1, std::mem::size_of_val(&children))?;
        visit_sources(raw, env, children.clone(), |source| plan.observe(source))
            .map_err(|error| plan.work.context("effective source union work", error))?;
        let mut output = output::Output::new(plan.work);
        let inherited = crate::source_table::inherited::prepare(
            count,
            plan.rows.len(),
            children,
            env,
            &mut output.work,
        )?;
        output
            .forecast(raw, env, &plan.rows, inherited)
            .map_err(|error| {
                output
                    .work
                    .context("effective source output/fingerprint preflight", error)
            })?;
        let retention = output.retained;
        // Every persistent/transfer byte and full fingerprint traversal is
        // admitted jointly with the still-live borrowed plan BEFORE any clone.
        let reachable = plan
            .rows
            .iter()
            .map(|source| (**source).clone())
            .collect::<Vec<_>>();
        let bindings = env
            .exports
            .iter()
            .map(|e| BzlEnvironmentSourceBinding {
                global: e.selection.global.clone(),
                export: e.selection.export.clone(),
                module: e.selection.module.clone(),
            })
            .collect::<Vec<_>>();
        let mut hash = Sha256::new();
        hash.update(b"slug-bzl-effective-source-context-v1\0");
        hash.update(raw.fingerprint);
        hash.update((bindings.len() as u64).to_be_bytes());
        for binding in &bindings {
            for name in [&binding.global, &binding.export] {
                hash.update((name.len() as u64).to_be_bytes());
                hash.update(name.as_bytes());
            }
            crate::bzl_module::fingerprint_complete_identity(&mut hash, &binding.module);
        }
        hash.update((reachable.len() as u64).to_be_bytes());
        for p in &reachable {
            crate::bzl_module::fingerprint_complete_identity(&mut hash, &p.identity);
            hash.update(p.source_digest);
        }
        let sources = reachable
            .iter()
            .map(|p| {
                let name = p.identity.source.diagnostic_path();
                Ok((
                    CompactString::from(name.to_str().ok_or("non-UTF8 effective source filename")?),
                    p.clone(),
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let raw_sources = sources.into();
        let sources = output.finish_table(&raw_sources)?;
        let producer_work = output.work;
        Ok(Self {
            retention,
            raw: Arc::new(raw.clone()),
            bindings: bindings.into(),
            reachable: reachable.into(),
            fingerprint: hash.finalize().into(),
            sources,
            producer_work,
        })
    }
}
