//! Compose already-owned lexical source authorities before ordinary evaluation.
//! Display filenames never select an authority or trigger a helper load.
use slug_build_api_v2::AnalysisDepsetSuccessor;
use slug_loading_v2::BzlModuleSourceProvenance;
use starlark_map::unordered_map::UnorderedMap;

use super::*;

struct Sources {
    rows: Vec<(CompactString, BzlModuleSourceProvenance)>,
    by_identity: UnorderedMap<BzlModuleIdentity, usize>,
    bytes: usize,
    remaining: usize,
}
impl Sources {
    fn step(&mut self) -> Result<(), String> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or("source authority composition visit limit")?;
        Ok(())
    }
    fn charge(&mut self, bytes: usize) -> Result<(), String> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|n| *n <= slug_build_api_v2::published_value::BYTE_LIMIT)
            .ok_or("source authority composition byte limit")?;
        Ok(())
    }
    fn insert(&mut self, display: &str, source: &BzlModuleSourceProvenance) -> Result<(), String> {
        self.step()?;
        if let Some(index) = self.by_identity.get(&source.identity) {
            if self.rows[*index].1 != *source {
                return Err("conflicting original-module source authority".into());
            }
            return Ok(());
        }
        let identity = &source.identity;
        let mut bytes = identity
            .workspace_path
            .as_os_str()
            .len()
            .saturating_add(identity.label.package().repo().as_str().len())
            .saturating_add(identity.label.package().package().as_str().len())
            .saturating_add(identity.label.target().as_str().len());
        for (a, c) in &*identity.repository_mapping {
            self.step()?;
            bytes = bytes
                .saturating_add(a.as_str().len())
                .saturating_add(c.as_str().len());
        }
        self.charge(
            bytes
                .saturating_mul(2)
                .saturating_add(display.len())
                .saturating_add(std::mem::size_of::<(
                    BzlModuleIdentity,
                    BzlModuleSourceProvenance,
                    CompactString,
                    usize,
                )>()),
        )?;
        self.by_identity
            .insert(source.identity.clone(), self.rows.len());
        self.rows.push((display.into(), source.clone()));
        Ok(())
    }
    fn value(&mut self, value: &AnalysisValue, depth: usize) -> Result<(), String> {
        self.step()?;
        if depth > slug_build_api_v2::published_value::DEPTH_LIMIT {
            return Err("source authority composition depth limit".into());
        }
        if let Some(origin) = value.original_origin() {
            for authority in origin.arena().sources() {
                for source in &*authority.data().reachable {
                    // Bound this temporary native identity conversion before its
                    // owned path is copied; insert separately budgets retention.
                    self.charge(source.identity.workspace_path.as_os_str().len())?;
                    let identity = BzlModuleIdentity {
                        label: source.identity.label.clone(),
                        workspace_path: source.identity.workspace_path.clone(),
                        repository_mapping: source.identity.repository_mapping.clone(),
                    };
                    let display = identity.workspace_path.to_string_lossy().into_owned();
                    self.insert(
                        &display,
                        &BzlModuleSourceProvenance::new(identity, source.digest),
                    )?;
                }
            }
            return Ok(());
        }
        match value.kind() {
            AnalysisValueKind::List(v) | AnalysisValueKind::Tuple(v) => {
                for child in v {
                    self.value(child, depth + 1)?;
                }
            }
            AnalysisValueKind::Dictionary(v) => {
                for (k, v) in v {
                    self.value(k, depth + 1)?;
                    self.value(v, depth + 1)?;
                }
            }
            AnalysisValueKind::Struct(v) => {
                for child in v.values() {
                    self.value(child, depth + 1)?;
                }
            }
            AnalysisValueKind::Provider(v) => {
                for child in v.fields().values() {
                    self.value(child, depth + 1)?;
                }
            }
            AnalysisValueKind::ConfiguredTarget(v) => self.providers(v.providers(), depth + 1)?,
            AnalysisValueKind::Depset(v) => self.depset(v, depth + 1)?,
            AnalysisValueKind::CcToolchainVariables(v) => {
                let model = v.model();
                if let Some(parent) = &model.parent {
                    self.value(
                        &AnalysisValue::cc_toolchain_variables(parent.clone()),
                        depth + 1,
                    )?;
                }
                for (_, child) in &model.locals {
                    self.value(child, depth + 1)?;
                }
                for (_, entry) in &model.cache {
                    for child in entry.values() {
                        self.value(child, depth + 1)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn depset(&mut self, value: &AnalysisDepset, depth: usize) -> Result<(), String> {
        self.step()?;
        if depth > slug_build_api_v2::published_value::DEPTH_LIMIT {
            return Err("source authority depset depth limit".into());
        }
        for child in value.successors() {
            match child {
                AnalysisDepsetSuccessor::Direct(v) => self.value(v, depth + 1)?,
                AnalysisDepsetSuccessor::Transitive(v) => self.depset(&v, depth + 1)?,
            }
        }
        Ok(())
    }
    fn providers(&mut self, values: &ProviderCollection, depth: usize) -> Result<(), String> {
        for (_, value) in values.iter() {
            self.step()?;
            if let ProviderValue::Occurrence(value) = value {
                self.value(&AnalysisValue::provider(value.clone()), depth + 1)?;
            }
        }
        Ok(())
    }
}

pub(super) fn compose(
    implementation: &slug_loading_v2::package::StarlarkRuleImplementation,
    dependencies: &[PreparedDependency],
    attributes: &[PreparedConfiguredAttribute],
    groups: &ConfiguredExecGroupCollection,
) -> Result<Arc<[(CompactString, BzlModuleSourceProvenance)]>, String> {
    let mut result = Sources {
        rows: Vec::new(),
        by_identity: UnorderedMap::default(),
        bytes: 0,
        remaining: slug_build_api_v2::published_value::NODE_LIMIT,
    };
    for (display, source) in &**implementation.source_identities_by_filename() {
        result.insert(display, source)?;
    }
    for dependency in dependencies {
        result.providers(&dependency.providers, 0)?;
    }
    for attribute in attributes {
        result.value(&attribute.value, 0)?;
    }
    for group in groups.rows() {
        if let Some(toolchain) = group.action_context().toolchain() {
            for row in toolchain.rows() {
                if let Some(selected) = row.selected() {
                    result.value(&AnalysisValue::provider(selected.info().clone()), 0)?;
                }
            }
        }
    }
    Ok(result.rows.into())
}
