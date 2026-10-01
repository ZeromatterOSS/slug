//! Phase preparation discovers complete retained authorities without flattening
//! depsets. Helper code remains in Loading bindings, outside semantic values.
use std::sync::Arc;

use compact_str::CompactString;
use slug_build_api_v2::AnalysisDepset;
use slug_build_api_v2::AnalysisDepsetSuccessor;
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::AnalysisValueKind;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::RustToolchainCallableSourceAuthority;
use slug_loading_v2::BzlModuleSourceProvenance;
use slug_loading_v2::PreparedRustToolchainHelperBinding;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

#[derive(Default)]
pub(crate) struct RustHelperAuthorities {
    authorities: SmallSet<Arc<RustToolchainCallableSourceAuthority>>,
    depsets: SmallSet<AnalysisDepset>,
    variables: SmallSet<usize>,
}

impl RustHelperAuthorities {
    pub(crate) fn providers(&mut self, providers: &ProviderCollection) {
        for provider in providers.occurrences() {
            for value in provider.fields().values() {
                self.value(value);
            }
        }
    }

    pub(crate) fn value(&mut self, value: &AnalysisValue) {
        let mut stack = vec![value.clone()];
        while let Some(value) = stack.pop() {
            match value.kind() {
                AnalysisValueKind::RustToolchainCallable(callable) => {
                    let model = callable.model();
                    self.authorities.insert(model.source.clone());
                    stack.extend([
                        model.cc_toolchain.clone(),
                        model.feature_configuration.clone(),
                        model.experimental_link_std_dylib.clone(),
                        model.rust_std.clone(),
                    ]);
                }
                AnalysisValueKind::CcToolchainCallable(callable) => {
                    stack.push(callable.model().capture.clone())
                }
                AnalysisValueKind::CcToolchainVariables(variables) => {
                    let model = variables.model();
                    // Model addresses are traversal memo keys only, never retained identity.
                    if self.variables.insert(model as *const _ as usize) {
                        stack.extend(
                            model
                                .parent
                                .iter()
                                .cloned()
                                .map(AnalysisValue::cc_toolchain_variables),
                        );
                        stack.extend(model.locals.iter().map(|(_, value)| value.clone()));
                        for (_, entry) in &model.cache {
                            stack.extend(entry.values().iter().cloned());
                        }
                    }
                }
                AnalysisValueKind::ConfiguredTarget(target) => {
                    for provider in target.providers().occurrences() {
                        stack.extend(provider.fields().values().cloned());
                    }
                }
                AnalysisValueKind::Provider(provider) => {
                    stack.extend(provider.fields().values().cloned())
                }
                AnalysisValueKind::List(values) | AnalysisValueKind::Tuple(values) => {
                    stack.extend(values.iter().cloned())
                }
                AnalysisValueKind::Dictionary(values) => {
                    for (key, value) in values {
                        stack.push(key.clone());
                        stack.push(value.clone());
                    }
                }
                AnalysisValueKind::Struct(fields) => stack.extend(fields.values().cloned()),
                AnalysisValueKind::Depset(depset) => {
                    if self.depsets.insert(depset.clone()) {
                        for successor in depset.successors() {
                            stack.push(match successor {
                                AnalysisDepsetSuccessor::Direct(value) => value.clone(),
                                AnalysisDepsetSuccessor::Transitive(child) => {
                                    AnalysisValue::depset(child)
                                }
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }

    pub(crate) fn into_authorities(
        self,
    ) -> impl Iterator<Item = Arc<RustToolchainCallableSourceAuthority>> {
        self.authorities.into_iter()
    }
}

pub(crate) fn merge_helper_sources(
    original: &[(CompactString, BzlModuleSourceProvenance)],
    bindings: &[PreparedRustToolchainHelperBinding],
) -> Result<Arc<[(CompactString, BzlModuleSourceProvenance)]>, String> {
    let mut filenames = SmallMap::new();
    let mut labels = SmallMap::new();
    for (filename, source) in original.iter().chain(
        bindings
            .iter()
            .flat_map(|binding| binding.source_identities().iter()),
    ) {
        if labels
            .get(&source.identity.label)
            .is_some_and(|previous| previous != source)
        {
            return Err(format!(
                "conflicting Rust helper canonical source {}",
                source.identity.label
            ));
        }
        labels.insert(source.identity.label.clone(), source.clone());
        if let Some(previous) = filenames.get(filename) {
            if previous != source {
                return Err(format!(
                    "conflicting Rust helper source filename {filename}"
                ));
            }
        } else {
            filenames.insert(filename.clone(), source.clone());
        }
    }
    Ok(filenames.into_iter().collect::<Vec<_>>().into())
}

#[cfg(test)]
mod tests;
