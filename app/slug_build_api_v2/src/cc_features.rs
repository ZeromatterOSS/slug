//! Owned native C++ feature metadata, selection and expansion (Bazel 9.2 source).
//!
//! Expansion admits NOOP path mapping and no execution-time artifact metadata.
//! Handles have occurrence identity; their payloads have complete structural identity.
use std::hash::Hash;
use std::hash::Hasher;
use std::ops::Deref;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

mod artifact_naming;
mod metadata;
pub use metadata::*;
mod expansion;
pub use expansion::CcVariableAccess;
#[cfg(test)]
mod tests;

/// First-seen deduplication with order included in structural publication identity.
/// Hash full entries, never the SmallSet's cached weak entry hashes.
#[derive(Debug, Clone, Allocative)]
pub struct CcSet<T>(SmallSet<T>);
impl<T> CcSet<T> {
    pub fn new() -> Self {
        Self(SmallSet::new())
    }
}
impl<T> Default for CcSet<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T> Deref for CcSet<T> {
    type Target = SmallSet<T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T: Eq + Hash> CcSet<T> {
    pub fn insert(&mut self, value: T) -> bool {
        self.0.insert(value)
    }
}
impl<T: Eq + Hash> FromIterator<T> for CcSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}
impl<T> IntoIterator for CcSet<T> {
    type Item = T;
    type IntoIter = <SmallSet<T> as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
impl<T: PartialEq> PartialEq for CcSet<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_ordered(&other.0)
    }
}
impl<T: Eq> Eq for CcSet<T> {}
impl<T: Hash> Hash for CcSet<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.len().hash(state);
        for value in self.iter() {
            value.hash(state);
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CcFeatureError(pub String);
impl std::fmt::Display for CcFeatureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Invalid toolchain configuration: {}", self.0)
    }
}
impl std::error::Error for CcFeatureError {}
fn invalid(message: impl Into<String>) -> CcFeatureError {
    CcFeatureError(message.into())
}

#[derive(Debug, Clone, Dupe, Allocative)]
pub struct CcFeatureOccurrence(Arc<()>);
impl CcFeatureOccurrence {
    fn new() -> Self {
        Self(Arc::new(()))
    }
    pub(crate) fn pointer(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
}
impl PartialEq for CcFeatureOccurrence {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for CcFeatureOccurrence {}
impl Hash for CcFeatureOccurrence {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pointer().hash(state);
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct CcToolchainFeatureModel {
    pub features: Box<[CcFeature]>,
    pub action_configs: Box<[CcActionConfig]>,
    pub artifact_name_patterns: Box<[CcArtifactNamePattern]>,
    pub tools_directory: CompactString,
    pub exec_os: CcExecOs,
    pub default_features_and_action_configs: Box<[CompactString]>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct CcFeatureConfigurationModel {
    pub model: Arc<CcToolchainFeatureModel>,
    pub requested: CcSet<CompactString>,
    pub enabled_features: CcSet<CompactString>,
    pub enabled_action_configs: CcSet<CompactString>,
}

macro_rules! handle {
    ($name:ident, $model:ident) => {
        #[derive(Debug, Clone, Dupe, Allocative)]
        pub struct $name {
            occurrence: CcFeatureOccurrence,
            model: Arc<$model>,
        }
        impl $name {
            pub fn occurrence(&self) -> &CcFeatureOccurrence {
                &self.occurrence
            }
            pub fn model(&self) -> &Arc<$model> {
                &self.model
            }
        }
        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.occurrence == other.occurrence
            }
        }
        impl Eq for $name {}
        impl Hash for $name {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.occurrence.hash(state);
            }
        }
    };
}
handle!(CcToolchainFeatures, CcToolchainFeatureModel);
handle!(CcFeatureConfiguration, CcFeatureConfigurationModel);

impl CcToolchainFeatures {
    pub fn new(
        mut features: Vec<CcFeature>,
        mut action_configs: Vec<CcActionConfig>,
        artifact_name_patterns: Vec<CcArtifactNamePattern>,
        tools_directory: &str,
        exec_os: CcExecOs,
    ) -> Result<Self, CcFeatureError> {
        let tools_directory = normalize_posix_path(tools_directory)?;
        for feature in &mut features {
            for flags in &mut feature.flag_sets {
                flags.with_features =
                    cc_with_features_set(std::mem::take(&mut flags.with_features));
            }
            for env in &mut feature.env_sets {
                env.with_features = cc_with_features_set(std::mem::take(&mut env.with_features));
            }
        }
        for action in &mut action_configs {
            for flags in &mut action.flag_sets {
                flags.with_features =
                    cc_with_features_set(std::mem::take(&mut flags.with_features));
            }
        }
        let mut names = SmallMap::new();
        let mut action_names = SmallSet::new();
        // Native checks duplicate action names before duplicate selectable names.
        for action in &action_configs {
            validate_name(&action.action_name, false)?;
            if !action_names.insert(action.action_name.as_str()) {
                return Err(invalid(format!(
                    "multiple action configs for action '{}'",
                    action.action_name
                )));
            }
        }
        let mut defaults = Vec::new();
        for feature in &features {
            validate_name(&feature.name, feature.enabled)?;
            validate_feature(feature)?;
            if names.insert(feature.name.as_str(), ()).is_some() {
                return Err(duplicate_name(&feature.name));
            }
            if feature.enabled {
                defaults.push(feature.name.clone());
            }
        }
        for action in &action_configs {
            validate_action(action)?;
            if names.insert(action.action_name.as_str(), ()).is_some() {
                return Err(duplicate_name(&action.action_name));
            }
            if action.enabled {
                defaults.push(action.action_name.clone());
            }
        }
        let check_ref = |name: &str, from: &str| {
            if names.contains_key(name) {
                Ok(())
            } else {
                Err(invalid(format!(
                    "feature '{name}', which is referenced from feature '{from}', is not defined."
                )))
            }
        };
        for feature in &features {
            for requirements in &feature.requires {
                for name in requirements.iter() {
                    check_ref(name, &feature.name)?;
                }
            }
            for name in &feature.implies {
                check_ref(name, &feature.name)?;
            }
        }
        for action in &action_configs {
            for name in &action.implies {
                check_ref(name, &action.action_name)?;
            }
        }
        // Borrowed construction indexes must end before the owned lists move
        // into the retained immutable payload.
        drop(names);
        drop(action_names);
        // Native mapper drops default-equivalent patterns before checking duplicate overrides.
        let mut overrides = SmallSet::new();
        let mut normalized_patterns = Vec::new();
        for pattern in artifact_name_patterns {
            pattern.validate()?;
            if !pattern.is_default() {
                if !overrides.insert(pattern.category.clone()) {
                    return Err(invalid(format!(
                        "duplicate artifact name pattern '{}'",
                        pattern.category
                    )));
                }
                normalized_patterns.push(pattern);
            }
        }
        Ok(Self {
            occurrence: CcFeatureOccurrence::new(),
            model: Arc::new(CcToolchainFeatureModel {
                features: features.into_boxed_slice(),
                action_configs: action_configs.into_boxed_slice(),
                artifact_name_patterns: normalized_patterns.into_boxed_slice(),
                tools_directory,
                exec_os,
                default_features_and_action_configs: defaults.into_boxed_slice(),
            }),
        })
    }

    pub fn default_features_and_action_configs(&self) -> &[CompactString] {
        &self.model.default_features_and_action_configs
    }

    /// Bazel FeatureSelection: implication closure followed by monotone pruning.
    /// Defaults are explicitly not added here. Unknown requested names remain requested.
    pub fn configure_features(
        &self,
        requested: Vec<CompactString>,
    ) -> Result<CcFeatureConfiguration, CcFeatureError> {
        let requested: CcSet<_> = requested.into_iter().collect();
        let model = &self.model;
        let count = model.features.len() + model.action_configs.len();
        let mut by_name = SmallMap::new();
        for (index, feature) in model.features.iter().enumerate() {
            by_name.insert(feature.name.as_str(), index);
        }
        for (offset, action) in model.action_configs.iter().enumerate() {
            by_name.insert(action.action_name.as_str(), model.features.len() + offset);
        }
        let mut implies: Vec<Vec<usize>> = Vec::with_capacity(count);
        let mut implied_by = vec![Vec::new(); count];
        for feature in &model.features {
            implies.push(
                feature
                    .implies
                    .iter()
                    .map(|name| *by_name.get(name.as_str()).expect("validated reference"))
                    .collect(),
            );
        }
        for action in &model.action_configs {
            implies.push(
                action
                    .implies
                    .iter()
                    .map(|name| *by_name.get(name.as_str()).expect("validated reference"))
                    .collect(),
            );
        }
        for (from, children) in implies.iter().enumerate() {
            for child in children {
                implied_by[*child].push(from);
            }
        }
        let mut directly_requested = vec![false; count];
        let mut enabled = vec![false; count];
        let mut stack = Vec::new();
        for name in requested.iter() {
            if let Some(index) = by_name.get(name.as_str()) {
                directly_requested[*index] = true;
                stack.push(*index);
            }
        }
        while let Some(index) = stack.pop() {
            if !enabled[index] {
                enabled[index] = true;
                stack.extend(implies[index].iter().copied());
            }
        }
        // Satisfaction predicates are monotone under removal. Rechecking in definition
        // order reaches the same greatest fixed point as native reverse-edge recursion,
        // without consuming the Rust call stack on implication cycles.
        loop {
            let mut changed = false;
            for index in 0..count {
                if !enabled[index] {
                    continue;
                }
                let supported = (directly_requested[index]
                    || implied_by[index].iter().any(|parent| enabled[*parent]))
                    && implies[index].iter().all(|child| enabled[*child])
                    && (index >= model.features.len()
                        || model.features[index].requires.is_empty()
                        || model.features[index].requires.iter().any(|all| {
                            all.iter().all(|name| {
                                enabled[*by_name.get(name.as_str()).expect("validated reference")]
                            })
                        }));
                if !supported {
                    enabled[index] = false;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        // Lists, not sets: a feature providing a symbol twice collides with itself.
        let mut providers: SmallMap<&str, Vec<&str>> = SmallMap::new();
        for (index, feature) in model.features.iter().enumerate() {
            for symbol in &feature.provides {
                let entries = providers.entry(symbol.as_str()).or_default();
                if enabled[index] {
                    entries.push(feature.name.as_str());
                }
            }
        }
        for (symbol, providers) in &providers {
            if providers.len() > 1 {
                return Err(invalid(format!(
                    "Symbol {symbol} is provided by all of the following features: {}",
                    providers.join(" ")
                )));
            }
        }
        Ok(CcFeatureConfiguration {
            occurrence: CcFeatureOccurrence::new(),
            model: Arc::new(CcFeatureConfigurationModel {
                model: Arc::clone(model),
                requested,
                enabled_features: model
                    .features
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| enabled[*i])
                    .map(|(_, f)| f.name.clone())
                    .collect(),
                enabled_action_configs: model
                    .action_configs
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| enabled[model.features.len() + *i])
                    .map(|(_, a)| a.action_name.clone())
                    .collect(),
            }),
        })
    }
}
impl CcFeatureConfiguration {
    pub fn is_requested(&self, name: &str) -> bool {
        self.model.requested.contains(name)
    }
    pub fn is_enabled(&self, name: &str) -> bool {
        self.model.enabled_features.contains(name)
    }
}
fn duplicate_name(name: &str) -> CcFeatureError {
    invalid(format!(
        "feature or action config '{name}' was specified multiple times."
    ))
}
