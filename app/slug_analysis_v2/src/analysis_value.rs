/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the above-listed
 * licenses.
 */

use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::hash::Hash;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use num_bigint::BigInt;
use num_bigint::Sign;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisConfiguredTargetKey;
use slug_build_api_v2::AnalysisDepset;
use slug_build_api_v2::AnalysisDepsetGraphInput;
use slug_build_api_v2::AnalysisDepsetGraphNode;
use slug_build_api_v2::AnalysisDepsetGraphRow;
#[cfg(test)]
use slug_build_api_v2::AnalysisDepsetSuccessor;
use slug_build_api_v2::AnalysisNumber;
use slug_build_api_v2::AnalysisTargetIdentity;
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::AnalysisValueKind;
use slug_build_api_v2::ConfiguredTargetValue;
use slug_build_api_v2::Depset;
use slug_build_api_v2::FilesToRunProvider;
use slug_build_api_v2::OutputGroupInfo;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::ProviderIdentity;
use slug_build_api_v2::ProviderOccurrence;
use slug_build_api_v2::ProviderValue;
use slug_build_api_v2::RetainedRunfiles;
use slug_build_api_v2::RunfilesSymlink;
use slug_build_api_v2::RunfilesSymlinkDepset;
use slug_loading_v2::alloc_budgeted_starlark_label;
use slug_loading_v2::provider::StarlarkDepset;
use slug_loading_v2::provider::StarlarkDepsetSuccessorGen;
use slug_loading_v2::provider::StarlarkOutputGroupInfo;
use slug_loading_v2::provider::StarlarkToolchainInfo;
use slug_loading_v2::provider::alloc_frozen_starlark_label;
use slug_loading_v2::provider::alloc_starlark_depset;
use slug_loading_v2::provider::alloc_starlark_depset_parts;
use slug_loading_v2::provider::alloc_starlark_user_provider;
use slug_loading_v2::provider::configured_target_provider_identity;
use slug_loading_v2::provider::starlark_label;
use slug_loading_v2::provider::starlark_user_provider_fields;
use slug_loading_v2::subrule_invocation::AnalysisArtifactValue;
use starlark::any::ProvidesStaticType;
use starlark::collections::StarlarkHasher;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::eval::ExecutionBudget;
use starlark::eval::ExecutionLimit;
use starlark::starlark_module;
use starlark::values::FrozenHeap;
use starlark::values::FrozenStringValue;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::UnpackValue;
use starlark::values::Value;
use starlark::values::ValueIdentity;
use starlark::values::ValueLike;
use starlark::values::dict::AllocDict;
use starlark::values::dict::DictRef;
use starlark::values::float::StarlarkFloat;
use starlark::values::list::AllocImmutableList;
use starlark::values::list::ListRef;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::starlark_value;
use starlark::values::structs::AllocStruct;
use starlark::values::structs::StructRef;
use starlark::values::tuple::AllocTuple;
use starlark::values::tuple::TupleRef;
use starlark_map::Hashed;
use starlark_map::small_map::BoundedMapError;
use starlark_map::small_map::SmallMap;

use crate::key::ConfiguredNodeKey;
use crate::key::ConfiguredTargetKey;

pub(crate) fn analysis_configured_key(key: &ConfiguredTargetKey) -> AnalysisConfiguredTargetKey {
    let value = AnalysisConfiguredTargetKey::new(
        key.label().clone(),
        key.configuration().complete_identity_bytes(),
    );
    match key.toolchain_execution_platform() {
        Some(platform) => value.with_toolchain_execution_platform(platform.clone()),
        None => value,
    }
}

#[cfg(test)]
pub(crate) fn test_artifact_root() -> slug_build_api_v2::DerivedArtifactRoot {
    use std::sync::Arc;

    use slug_configuration_v2::native::host::AutoCpuToken;
    use slug_configuration_v2::native::host::HostConversionInputs;
    use slug_configuration_v2::native::host::HostPathFlavor;
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap();
    slug_build_api_v2::DerivedArtifactRoot::bin(
        slug_configuration_v2::SlugConfiguration::default_target(&host).unwrap(),
    )
    .unwrap()
}

#[cfg(test)]
#[path = "analysis_value/artifact_root_tests.rs"]
mod artifact_root_tests;

mod native_depset;
pub(crate) mod published;

#[cfg(test)]
#[path = "analysis_value/budgeted_projection_tests.rs"]
mod budgeted_projection_tests;

mod empty_instrumented_files;
use empty_instrumented_files::EmptyInstrumentedFiles;

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct BuiltinProviderView {
    identity: ProviderIdentity,
    fields: SmallMap<CompactString, FrozenValue>,
}

impl BuiltinProviderView {
    fn fields_from_value(
        value: Value<'_>,
    ) -> Option<(ProviderIdentity, Vec<(CompactString, Value<'_>)>)> {
        let view = Self::from_value(value)?;
        Some((
            view.identity.clone(),
            view.fields
                .iter()
                .map(|(name, value)| (name.clone(), value.to_value()))
                .collect(),
        ))
    }
}

impl fmt::Display for BuiltinProviderView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(...)", self.identity.name())
    }
}

starlark::starlark_simple_value!(BuiltinProviderView);
#[starlark_value(type = "provider")]
impl<'v> StarlarkValue<'v> for BuiltinProviderView {
    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        let Some(other) = Self::from_value(other) else {
            return Ok(false);
        };
        if self.identity != other.identity || self.fields.len() != other.fields.len() {
            return Ok(false);
        }
        for (name, value) in &self.fields {
            let Some(other) = other.fields.get(name) else {
                return Ok(false);
            };
            if !value.to_value().equals(other.to_value())? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn get_attr(&self, name: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        self.fields.get(name).map(|value| value.to_value())
    }

    fn get_attr_bounded(
        &self,
        name: &str,
        _heap: Heap<'v>,
        budget: &ExecutionBudget,
    ) -> starlark::Result<Option<Value<'v>>> {
        budget.charge_text_work(name.len())?;
        let name = Hashed::new(name);
        self.fields
            .get_hashed_bounded(
                name.hash(),
                |candidate| {
                    if candidate.len() != name.key().len() {
                        return Ok(false);
                    }
                    budget.charge_text_work(candidate.len())?;
                    Ok(candidate.as_str() == *name.key())
                },
                |work, bytes| budget.charge(work, bytes),
            )
            .map(|value| value.map(|value| value.to_value()))
            .map_err(|error| projection_map_error(budget, error))
    }

    fn dir_attr(&self) -> Vec<String> {
        let mut fields = self
            .fields
            .keys()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        fields.sort();
        fields
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct StarlarkFilesToRunProvider {
    retained: FilesToRunProvider,
    executable: FrozenValue,
    runfiles_manifest: FrozenValue,
    repo_mapping_manifest: FrozenValue,
}

impl fmt::Display for StarlarkFilesToRunProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FilesToRunProvider(...)")
    }
}

starlark::starlark_simple_value!(StarlarkFilesToRunProvider);
#[starlark_value(type = "FilesToRunProvider")]
impl<'v> StarlarkValue<'v> for StarlarkFilesToRunProvider {
    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other).is_some_and(|other| self.retained == other.retained))
    }

    fn get_attr(&self, name: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match name {
            "executable" => Some(self.executable.to_value()),
            "runfiles_manifest" => Some(self.runfiles_manifest.to_value()),
            "repo_mapping_manifest" => Some(self.repo_mapping_manifest.to_value()),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec![
            "executable".to_owned(),
            "repo_mapping_manifest".to_owned(),
            "runfiles_manifest".to_owned(),
        ]
    }
}

pub(crate) fn is_files_to_run_provider(value: Value<'_>) -> bool {
    StarlarkFilesToRunProvider::from_value(value).is_some()
}

pub(crate) fn files_to_run_provider<'v>(value: Value<'v>) -> Option<&'v FilesToRunProvider> {
    StarlarkFilesToRunProvider::from_value(value).map(|value| &value.retained)
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct StarlarkRunfilesSymlink {
    retained: RunfilesSymlink,
    path: FrozenValue,
    target_file: FrozenValue,
}

impl fmt::Display for StarlarkRunfilesSymlink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SymlinkEntry(path = {:?})", self.retained.path)
    }
}

starlark::starlark_simple_value!(StarlarkRunfilesSymlink);
#[starlark_value(type = "SymlinkEntry")]
impl<'v> StarlarkValue<'v> for StarlarkRunfilesSymlink {
    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.retained.hash(hasher);
        Ok(())
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other).is_some_and(|other| self.retained == other.retained))
    }

    fn get_attr(&self, name: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match name {
            "path" => Some(self.path.to_value()),
            "target_file" => Some(self.target_file.to_value()),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec!["path".to_owned(), "target_file".to_owned()]
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct StarlarkRunfiles {
    occurrence: slug_build_api_v2::RunfilesOccurrence,
    retained: RetainedRunfiles,
    files: FrozenValue,
    symlinks: FrozenValue,
    root_symlinks: FrozenValue,
    empty_filenames: FrozenValue,
}

impl fmt::Display for StarlarkRunfiles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("runfiles(...)")
    }
}

starlark::starlark_simple_value!(StarlarkRunfiles);
#[starlark_value(type = "runfiles")]
impl<'v> StarlarkValue<'v> for StarlarkRunfiles {
    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        // Bazel's native Runfiles is immutable/hashable. A coarse Slug-native
        // hash preserves wrapper/EMPTY equality without making addresses part
        // of retained semantic identity.
        27u8.hash(hasher);
        Ok(())
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other).is_some_and(|other| {
            self.occurrence == other.occurrence
                || (self.retained.is_empty() && other.retained.is_empty())
        }))
    }

    fn get_attr(&self, name: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match name {
            "files" => Some(self.files.to_value()),
            "symlinks" => Some(self.symlinks.to_value()),
            "root_symlinks" => Some(self.root_symlinks.to_value()),
            "empty_filenames" => Some(self.empty_filenames.to_value()),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec![
            "empty_filenames".to_owned(),
            "files".to_owned(),
            "root_symlinks".to_owned(),
            "symlinks".to_owned(),
        ]
    }

    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(starlark_runfiles_methods)
    }
}

#[starlark_module]
fn starlark_runfiles_methods(builder: &mut MethodsBuilder) {
    fn merge<'v>(
        this: Value<'v>,
        #[starlark(require = pos)] other: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let this_runfiles = StarlarkRunfiles::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("runfiles.merge receiver is not runfiles"))?;
        let other_runfiles = StarlarkRunfiles::from_value(other)
            .ok_or_else(|| anyhow::anyhow!("runfiles.merge requires a runfiles value"))?;
        if this_runfiles.retained.is_empty() {
            return Ok(other);
        }
        if other_runfiles.retained.is_empty() {
            return Ok(this);
        }
        let retained = this_runfiles.retained.merge(&other_runfiles.retained)?;
        materialize_runfiles(&retained, eval.frozen_heap())
            .map(|value| value.to_value())
            .map_err(anyhow::Error::msg)
    }

    fn merge_all<'v>(
        this: Value<'v>,
        #[starlark(require = pos)] other: UnpackListOrTuple<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let mut values = Vec::with_capacity(other.items.len() + 1);
        values.push(this);
        values.extend(other.items.iter().copied());
        let mut nonempty = Vec::new();
        for value in values {
            let runfiles = StarlarkRunfiles::from_value(value)
                .ok_or_else(|| anyhow::anyhow!("runfiles.merge_all requires runfiles values"))?;
            if !runfiles.retained.is_empty() {
                nonempty.push((value, &runfiles.retained));
            }
        }
        match nonempty.as_slice() {
            [] => Ok(this),
            [(value, _)] => Ok(*value),
            _ => {
                let retained =
                    RetainedRunfiles::merge_all(nonempty.iter().map(|(_, retained)| *retained))?;
                materialize_runfiles(&retained, eval.frozen_heap())
                    .map(|value| value.to_value())
                    .map_err(anyhow::Error::msg)
            }
        }
    }
}

pub(crate) fn retained_runfiles(value: Value<'_>) -> Option<&RetainedRunfiles> {
    StarlarkRunfiles::from_value(value).map(|value| &value.retained)
}

pub(crate) fn lower_runfiles_symlink_depset(
    root: Value<'_>,
    path: &str,
) -> Result<RunfilesSymlinkDepset, String> {
    let mut memo = HashMap::new();
    let mut visiting = HashSet::new();
    let mut stack = vec![(root, path.to_owned(), false)];
    while let Some((value, path, exiting)) = stack.pop() {
        let identity = value.identity();
        if memo.contains_key(&identity) {
            continue;
        }
        if !exiting {
            if !visiting.insert(identity) {
                return Err(format!("{path}: cyclic SymlinkEntry depset"));
            }
            let (_, _, _, _, _, successors) = StarlarkDepset::parts_from_value(value)
                .ok_or_else(|| format!("{path} must be a depset of SymlinkEntry values"))?;
            stack.push((value, path.clone(), true));
            for (index, successor) in successors.into_iter().enumerate().rev() {
                if let StarlarkDepsetSuccessorGen::Transitive(child) = successor {
                    stack.push((child, format!("{path}.successor[{index}]"), false));
                }
            }
            continue;
        }
        let (order, _, _, _, _, successors) = StarlarkDepset::parts_from_value(value)
            .ok_or_else(|| format!("{path} must be a depset of SymlinkEntry values"))?;
        let mut direct = Vec::new();
        let mut transitive = Vec::new();
        for (index, successor) in successors.into_iter().enumerate() {
            match successor {
                StarlarkDepsetSuccessorGen::Direct(value) => {
                    direct.push(
                        StarlarkRunfilesSymlink::from_value(value)
                            .map(|value| value.retained.clone())
                            .ok_or_else(|| {
                                format!("{path}.successor[{index}] must be a SymlinkEntry value")
                            })?,
                    );
                }
                StarlarkDepsetSuccessorGen::Transitive(value) => transitive.push(
                    memo.get(&value.identity())
                        .cloned()
                        .ok_or_else(|| format!("{path}.successor[{index}] was not materialized"))?,
                ),
            }
        }
        visiting.remove(&identity);
        let result =
            Depset::new(order, direct, transitive).map_err(|error| format!("{path}: {error}"))?;
        memo.insert(identity, result.clone());
    }
    memo.remove(&root.identity())
        .ok_or_else(|| format!("{path} was not materialized"))
}

pub(crate) fn materialize_runfiles(
    value: &RetainedRunfiles,
    heap: &FrozenHeap,
) -> Result<FrozenValue, String> {
    AnalysisValueMaterializer::new(heap).runfiles(value)
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct AnalysisConfiguredTargetValue {
    retained: ConfiguredTargetValue,
    providers: SmallMap<ProviderIdentity, FrozenValue>,
}

impl AnalysisConfiguredTargetValue {
    pub(crate) fn providers(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ProviderIdentity, FrozenValue)> {
        self.providers
            .iter()
            .map(|(identity, value)| (identity, *value))
    }

    fn files(&self) -> Option<FrozenValue> {
        let default = self
            .providers
            .get(&ProviderIdentity::builtin("DefaultInfo"))?;
        BuiltinProviderView::from_value(default.to_value())?
            .fields
            .get("files")
            .copied()
    }

    fn bounded_files<'v>(&self, budget: &ExecutionBudget) -> starlark::Result<Option<Value<'v>>> {
        for (identity, value) in &self.providers {
            budget.charge(1, 0)?;
            if !identity.is_builtin("DefaultInfo") {
                continue;
            }
            if let Some(default) = BuiltinProviderView::from_value(value.to_value()) {
                for (name, value) in &default.fields {
                    budget.charge(1, 0)?;
                    if name.as_str() == "files" {
                        return Ok(Some(value.to_value()));
                    }
                }
            }
            break;
        }
        Ok(None)
    }
}

impl fmt::Display for AnalysisConfiguredTargetValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.retained.identity().label().fmt(f)
    }
}

starlark::starlark_simple_value!(AnalysisConfiguredTargetValue);
#[starlark_value(type = "Target")]
impl<'v> StarlarkValue<'v> for AnalysisConfiguredTargetValue {
    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.retained.identity().hash(hasher);
        Ok(())
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other)
            .is_some_and(|other| self.retained.identity() == other.retained.identity()))
    }

    fn at(&self, index: Value<'v>, _heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let identity = configured_target_provider_identity(index)?.ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!(
                "provider lookup requires an exported provider constructor"
            ))
        })?;
        self.providers
            .get(&identity)
            .map(|value| value.to_value())
            .ok_or_else(|| {
                starlark::Error::new_other(anyhow::anyhow!(
                    "target does not provide {}",
                    identity.name()
                ))
            })
    }

    fn is_in(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(configured_target_provider_identity(other)?
            .is_some_and(|identity| self.providers.contains_key(&identity)))
    }

    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match name {
            "files" => self.files().map(FrozenValue::to_value),
            "label" => Some(slug_loading_v2::provider::alloc_starlark_label(
                heap,
                self.retained.identity().label().clone(),
            )),
            _ => None,
        }
    }

    fn get_attr_bounded(
        &self,
        name: &str,
        heap: Heap<'v>,
        budget: &ExecutionBudget,
    ) -> starlark::Result<Option<Value<'v>>> {
        match name {
            "files" => self.bounded_files(budget),
            "label" => Ok(Some(alloc_budgeted_starlark_label(
                heap,
                self.retained.identity().label(),
            )?)),
            _ => Ok(None),
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        if self.files().is_some() {
            vec!["files".to_owned(), "label".to_owned()]
        } else {
            vec!["label".to_owned()]
        }
    }
}

pub(crate) struct AnalysisValueMaterializer<'a> {
    heap: &'a FrozenHeap,
    budget: Option<&'a ExecutionBudget>,
    depth: usize,
    // Owning keys are phase scratch: aliases survive all roots handled by this
    // materializer, and temporary native allocations cannot cause pointer ABA.
    containers: SmallMap<slug_build_api_v2::analysis_value::AnalysisContainerIdentity, FrozenValue>,
    cc_variables: SmallMap<slug_build_api_v2::cc_variables::CcVariablesOccurrence, FrozenValue>,
    depsets: native_depset::RetainedMemo,
    file_depsets: native_depset::Memo<String>,
    symlink_depsets: native_depset::Memo<RunfilesSymlink>,
}

impl<'a> AnalysisValueMaterializer<'a> {
    pub(crate) fn new(heap: &'a FrozenHeap) -> Self {
        Self::new_with_budget(heap, None)
    }

    pub(crate) fn new_with_budget(
        heap: &'a FrozenHeap,
        budget: Option<&'a ExecutionBudget>,
    ) -> Self {
        Self {
            heap,
            budget,
            depth: 0,
            containers: SmallMap::new(),
            cc_variables: SmallMap::new(),
            depsets: SmallMap::new(),
            file_depsets: SmallMap::new(),
            symlink_depsets: SmallMap::new(),
        }
    }

    fn project_original(
        &self,
        original: &slug_build_api_v2::PublishedValueRef,
    ) -> starlark::Result<FrozenValue> {
        if let Some(budget) = self.budget {
            budget.charge(1, 0)?;
            let root = published::owned_original_root(original).map_err(projection_error)?;
            // The admitted attachment keeps the exact publication root and its heap alive.
            Ok(root
                .owned_value_budgeted(self.heap, budget)?
                .unpack_frozen()
                .expect("an owned publication root is frozen"))
        } else {
            published::materialize_original(original, self.heap).map_err(projection_error)
        }
    }

    fn project_string(&self, text: &str) -> starlark::Result<FrozenStringValue> {
        Ok(match self.budget {
            Some(budget) => self.heap.alloc_budgeted_str(budget, text)?,
            None => self.heap.alloc_str(text),
        })
    }

    fn project_artifact(&self, artifact: &AnalysisArtifact) -> starlark::Result<FrozenValue> {
        Ok(match self.budget {
            Some(budget) => {
                AnalysisArtifactValue::alloc_frozen_budgeted(self.heap, budget, artifact)?
            }
            None => self
                .heap
                .alloc(AnalysisArtifactValue::new(artifact.clone())),
        })
    }

    fn project_label(
        &self,
        label: &slug_identity_v2::CanonicalLabel,
    ) -> starlark::Result<FrozenValue> {
        Ok(match self.budget {
            Some(budget) => slug_loading_v2::provider::alloc_budgeted_frozen_starlark_label(
                self.heap, budget, label,
            )?,
            None => alloc_frozen_starlark_label(self.heap, label.clone()),
        })
    }

    fn project_simple<T>(&self, build: impl FnOnce() -> T) -> starlark::Result<FrozenValue>
    where
        T: StarlarkValue<'static> + ProvidesStaticType<'static, StaticType = T> + Send + Sync,
    {
        Ok(match self.budget {
            Some(budget) => self.heap.alloc_budgeted_simple(budget, build)?,
            None => self.heap.alloc_simple(build()),
        })
    }

    fn project_output_group_info(
        &mut self,
        info: &OutputGroupInfo,
    ) -> starlark::Result<FrozenValue> {
        if let Some(budget) = self.budget {
            return StarlarkOutputGroupInfo::alloc_budgeted(self.heap, budget, info, |files| {
                self.project_depset(files)
            });
        }
        let fields = info
            .groups()
            .iter()
            .map(|(name, files)| {
                Ok((
                    CompactString::new(name.as_str()),
                    self.project_depset(files)?,
                ))
            })
            .collect::<starlark::Result<Vec<_>>>()?;
        Ok(StarlarkOutputGroupInfo::alloc(self.heap, fields))
    }

    #[cfg(test)]
    fn provider(&mut self, occurrence: &ProviderOccurrence) -> Result<FrozenValue, String> {
        self.project_provider(occurrence)
            .map_err(|error| error.without_diagnostic().to_string())
    }

    fn project_provider(
        &mut self,
        occurrence: &ProviderOccurrence,
    ) -> starlark::Result<FrozenValue> {
        if let Some(original) = occurrence.original_origin() {
            return self.project_original(original);
        }
        if let Some(value) = FilesToRunProvider::borrow_from_occurrence(occurrence) {
            return self.files_to_run(value);
        }
        if let Some(budget) = self.budget {
            let heap = self.heap;
            if occurrence.identity().user_id().is_some() {
                return slug_loading_v2::provider::alloc_budgeted_starlark_user_provider(
                    heap,
                    budget,
                    occurrence,
                    |value| self.project_value(value),
                );
            }
            if occurrence.identity().is_builtin("ToolchainInfo") {
                return StarlarkToolchainInfo::alloc_budgeted(heap, budget, occurrence, |value| {
                    self.project_value(value)
                });
            }
            if occurrence.identity().is_builtin("InstrumentedFilesInfo") {
                // This class has exactly two statically named empty depset fields.
                budget.charge(9, 0)?;
                EmptyInstrumentedFiles::validate(occurrence).map_err(projection_error)?;
                let [first, second] = empty_instrumented_files::FIELDS;
                let fields = occurrence.fields();
                let first = self.project_value(
                    fields
                        .get(first)
                        .expect("checked InstrumentedFilesInfo fields"),
                )?;
                let second = self.project_value(
                    fields
                        .get(second)
                        .expect("checked InstrumentedFilesInfo fields"),
                )?;
                return self
                    .project_simple(|| EmptyInstrumentedFiles::from_fields([first, second]));
            }
            if occurrence.identity().is_builtin("OutputGroupInfo") {
                return Err(budget
                    .refuse(ExecutionLimit::UnsupportedOperation {
                        operation: "provider materialization",
                        value_type: occurrence.value_type().as_str(),
                    })
                    .into());
            }
            let fields = occurrence.fields();
            let rows = fields.iter_hashed().map(|(name, value)| {
                let name_copy =
                    slug_loading_v2::provider::copy_budgeted_compact_str(budget, name.key())?;
                Ok((
                    Hashed::new_unchecked(name.hash(), name_copy),
                    self.project_value(value)?,
                ))
            });
            let fields = SmallMap::try_from_unique_hashed_fallible_rows(
                fields.len(),
                rows,
                |work, bytes| Ok::<_, starlark::Error>(budget.charge(work, bytes)?),
            )
            .map_err(|error| projection_map_error(budget, error))?;
            let identity = self.project_provider_identity(occurrence.identity())?;
            return self.project_simple(|| BuiltinProviderView { identity, fields });
        }
        if occurrence.identity().is_builtin("InstrumentedFilesInfo") {
            EmptyInstrumentedFiles::validate(occurrence).map_err(projection_error)?;
        }
        if occurrence.identity().is_builtin("OutputGroupInfo") {
            let info = OutputGroupInfo::from_occurrence(occurrence)
                .map_err(|error| projection_error(error.to_string()))?;
            return self.project_output_group_info(&info);
        }
        let fields = occurrence
            .fields()
            .iter()
            .map(|(name, value)| Ok((name.clone(), self.project_value(value)?)))
            .collect::<starlark::Result<Vec<_>>>()?;
        Ok(
            if occurrence.identity().is_builtin("InstrumentedFilesInfo") {
                EmptyInstrumentedFiles::alloc(self.heap, fields)
            } else if occurrence.identity().is_builtin("ToolchainInfo") {
                StarlarkToolchainInfo::alloc(self.heap, fields)
            } else if let Some(id) = occurrence.identity().user_id() {
                alloc_starlark_user_provider(self.heap, id.dupe(), occurrence.has_schema(), fields)
            } else {
                self.heap.alloc(BuiltinProviderView {
                    identity: occurrence.identity().clone(),
                    fields: fields.into_iter().collect(),
                })
            },
        )
    }

    fn artifact_option(&self, value: Option<&AnalysisArtifact>) -> starlark::Result<FrozenValue> {
        value.map_or_else(
            || Ok(FrozenValue::new_none()),
            |value| self.project_artifact(value),
        )
    }

    fn string_map(
        &self,
        values: &std::collections::BTreeMap<String, String>,
    ) -> starlark::Result<FrozenValue> {
        if let Some(budget) = self.budget {
            let rows = values.iter().map(|(key, value)| {
                Ok((
                    self.project_string(key)?.to_frozen_value(),
                    self.project_string(value)?.to_frozen_value(),
                ))
            });
            return slug_loading_v2::provider::alloc_budgeted_analysis_dict(
                self.heap,
                budget,
                values.len(),
                rows,
            );
        }
        Ok(self.heap.alloc(AllocDict(values.iter().map(|(key, value)| {
            (
                self.heap.alloc_str(key).to_frozen_value(),
                self.heap.alloc_str(value).to_frozen_value(),
            )
        }))))
    }

    fn file_depset(&mut self, value: &Depset<String>) -> FrozenValue {
        let heap = self.heap;
        native_depset::project(
            heap,
            value,
            &mut self.file_depsets,
            "string",
            true,
            |leaf| heap.alloc_str(leaf).to_frozen_value(),
        )
    }

    fn symlink_depset(&mut self, value: &RunfilesSymlinkDepset) -> FrozenValue {
        let heap = self.heap;
        native_depset::project(
            heap,
            value,
            &mut self.symlink_depsets,
            "SymlinkEntry",
            false,
            |leaf| {
                heap.alloc(StarlarkRunfilesSymlink {
                    retained: leaf.clone(),
                    path: heap.alloc_str(&leaf.path).to_frozen_value(),
                    target_file: heap.alloc(AnalysisArtifactValue::new(leaf.artifact.clone())),
                })
            },
        )
    }

    fn runfiles(&mut self, value: &RetainedRunfiles) -> Result<FrozenValue, String> {
        self.project_runfiles(value, None)
            .map_err(|error| error.without_diagnostic().to_string())
    }

    fn project_runfiles(
        &mut self,
        value: &RetainedRunfiles,
        occurrence: Option<&slug_build_api_v2::RunfilesOccurrence>,
    ) -> starlark::Result<FrozenValue> {
        let files = self.project_depset(&value.files)?;
        let heap = self.heap;
        let (retained, symlinks, root_symlinks, empty_filenames) = if let Some(budget) = self.budget
        {
            let retained = RetainedRunfiles {
                files: value.files.dupe(),
                symlinks: value.symlinks.dupe(),
                root_symlinks: value.root_symlinks.dupe(),
                empty_filenames: value.empty_filenames.dupe(),
                conflict_policy: value.conflict_policy,
                repository_prefix: slug_loading_v2::provider::copy_budgeted_compact_str(
                    budget,
                    &value.repository_prefix,
                )?,
            };
            let symlink = |leaf: &RunfilesSymlink| -> Result<FrozenValue, ExecutionLimit> {
                let path = heap
                    .alloc_budgeted_str(budget, &leaf.path)?
                    .to_frozen_value();
                let target_file =
                    AnalysisArtifactValue::alloc_frozen_budgeted(heap, budget, &leaf.artifact)?;
                if let AnalysisArtifact::Source(label) = &leaf.artifact {
                    for bytes in label.cloned_text_lengths() {
                        budget.charge(1, bytes)?;
                        budget.charge_text_work(bytes)?;
                    }
                }
                let retained = leaf.try_clone_with_path(|text| {
                    slug_loading_v2::provider::copy_budgeted_compact_str(budget, text)
                })?;
                heap.alloc_budgeted_simple(budget, || StarlarkRunfilesSymlink {
                    retained,
                    path,
                    target_file,
                })
            };
            let symlink_node = |node: &RunfilesSymlinkDepset, row| {
                native_depset::alloc_budgeted_native_node(
                    heap,
                    budget,
                    node,
                    "SymlinkEntry",
                    false,
                    row,
                )
            };
            let symlinks = native_depset::try_project(
                &value.symlinks,
                &mut self.symlink_depsets,
                Some(budget),
                symlink,
                symlink_node,
            )?;
            let root_symlinks = native_depset::try_project(
                &value.root_symlinks,
                &mut self.symlink_depsets,
                Some(budget),
                symlink,
                symlink_node,
            )?;
            let empty_filenames = native_depset::try_project(
                &value.empty_filenames,
                &mut self.file_depsets,
                Some(budget),
                |text| Ok(heap.alloc_budgeted_str(budget, text)?.to_frozen_value()),
                |node, row| {
                    native_depset::alloc_budgeted_native_node(
                        heap, budget, node, "string", true, row,
                    )
                },
            )?;
            (retained, symlinks, root_symlinks, empty_filenames)
        } else {
            (
                value.clone(),
                self.symlink_depset(&value.symlinks),
                self.symlink_depset(&value.root_symlinks),
                self.file_depset(&value.empty_filenames),
            )
        };
        let occurrence = if let Some(occurrence) = occurrence {
            occurrence.clone()
        } else {
            if let Some(budget) = self.budget {
                budget.charge(1, size_of::<[std::sync::atomic::AtomicUsize; 2]>())?;
            }
            slug_build_api_v2::RunfilesOccurrence::new()
        };
        self.project_simple(|| StarlarkRunfiles {
            occurrence,
            retained,
            files,
            symlinks,
            root_symlinks,
            empty_filenames,
        })
    }

    fn files_to_run(&self, value: &FilesToRunProvider) -> starlark::Result<FrozenValue> {
        let executable = self.artifact_option(value.executable.as_ref())?;
        let runfiles_manifest = self.artifact_option(value.runfiles_manifest())?;
        let repo_mapping_manifest = self.artifact_option(value.repo_mapping_manifest())?;
        if let (Some(budget), Some(AnalysisArtifact::Source(label))) =
            (self.budget, value.executable.as_ref())
        {
            for bytes in label.cloned_text_lengths() {
                budget.charge(1, bytes)?;
                budget.charge_text_work(bytes)?;
            }
        }
        self.project_simple(|| StarlarkFilesToRunProvider {
            retained: value.clone(),
            executable,
            runfiles_manifest,
            repo_mapping_manifest,
        })
    }

    fn project_builtin(
        &mut self,
        identity: &ProviderIdentity,
        value: &ProviderValue,
    ) -> starlark::Result<FrozenValue> {
        let fields = match value {
            ProviderValue::FeatureFlagInfo(info) => {
                return Ok(match self.budget {
                    Some(budget) => {
                        slug_loading_v2::provider::alloc_budgeted_frozen_feature_flag_info(
                            self.heap, budget, info,
                        )?
                    }
                    None => slug_loading_v2::provider::alloc_frozen_feature_flag_info(
                        self.heap,
                        info.dupe(),
                    ),
                });
            }
            ProviderValue::DefaultInfo(info) => self.project_fixed_fields(
                ["files", "default_runfiles", "data_runfiles", "files_to_run"],
                |state, name| match name {
                    "files" => state.project_depset(info.files()),
                    "default_runfiles" => state.project_runfiles(&info.default_runfiles, None),
                    "data_runfiles" => state.project_runfiles(&info.data_runfiles, None),
                    "files_to_run" => state.files_to_run(&info.files_to_run),
                    _ => unreachable!("fixed DefaultInfo field table"),
                },
            )?,
            ProviderValue::OutputGroupInfo(info) => return self.project_output_group_info(info),
            ProviderValue::PackageSpecificationInfo(info) => {
                return Ok(crate::package_specification::alloc_with_budget(
                    self.heap,
                    self.budget,
                    info,
                )?);
            }
            ProviderValue::TemplateVariableInfo(info) => {
                return Ok(match self.budget {
                    Some(budget) => {
                        slug_loading_v2::provider::alloc_budgeted_frozen_template_variable_info(
                            self.heap, budget, info,
                        )?
                    }
                    None => {
                        slug_loading_v2::alloc_frozen_template_variable_info(self.heap, info.dupe())
                    }
                });
            }
            ProviderValue::RunEnvironmentInfo(info) => {
                return Ok(match self.budget {
                    Some(budget) => {
                        slug_loading_v2::provider::alloc_budgeted_frozen_run_environment_info(
                            self.heap, budget, info,
                        )?
                    }
                    None => slug_loading_v2::provider::alloc_frozen_run_environment_info(
                        self.heap,
                        info.dupe(),
                    ),
                });
            }
            ProviderValue::FilesToRunProvider(info) => return self.files_to_run(info),
            ProviderValue::PlatformInfo(info) => self.project_fixed_fields(
                ["label", "constraints", "exec_properties"],
                |state, name| match name {
                    "label" => Ok(state.project_string(&info.label)?.to_frozen_value()),
                    "constraints" => state.string_map(&info.constraints),
                    "exec_properties" => state.string_map(&info.exec_properties),
                    _ => unreachable!("fixed PlatformInfo field table"),
                },
            )?,
            ProviderValue::Occurrence(_) => unreachable!("occurrences use the shared classes"),
        };
        let identity = self.project_provider_identity(identity)?;
        self.project_simple(|| BuiltinProviderView { identity, fields })
    }

    fn project_fixed_fields<const N: usize>(
        &mut self,
        names: [&'static str; N],
        mut project: impl FnMut(&mut Self, &str) -> starlark::Result<FrozenValue>,
    ) -> starlark::Result<SmallMap<CompactString, FrozenValue>> {
        let budget = self.budget;
        let rows = names.into_iter().map(|name| {
            let value = project(self, name)?;
            Ok((Hashed::new(CompactString::const_new(name)), value))
        });
        if let Some(budget) = budget {
            SmallMap::try_from_unique_hashed_fallible_rows(N, rows, |work, bytes| {
                Ok::<_, starlark::Error>(budget.charge(work, bytes)?)
            })
            .map_err(|error| projection_map_error(budget, error))
        } else {
            rows.collect()
        }
    }

    fn target(&mut self, target: &ConfiguredTargetValue) -> Result<FrozenValue, String> {
        self.project_target(target)
            .map_err(|error| error.without_diagnostic().to_string())
    }

    fn project_target(&mut self, target: &ConfiguredTargetValue) -> starlark::Result<FrozenValue> {
        if let Some(budget) = self.budget {
            let providers = target.providers();
            let rows = providers.iter_hashed().map(|(key, value)| {
                let identity = self.project_provider_identity(key.key())?;
                let value = self.project_provider_row(key.key(), value)?;
                Ok((Hashed::new_unchecked(key.hash(), identity), value))
            });
            let providers = SmallMap::try_from_unique_hashed_fallible_rows(
                providers.len(),
                rows,
                |work, bytes| Ok::<_, starlark::Error>(budget.charge(work, bytes)?),
            )
            .map_err(|error| projection_map_error(budget, error))?;
            return self.project_simple(|| AnalysisConfiguredTargetValue {
                retained: target.clone(),
                providers,
            });
        }
        let mut providers = SmallMap::new();
        for (identity, provider) in target.providers().iter() {
            let value = self.project_provider_row(identity, provider)?;
            providers.insert(identity.clone(), value);
        }
        Ok(self.heap.alloc(AnalysisConfiguredTargetValue {
            retained: target.clone(),
            providers,
        }))
    }

    fn project_provider_identity(
        &self,
        identity: &ProviderIdentity,
    ) -> starlark::Result<ProviderIdentity> {
        Ok(match (self.budget, identity) {
            (Some(budget), ProviderIdentity::Builtin(name)) => ProviderIdentity::Builtin(
                slug_loading_v2::provider::copy_budgeted_compact_str(budget, name)?,
            ),
            _ => identity.clone(),
        })
    }

    fn project_provider_row(
        &mut self,
        identity: &ProviderIdentity,
        value: &ProviderValue,
    ) -> starlark::Result<FrozenValue> {
        match value {
            ProviderValue::Occurrence(value) => self.project_provider(value),
            value => self.project_builtin(identity, value),
        }
    }

    fn project_depset(&mut self, value: &AnalysisDepset) -> starlark::Result<FrozenValue> {
        let budget = self.budget;
        native_depset::try_project_retained(
            self,
            value,
            budget,
            |state| &mut state.depsets,
            Self::project_value,
            |state, node, successors| {
                Ok(match budget {
                    Some(budget) => slug_loading_v2::provider::alloc_budgeted_starlark_depset(
                        state.heap,
                        budget,
                        node.dupe(),
                        successors,
                    )?,
                    None => alloc_starlark_depset(state.heap, node.dupe(), successors),
                })
            },
        )
    }

    pub(crate) fn value(&mut self, value: &AnalysisValue) -> Result<FrozenValue, String> {
        self.project_value(value)
            .map_err(|error| error.without_diagnostic().to_string())
    }

    fn project_value(&mut self, value: &AnalysisValue) -> starlark::Result<FrozenValue> {
        if let Some(budget) = self.budget {
            budget.charge(1, 0)?;
            if self.depth >= budget.text_depth() {
                return Err(budget.refuse(ExecutionLimit::StructuralDepth).into());
            }
        }
        if let Some(original) = value.original_origin() {
            return self.project_original(original);
        }
        let key = value.container_identity().map(Hashed::new);
        if let Some(key) = &key {
            let cached = if let Some(budget) = self.budget {
                self.containers
                    .get_hashed_bounded(
                        key.hash(),
                        |existing| Ok(key.key() == existing),
                        |work, bytes| budget.charge(work, bytes),
                    )
                    .map_err(|error| projection_map_error(budget, error))?
            } else {
                self.containers.get_hashed(key.as_ref())
            };
            if let Some(frozen) = cached {
                return Ok(*frozen);
            }
        }
        self.depth += 1;
        let result = self.value_unmemoized(value);
        self.depth -= 1;
        let frozen = result?;
        if let Some(key) = key {
            if let Some(budget) = self.budget {
                self.containers
                    .try_insert_unique_hashed_bounded(key, frozen, |work, bytes| {
                        budget.charge(work, bytes)
                    })
                    .map_err(|error| projection_map_error(budget, error))?;
            } else {
                self.containers.insert_hashed_unique_unchecked(key, frozen);
            }
        }
        Ok(frozen)
    }

    fn value_unmemoized(&mut self, value: &AnalysisValue) -> starlark::Result<FrozenValue> {
        Ok(match value.kind() {
            AnalysisValueKind::CppFragment(_)
            | AnalysisValueKind::DottedVersion(_)
            | AnalysisValueKind::CcToolchainVariables(_)
            | AnalysisValueKind::CcHeaderInfo(_)
            | AnalysisValueKind::CcToolchainFeatures(_)
            | AnalysisValueKind::CcFeatureConfiguration(_)
                if self.budget.is_some() =>
            {
                let budget = self.budget.expect("guarded request budget");
                return Err(budget
                    .refuse(ExecutionLimit::UnsupportedOperation {
                        operation: "value materialization",
                        value_type: value.value_type().as_str(),
                    })
                    .into());
            }
            AnalysisValueKind::Function(_)
            | AnalysisValueKind::ProviderConstructor(_)
            | AnalysisValueKind::RawProviderConstructor(_) => {
                return Err(projection_error(
                    "callable projection has no original Starlark binding",
                ));
            }
            AnalysisValueKind::Runfiles(runfiles) => {
                self.project_runfiles(runfiles.retained(), Some(runfiles.occurrence()))?
            }
            AnalysisValueKind::CppFragment(fragment) => {
                slug_loading_v2::analysis_fragments::alloc_frozen_cpp_fragment(
                    self.heap,
                    fragment.clone(),
                )
            }
            AnalysisValueKind::DottedVersion(version) => {
                slug_loading_v2::alloc_frozen_dotted_version(self.heap, version.clone())
            }
            AnalysisValueKind::CcToolchainVariables(value) => {
                self.cc_variables(value).map_err(projection_error)?
            }
            AnalysisValueKind::CcHeaderInfo(info) => {
                slug_loading_v2::alloc_frozen_cc_header_info(self.heap, info.clone())
            }
            AnalysisValueKind::PackageSpecificationInfo(info) => {
                crate::package_specification::alloc_with_budget(self.heap, self.budget, info)?
            }
            AnalysisValueKind::TemplateVariableInfo(info) => match self.budget {
                Some(budget) => {
                    slug_loading_v2::provider::alloc_budgeted_frozen_template_variable_info(
                        self.heap, budget, info,
                    )?
                }
                None => {
                    slug_loading_v2::alloc_frozen_template_variable_info(self.heap, info.dupe())
                }
            },
            AnalysisValueKind::RunEnvironmentInfo(info) => match self.budget {
                Some(budget) => {
                    slug_loading_v2::provider::alloc_budgeted_frozen_run_environment_info(
                        self.heap, budget, info,
                    )?
                }
                None => slug_loading_v2::provider::alloc_frozen_run_environment_info(
                    self.heap,
                    info.dupe(),
                ),
            },
            AnalysisValueKind::CcToolchainFeatures(value) => {
                slug_loading_v2::alloc_frozen_cc_toolchain_features(self.heap, value.clone())
            }
            AnalysisValueKind::CcFeatureConfiguration(value) => {
                slug_loading_v2::alloc_frozen_cc_feature_configuration(self.heap, value.clone())
            }
            AnalysisValueKind::None => FrozenValue::new_none(),
            AnalysisValueKind::Boolean(value) => FrozenValue::new_bool(value),
            AnalysisValueKind::Number(AnalysisNumber::Integer(value)) => {
                if let Some(budget) = self.budget {
                    return Ok(self.heap.alloc_budgeted_integer_be(
                        budget,
                        value.is_negative(),
                        value.magnitude(),
                    )?);
                }
                self.heap.alloc(BigInt::from_bytes_be(
                    if value.is_negative() {
                        Sign::Minus
                    } else {
                        Sign::Plus
                    },
                    value.magnitude(),
                ))
            }
            AnalysisValueKind::Number(AnalysisNumber::Float(bits)) => {
                self.project_simple(|| StarlarkFloat(f64::from_bits(*bits)))?
            }
            AnalysisValueKind::String(value) => self.project_string(value)?.to_frozen_value(),
            AnalysisValueKind::Label(value) => self.project_label(value)?,
            AnalysisValueKind::ConfiguredTarget(value) => self.project_target(value)?,
            AnalysisValueKind::Artifact(value) => self.project_artifact(value)?,
            AnalysisValueKind::ArtifactRoot(value) => match self.budget {
                Some(budget) => {
                    slug_loading_v2::alloc_budgeted_frozen_artifact_root(self.heap, budget, value)?
                }
                None => slug_loading_v2::alloc_frozen_artifact_root(self.heap, value.clone()),
            },
            AnalysisValueKind::List(values) => self.project_sequence(values, false)?,
            AnalysisValueKind::Tuple(values) => self.project_sequence(values, true)?,
            AnalysisValueKind::Dictionary(values) => {
                let heap = self.heap;
                let budget = self.budget;
                let rows = values
                    .iter()
                    .map(|(key, value)| Ok((self.project_value(key)?, self.project_value(value)?)));
                if let Some(budget) = budget {
                    slug_loading_v2::provider::alloc_budgeted_analysis_dict(
                        heap,
                        budget,
                        values.len(),
                        rows,
                    )?
                } else {
                    heap.alloc(AllocDict(rows.collect::<starlark::Result<Vec<_>>>()?))
                }
            }
            AnalysisValueKind::Struct(fields) => {
                let heap = self.heap;
                if let Some(budget) = self.budget {
                    let rows = fields.iter().map(|(name, value)| {
                        Ok((self.project_string(name)?, self.project_value(value)?))
                    });
                    heap.alloc_budgeted_struct_from_rows(budget, fields.len(), rows)?
                } else {
                    let rows = fields
                        .iter()
                        .map(|(name, value)| Ok((name.to_string(), self.project_value(value)?)));
                    heap.alloc(AllocStruct(rows.collect::<starlark::Result<Vec<_>>>()?))
                }
            }
            AnalysisValueKind::Provider(value) => self.project_provider(value)?,
            AnalysisValueKind::Depset(value) => self.project_depset(value)?,
        })
    }

    fn project_sequence(
        &mut self,
        values: &[AnalysisValue],
        tuple: bool,
    ) -> starlark::Result<FrozenValue> {
        let mut materialized = if let Some(budget) = self.budget {
            let mut values_out = Vec::new();
            budget.reserve_vec(&mut values_out, values.len())?;
            values_out
        } else {
            Vec::with_capacity(values.len())
        };
        for value in values {
            materialized.push(self.project_value(value)?);
        }
        Ok(match (self.budget, tuple) {
            (Some(budget), true) => self.heap.alloc_budgeted_tuple(budget, &materialized)?,
            (Some(budget), false) => self
                .heap
                .alloc_budgeted_immutable_list(budget, &materialized)?,
            (None, true) => self.heap.alloc(AllocTuple(materialized)),
            (None, false) => self.heap.alloc(AllocImmutableList(materialized)),
        })
    }

    pub(crate) fn configured_dependency_target(
        &mut self,
        key: &ConfiguredNodeKey,
        providers: ProviderCollection,
    ) -> Result<FrozenValue, String> {
        let identity = key.configured_target().map_or_else(
            || AnalysisTargetIdentity::null(key.label().clone()),
            |configured| analysis_configured_key(configured).into(),
        );
        self.target(&ConfiguredTargetValue::new(identity, providers))
    }

    pub(crate) fn budgeted_cquery_target(
        &mut self,
        key: &ConfiguredNodeKey,
        providers: &ProviderCollection,
    ) -> starlark::Result<FrozenValue> {
        let budget = self
            .budget
            .expect("cquery target projection has request credits");
        let admit = |owner_bytes, label_bytes| {
            budget.charge(1, owner_bytes)?;
            for bytes in label_bytes {
                budget.charge(1, bytes)?;
                budget.charge_text_work(bytes)?;
            }
            Ok::<_, ExecutionLimit>(())
        };
        let identity = if let Some(key) = key.configured_target() {
            // Public configured nodes already retain the complete canonical bytes.
            let configuration = key.configuration().slug_configuration().ok_or_else(|| {
                budget.refuse(ExecutionLimit::UnsupportedOperation {
                    operation: "configured target materialization",
                    value_type: "legacy configuration",
                })
            })?;
            AnalysisConfiguredTargetKey::try_from_borrowed(
                key.label(),
                configuration.shared_canonical_bytes(),
                key.toolchain_execution_platform(),
                admit,
            )?
            .into()
        } else {
            AnalysisTargetIdentity::try_null_from_borrowed(key.label(), admit)?
        };
        self.project_target(&ConfiguredTargetValue::new(identity, providers.clone()))
    }
}

fn projection_error(message: impl Into<String>) -> starlark::Error {
    starlark::Error::new_other(anyhow::Error::msg(message.into()))
}

fn projection_map_error(
    budget: &ExecutionBudget,
    error: BoundedMapError<impl Into<starlark::Error>>,
) -> starlark::Error {
    match error {
        BoundedMapError::Operation(limit) => limit.into(),
        BoundedMapError::SizeOverflow => budget.refuse(ExecutionLimit::AllocationBytes).into(),
        BoundedMapError::AllocationFailed => budget.refuse(ExecutionLimit::AllocationFailed).into(),
        BoundedMapError::RowCountMismatch => projection_error("materialization row count changed"),
    }
}

#[derive(Default)]
pub(crate) struct AnalysisValueLowerer<'v> {
    visiting: HashSet<ValueIdentity<'v>>,
    memo: HashMap<ValueIdentity<'v>, AnalysisValue>,
    lower_depth: usize,
}

impl<'v> AnalysisValueLowerer<'v> {
    pub(crate) fn from_module(_module: &'v starlark::environment::Module) -> Self {
        Self::default()
    }
    pub(crate) fn lower(&mut self, value: Value<'v>, path: &str) -> Result<AnalysisValue, String> {
        if self.lower_depth == 0 {
            self.lower_depth = 1;
            let prepared = self.prepare_reachable_depsets(value, path);
            self.lower_depth = 0;
            prepared?;
        }
        self.lower_depth += 1;
        let result = self.lower_one(value, path);
        self.lower_depth -= 1;
        result
    }

    fn lower_one(&mut self, value: Value<'v>, path: &str) -> Result<AnalysisValue, String> {
        if value.is_none() {
            return Ok(AnalysisValue::none());
        }
        if let Some(value) = value.unpack_bool() {
            return Ok(AnalysisValue::boolean(value));
        }
        if let Some(value) = value.unpack_str() {
            return Ok(AnalysisValue::string(value));
        }
        if let Some(value) = value.downcast_ref::<StarlarkFloat>() {
            return Ok(AnalysisValue::float(value.0));
        }
        if let Some(value) = BigInt::unpack_value(value).map_err(|error| error.to_string())? {
            let (sign, magnitude) = value.to_bytes_be();
            return Ok(AnalysisValue::integer_from_magnitude(
                sign == Sign::Minus,
                magnitude,
            ));
        }
        if let Some(value) = starlark_label(value) {
            return Ok(AnalysisValue::label(value));
        }
        if let Some(value) = AnalysisArtifactValue::from_starlark(value) {
            return Ok(AnalysisValue::artifact(value.artifact().clone()));
        }
        if let Some(root) = slug_loading_v2::artifact_root_value(value) {
            return Ok(AnalysisValue::artifact_root(root));
        }
        if let Some(value) = StarlarkRunfiles::from_value(value) {
            return Ok(AnalysisValue::runfiles(
                value.retained.clone(),
                value.occurrence.clone(),
            ));
        }
        if let Some(value) = StarlarkFilesToRunProvider::from_value(value) {
            return Ok(AnalysisValue::provider(value.retained.to_occurrence()));
        }
        if let Some(value) = AnalysisConfiguredTargetValue::from_value(value) {
            return Ok(AnalysisValue::configured_target(value.retained.clone()));
        }
        if let Some((_, _, _, Some(retained), _, _)) = StarlarkDepset::parts_from_value(value) {
            return Ok(AnalysisValue::depset(retained));
        }

        let identity = value.identity();
        if let Some(value) = self.memo.get(&identity) {
            return Ok(value.dupe());
        }
        if !self.visiting.insert(identity) {
            return Err(format!("{path}: cyclic analysis value"));
        }
        let lowered = self.lower_recursive(value, path);
        self.visiting.remove(&identity);
        if let Ok(value) = &lowered {
            self.memo.insert(identity, value.dupe());
        }
        lowered
    }

    fn prepare_reachable_depsets(&mut self, root: Value<'v>, path: &str) -> Result<(), String> {
        let mut roots = Vec::new();
        let mut seen = HashSet::new();
        let mut stack = vec![root];
        while let Some(value) = stack.pop() {
            if !seen.insert(value.identity()) {
                continue;
            }
            if let Some(variables) = slug_loading_v2::cc_variables_value(value) {
                stack.extend(variables.parent);
                stack.extend(variables.locals.into_iter().map(|(_, value)| value));
                for (_, entry) in variables.cache {
                    stack.extend(entry.values().iter().copied());
                }
                continue;
            }
            if let Some((_, _, _, retained, _, _)) = StarlarkDepset::parts_from_value(value) {
                if retained.is_none() {
                    roots.push(value);
                }
                continue;
            }
            let provider_fields =
                if let Some(fields) = StarlarkOutputGroupInfo::fields_from_value(value) {
                    Some(fields)
                } else if let Some(fields) = StarlarkToolchainInfo::fields_from_value(value) {
                    Some(fields)
                } else if let Some(fields) = EmptyInstrumentedFiles::fields_from_value(value) {
                    Some(fields)
                } else if let Some((_, fields)) = BuiltinProviderView::fields_from_value(value) {
                    Some(fields)
                } else {
                    starlark_user_provider_fields(value).map(|(_, fields)| fields)
                };
            if let Some(fields) = provider_fields {
                stack.extend(fields.into_iter().map(|(_, value)| value));
            } else if let Some(values) = ListRef::from_value(value) {
                stack.extend(values.iter());
            } else if let Some(values) = TupleRef::from_value(value) {
                stack.extend(values.iter());
            } else if let Some(values) = DictRef::from_value(value) {
                for (key, value) in values.iter() {
                    stack.push(key);
                    stack.push(value);
                }
            } else if let Some(fields) = StructRef::from_value(value) {
                stack.extend(fields.iter().map(|(_, value)| value));
            }
        }
        if roots.is_empty() {
            return Ok(());
        }
        self.prepare_depsets(roots, path)
    }

    fn prepare_depsets(&mut self, roots: Vec<Value<'v>>, path: &str) -> Result<(), String> {
        let mut local_ids = HashMap::new();
        let mut local_identities = Vec::new();
        let mut local_orders = Vec::new();
        let mut graph = Vec::new();
        let mut stack = roots
            .into_iter()
            .rev()
            .map(|root| (root, path.to_owned(), false))
            .collect::<Vec<_>>();
        while let Some((value, path, exiting)) = stack.pop() {
            let identity = value.identity();
            if !exiting {
                // Completed nodes may already be retained by an earlier provider.
                // Rebuilding them would split aliases across publication roots.
                if local_ids.contains_key(&identity) || self.memo.contains_key(&identity) {
                    continue;
                }
                let (_, _, _, retained, _, successors) = StarlarkDepset::parts_from_value(value)
                    .expect("depset lowering starts from a depset");
                if let Some(retained) = retained {
                    self.memo.insert(identity, AnalysisValue::depset(retained));
                    continue;
                }
                if !self.visiting.insert(identity) {
                    return Err(format!("{path}: cyclic analysis value"));
                }
                stack.push((value, path.clone(), true));
                for (index, successor) in successors.into_iter().enumerate().rev() {
                    if let StarlarkDepsetSuccessorGen::Transitive(child) = successor {
                        stack.push((child, format!("{path}.successor[{index}]"), false));
                    }
                }
                continue;
            }
            let (order, _, occurrence, _, depth, successors) =
                StarlarkDepset::parts_from_value(value).expect("depset frame remains a depset");
            let successors = successors
                .into_iter()
                .enumerate()
                .map(|(index, successor)| match successor {
                    StarlarkDepsetSuccessorGen::Direct(value) => self
                        .lower(value, &format!("{path}.successor[{index}]"))
                        .map(AnalysisDepsetGraphInput::Direct),
                    StarlarkDepsetSuccessorGen::Transitive(value) => {
                        let identity = value.identity();
                        if let Some(node) = local_ids.get(&identity) {
                            Ok(AnalysisDepsetGraphInput::Local(*node))
                        } else {
                            self.memo
                                .get(&identity)
                                .and_then(|value| match value.kind() {
                                    AnalysisValueKind::Depset(value) => Some(value.clone()),
                                    _ => None,
                                })
                                .map(AnalysisDepsetGraphInput::External)
                                .ok_or_else(|| format!("{path}: transitive item is not a depset"))
                        }
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            let row = match successors.as_slice() {
                [AnalysisDepsetGraphInput::Local(child)] if order != local_orders[*child] => {
                    AnalysisDepsetGraphRow::Local(*child)
                }
                [AnalysisDepsetGraphInput::External(child)] if order != child.order() => {
                    AnalysisDepsetGraphRow::External(child.dupe())
                }
                _ => AnalysisDepsetGraphRow::Successors(successors),
            };
            self.visiting.remove(&identity);
            let node = graph.len();
            local_ids.insert(identity, node);
            local_identities.push(identity);
            local_orders.push(order);
            graph.push(AnalysisDepsetGraphNode::new(occurrence, order, depth, row));
        }
        let depsets =
            AnalysisDepset::from_local_graph(graph).map_err(|error| format!("{path}: {error}"))?;
        for (identity, depset) in local_identities.into_iter().zip(depsets) {
            self.memo.insert(identity, AnalysisValue::depset(depset));
        }
        Ok(())
    }

    fn lower_depset(&mut self, root: Value<'v>, path: &str) -> Result<AnalysisValue, String> {
        let identity = root.identity();
        self.prepare_depsets(vec![root], path)?;
        self.memo
            .get(&identity)
            .map(Dupe::dupe)
            .ok_or_else(|| format!("{path}: depset lowering did not retain its root"))
    }

    fn lower_recursive(&mut self, value: Value<'v>, path: &str) -> Result<AnalysisValue, String> {
        if let Some(info) = slug_loading_v2::provider::run_environment_info(value) {
            return Ok(AnalysisValue::run_environment_info(info));
        }
        if let Some(info) = slug_loading_v2::template_variable_info(value) {
            return Ok(AnalysisValue::template_variable_info(info));
        }
        if let Some(info) = crate::package_specification::from_value(value) {
            return Ok(AnalysisValue::package_specification_info(info));
        }
        if let Some(version) = slug_loading_v2::dotted_version_ref(value) {
            return Ok(AnalysisValue::dotted_version(version.clone()));
        }
        if let Some(fragment) = slug_loading_v2::analysis_fragments::cpp_fragment_value(value) {
            return Ok(AnalysisValue::cpp_fragment(fragment));
        }
        if let Some(variables) = slug_loading_v2::cc_variables_value(value) {
            return self.lower_cc_variables(variables, path);
        }
        if let Some(info) = slug_loading_v2::cc_header_info(value) {
            return Ok(AnalysisValue::cc_header_info(info));
        }
        if let Some(value) = slug_loading_v2::cc_toolchain_features_value(value) {
            return Ok(AnalysisValue::cc_toolchain_features(value));
        }
        if let Some(value) = slug_loading_v2::cc_feature_configuration_value(value) {
            return Ok(AnalysisValue::cc_feature_configuration(value));
        }
        if StarlarkDepset::parts_from_value(value).is_some() {
            return self.lower_depset(value, path);
        }
        let provider = if let Some(fields) = StarlarkOutputGroupInfo::fields_from_value(value) {
            Some((ProviderIdentity::builtin("OutputGroupInfo"), fields))
        } else if let Some(fields) = StarlarkToolchainInfo::fields_from_value(value) {
            Some((ProviderIdentity::builtin("ToolchainInfo"), fields))
        } else if let Some(fields) = EmptyInstrumentedFiles::fields_from_value(value) {
            Some((ProviderIdentity::builtin("InstrumentedFilesInfo"), fields))
        } else if let Some((identity, fields)) = BuiltinProviderView::fields_from_value(value) {
            Some((identity, fields))
        } else {
            starlark_user_provider_fields(value)
                .map(|(id, fields)| (ProviderIdentity::user(id), fields))
        };
        if let Some((identity, fields)) = provider {
            let fields = fields
                .into_iter()
                .map(|(name, value)| {
                    Ok((name.clone(), self.lower(value, &format!("{path}.{name}"))?))
                })
                .collect::<Result<Vec<_>, String>>()?;
            return Ok(AnalysisValue::provider(
                ProviderOccurrence::new(identity, fields).with_schema_classification(
                    slug_loading_v2::provider::starlark_user_provider_has_schema(value)
                        .unwrap_or(false),
                ),
            ));
        }
        if let Some(values) = ListRef::from_value(value) {
            return values
                .iter()
                .enumerate()
                .map(|(index, value)| self.lower(value, &format!("{path}[{index}]")))
                .collect::<Result<Vec<_>, _>>()
                .map(AnalysisValue::list);
        }
        if let Some(values) = TupleRef::from_value(value) {
            return values
                .iter()
                .enumerate()
                .map(|(index, value)| self.lower(value, &format!("{path}[{index}]")))
                .collect::<Result<Vec<_>, _>>()
                .map(AnalysisValue::tuple);
        }
        if let Some(values) = DictRef::from_value(value) {
            let entries = values
                .iter()
                .enumerate()
                .map(|(index, (key, value))| {
                    Ok((
                        self.lower(key, &format!("{path}.key[{index}]"))?,
                        self.lower(value, &format!("{path}.value[{index}]"))?,
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?;
            return AnalysisValue::dictionary(entries).map_err(|error| format!("{path}: {error}"));
        }
        if let Some(fields) = StructRef::from_value(value) {
            let fields = fields
                .iter()
                .map(|(name, value)| {
                    Ok((
                        name.as_str().to_owned(),
                        self.lower(value, &format!("{path}.{name}"))?,
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?;
            return Ok(AnalysisValue::strukt(fields));
        }
        Err(format!(
            "{path}: unsupported analysis value of type `{}`",
            value.get_type()
        ))
    }
}

#[cfg(test)]
#[path = "analysis_value/cc_features_tests.rs"]
mod cc_features_tests;
#[cfg(test)]
mod run_environment_tests;
#[cfg(test)]
mod template_variable_tests;

#[cfg(test)]
mod workspace_status_tests;

#[cfg(test)]
#[path = "analysis_value/cpp_compile_publication_tests.rs"]
mod cpp_compile_publication_tests;

#[cfg(test)]
mod cc_header_info_tests;

#[cfg(test)]
mod provider_schema_tests;

#[cfg(test)]
mod depset_truth_tests;

#[path = "analysis_value/cc_variables.rs"]
mod cc_variables;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use slug_build_api_v2::DefaultInfo;
    use slug_build_api_v2::DepsetOrder;
    use slug_build_api_v2::FilesToRunProvider;
    use slug_build_api_v2::OutputGroupInfo;
    use slug_build_api_v2::PlatformInfo;
    use slug_build_api_v2::ProviderId;
    use slug_build_api_v2::RunEnvironmentInfo;
    use slug_identity_v2::ApparentLabel;
    use slug_identity_v2::ApparentRepoName;
    use slug_identity_v2::CanonicalLabel;
    use slug_identity_v2::CanonicalRepoName;
    use slug_identity_v2::RepositoryMapping;
    use slug_identity_v2::RepositoryMappingId;
    use slug_loading_v2::provider::alloc_starlark_provider_callable;
    use slug_loading_v2::provider::starlark_label_ref;
    use slug_loading_v2::provider::starlark_provider_identity;
    use starlark::environment::Module;
    use starlark::eval::ExecutionLimit;
    use starlark::values::list::ListRef;

    use super::*;

    fn builtin<'a>(
        target: &'a AnalysisConfiguredTargetValue,
        name: &str,
    ) -> &'a BuiltinProviderView {
        BuiltinProviderView::from_value(
            target
                .providers
                .get(&ProviderIdentity::builtin(name))
                .unwrap()
                .to_value(),
        )
        .unwrap()
    }

    fn field<'a>(view: &'a BuiltinProviderView, name: &str) -> &'a FrozenValue {
        view.fields.get(name).unwrap()
    }

    #[test]
    fn bounded_target_files_reuses_the_existing_frozen_handle() {
        let label = CanonicalLabel::parse("@@//pkg:target").unwrap();
        let default =
            DefaultInfo::from_executable(AnalysisArtifact::Source(label.clone()), None).unwrap();
        let heap = FrozenHeap::new();
        let mut materializer = AnalysisValueMaterializer::new(&heap);
        let budget = Arc::new(ExecutionBudget::new(1000, 0, 16).with_bounded_operations());
        let module = Module::new_with_execution_budget(budget.clone());
        for providers in [
            ProviderCollection::new(vec![ProviderValue::DefaultInfo(default)]).unwrap(),
            ProviderCollection::from_values(Vec::new(), false).unwrap(),
        ] {
            let target =
                ConfiguredTargetValue::new(AnalysisTargetIdentity::null(label.clone()), providers);
            let target = materializer.target(&target).unwrap().to_value();
            let expected = AnalysisConfiguredTargetValue::from_value(target)
                .unwrap()
                .files();
            let actual = target.get_attr("files", module.heap()).unwrap();
            match expected {
                Some(handle) => assert!(actual.unwrap().ptr_eq(handle.to_value())),
                None => assert!(actual.is_none()),
            }
            assert!(target.get_attr("unknown", module.heap()).unwrap().is_none());
        }
        assert_eq!(budget.check(), Ok(()));
        assert_eq!(module.heap().allocated_summary().total_allocated_bytes(), 0);
    }

    #[test]
    fn bounded_target_label_admits_all_owned_components_and_the_aligned_object() {
        let mut mapping =
            RepositoryMapping::new(RepositoryMappingId::new("mapping".repeat(80)).unwrap());
        mapping.insert(
            ApparentRepoName::new("visible").unwrap(),
            CanonicalRepoName::new(format!("dep+{}", "r".repeat(260))).unwrap(),
        );
        let mapped = ApparentLabel::parse(&format!(
            "@visible//{}:{}",
            "package".repeat(60),
            "résumé".repeat(60)
        ))
        .unwrap()
        .resolve(&mapping);
        assert_eq!(mapped.mapping_id(), Some(mapping.id()));

        for label in [CanonicalLabel::parse("@@//:root").unwrap(), mapped] {
            let target = ConfiguredTargetValue::new(
                AnalysisTargetIdentity::null(label.clone()),
                ProviderCollection::new(vec![ProviderValue::DefaultInfo(DefaultInfo::empty())])
                    .unwrap(),
            );
            let heap = FrozenHeap::new();
            let target = AnalysisValueMaterializer::new(&heap)
                .target(&target)
                .unwrap();
            let ordinary = Module::new();
            let ordinary_label = target
                .to_value()
                .get_attr("label", ordinary.heap())
                .unwrap()
                .unwrap();
            assert_eq!(starlark_label_ref(ordinary_label), Some(&label));
            // The type profile includes both the aligned arena object and
            // uniquely owned allocations made by the ordinary label clone.
            let bytes = ordinary
                .heap()
                .allocated_summary()
                .summary()
                .get("Label")
                .unwrap()
                .1;
            for allowance in [bytes - 1, bytes] {
                let budget =
                    Arc::new(ExecutionBudget::new(1000, allowance, 16).with_bounded_operations());
                let module = Module::new_with_execution_budget(budget.clone());
                let result = target.to_value().get_attr("label", module.heap());
                if allowance == bytes {
                    let actual = result.unwrap().unwrap();
                    assert_eq!(starlark_label_ref(actual), Some(&label));
                    assert_eq!(
                        module.heap().allocated_summary().summary().get("Label"),
                        Some(&(1, bytes))
                    );
                    assert_eq!(budget.charge(0, 1), Err(ExecutionLimit::AllocationBytes));
                } else {
                    let error = result.unwrap_err();
                    assert!(matches!(error.kind(), starlark::ErrorKind::Other(cause)
                        if cause.downcast_ref::<ExecutionLimit>() == Some(&ExecutionLimit::AllocationBytes)));
                    assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));
                    assert_eq!(module.heap().allocated_summary().total_allocated_bytes(), 0);
                }
            }
        }
    }

    #[test]
    fn complete_target_view_projects_every_retained_provider_variant() {
        let user_id = ProviderId::new("//:defs.bzl", "Info").unwrap();
        let user = ProviderOccurrence::new(
            ProviderIdentity::user(user_id.dupe()),
            [("value", AnalysisValue::string("user"))],
        );
        let toolchain = ProviderOccurrence::new(
            ProviderIdentity::builtin("ToolchainInfo"),
            [("marker", AnalysisValue::string("toolchain"))],
        );
        let executable_artifact = AnalysisArtifact::Source(
            slug_identity_v2::CanonicalLabel::parse("@@//bin:tool").unwrap(),
        );
        let providers = ProviderCollection::new(vec![
            ProviderValue::DefaultInfo(
                DefaultInfo::from_executable(executable_artifact.clone(), None).unwrap(),
            ),
            ProviderValue::OutputGroupInfo(
                OutputGroupInfo::new(BTreeMap::from([(
                    "validation".to_owned(),
                    AnalysisDepset::new(
                        slug_build_api_v2::DepsetOrder::Default,
                        vec![AnalysisValue::artifact(AnalysisArtifact::Source(
                            slug_identity_v2::CanonicalLabel::parse("@@//pkg:validation.txt")
                                .unwrap(),
                        ))],
                        vec![],
                    )
                    .unwrap(),
                )]))
                .unwrap(),
            ),
            ProviderValue::RunEnvironmentInfo(RunEnvironmentInfo::new(
                [(CompactString::new("KEY"), CompactString::new("value"))]
                    .into_iter()
                    .collect(),
                std::sync::Arc::from([CompactString::new("PATH")]),
            )),
            ProviderValue::FilesToRunProvider(
                FilesToRunProvider::single_executable_without_support(executable_artifact),
            ),
            ProviderValue::PlatformInfo(PlatformInfo {
                label: "@@platforms//:host".to_owned(),
                constraints: BTreeMap::from([("cpu".to_owned(), "x86_64".to_owned())]),
                exec_properties: BTreeMap::from([("pool".to_owned(), "linux".to_owned())]),
            }),
            ProviderValue::Occurrence(user),
            ProviderValue::Occurrence(toolchain),
        ])
        .unwrap();
        let retained = ConfiguredTargetValue::new(
            AnalysisConfiguredTargetKey::new(
                slug_identity_v2::CanonicalLabel::parse("@@//:target").unwrap(),
                [1, 2, 3],
            ),
            providers,
        );
        let heap = FrozenHeap::new();
        let value = AnalysisValueMaterializer::new(&heap)
            .target(&retained)
            .unwrap();
        let target = AnalysisConfiguredTargetValue::from_value(value.to_value()).unwrap();
        assert_eq!(target.providers.len(), 7);
        for name in [
            "DefaultInfo",
            "ToolchainInfo",
            "OutputGroupInfo",
            "RunEnvironmentInfo",
        ] {
            let callable = alloc_starlark_provider_callable(&heap, name).unwrap();
            assert!(target.is_in(callable.to_value()).unwrap(), "{name}");
            let expected = target
                .providers
                .get(&ProviderIdentity::builtin(name))
                .unwrap()
                .to_value();
            Heap::temp(|scratch| {
                assert!(
                    target
                        .at(callable.to_value(), scratch)
                        .unwrap()
                        .ptr_eq(expected)
                );
            });
        }

        let default = builtin(target, "DefaultInfo");
        assert_eq!(
            default.dir_attr(),
            ["data_runfiles", "default_runfiles", "files", "files_to_run"]
        );
        let default_files =
            StarlarkDepset::direct_from_value(field(default, "files").to_value()).unwrap();
        assert_eq!(
            AnalysisArtifactValue::from_value(default_files[0])
                .unwrap()
                .artifact()
                .path()
                .as_ref(),
            "bin/tool"
        );
        let files_to_run =
            StarlarkFilesToRunProvider::from_value(field(default, "files_to_run").to_value())
                .unwrap();
        assert_eq!(
            AnalysisArtifactValue::from_value(files_to_run.executable.to_value())
                .unwrap()
                .artifact()
                .path()
                .as_ref(),
            "bin/tool"
        );
        assert!(files_to_run.runfiles_manifest.to_value().is_none());
        let output_groups = target
            .providers
            .get(&ProviderIdentity::builtin("OutputGroupInfo"))
            .unwrap();
        let fields = StarlarkOutputGroupInfo::fields_from_value(output_groups.to_value()).unwrap();
        assert_eq!(fields[0].0, "validation");
        let file = StarlarkDepset::direct_from_value(fields[0].1).unwrap()[0];
        assert_eq!(
            AnalysisArtifactValue::from_starlark(file)
                .unwrap()
                .artifact()
                .path()
                .as_ref(),
            "pkg/validation.txt"
        );
        let run_environment = *target
            .providers
            .get(&ProviderIdentity::builtin("RunEnvironmentInfo"))
            .unwrap();
        assert_eq!(run_environment.to_value().get_type(), "RunEnvironmentInfo");
        Heap::temp(|scratch| {
            let run_environment = run_environment.to_value();
            let first = run_environment
                .get_attr_error("environment", scratch)
                .unwrap();
            let second = run_environment
                .get_attr_error("environment", scratch)
                .unwrap();
            assert!(!first.ptr_eq(second));
            assert_eq!(DictRef::from_value(first).unwrap().len(), 1);
            let inherited = run_environment
                .get_attr_error("inherited_environment", scratch)
                .unwrap();
            assert_eq!(
                ListRef::from_value(inherited).unwrap()[0].unpack_str(),
                Some("PATH")
            );
        });

        let files_to_run = StarlarkFilesToRunProvider::from_value(
            target
                .providers
                .get(&ProviderIdentity::builtin("FilesToRunProvider"))
                .unwrap()
                .to_value(),
        )
        .unwrap();
        assert_eq!(
            AnalysisArtifactValue::from_value(files_to_run.executable.to_value())
                .unwrap()
                .artifact()
                .path()
                .as_ref(),
            "bin/tool"
        );
        assert!(files_to_run.repo_mapping_manifest.to_value().is_none());
        let platform = builtin(target, "PlatformInfo");
        assert_eq!(
            field(platform, "label").to_value().unpack_str(),
            Some("@@platforms//:host")
        );
        assert_eq!(
            DictRef::from_value(field(platform, "constraints").to_value())
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            DictRef::from_value(field(platform, "exec_properties").to_value())
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn target_files_reuses_default_files_without_executable_or_missing_provider_fallback() {
        // Bazel 9.2 AbstractConfiguredTarget.get: files is DefaultInfo.files only.
        let source = AnalysisArtifact::Source(
            slug_identity_v2::CanonicalLabel::parse("@@//pkg:source.txt").unwrap(),
        );
        let owner = AnalysisConfiguredTargetKey::new(
            slug_identity_v2::CanonicalLabel::parse("@@//pkg:producer").unwrap(),
            b"configured".as_slice(),
        );
        let generated = AnalysisArtifact::Derived {
            owner: owner.clone(),
            output: slug_build_api_v2::ActionOutput::new(
                test_artifact_root(),
                "pkg/generated.txt",
                slug_build_api_v2::ActionOutputKind::File,
            ),
        };
        let executable = AnalysisArtifact::Source(
            slug_identity_v2::CanonicalLabel::parse("@@//pkg:tool").unwrap(),
        );
        let files = AnalysisDepset::new(
            DepsetOrder::Default,
            vec![
                AnalysisValue::artifact(generated.clone()),
                AnalysisValue::artifact(source.clone()),
            ],
            vec![],
        )
        .unwrap();
        let info = DefaultInfo::from_executable(executable.clone(), Some(files.clone())).unwrap();
        assert_eq!(info.files_to_run.executable.as_ref(), Some(&executable));
        let heap = FrozenHeap::new();
        let mut materializer = AnalysisValueMaterializer::new(&heap);
        let value = materializer
            .target(&ConfiguredTargetValue::new(
                owner.clone(),
                ProviderCollection::new(vec![ProviderValue::DefaultInfo(info)]).unwrap(),
            ))
            .unwrap();
        let target = AnalysisConfiguredTargetValue::from_value(value.to_value()).unwrap();
        Heap::temp(|scratch| {
            let projected = target.get_attr("files", scratch).unwrap();
            assert!(projected.ptr_eq(field(builtin(target, "DefaultInfo"), "files").to_value()));
            let lowered = AnalysisValueLowerer::default()
                .lower(projected, "$files")
                .unwrap();
            assert!(AnalysisValue::depset(files).publication_eq(&lowered));
            let artifacts = StarlarkDepset::direct_from_value(projected).unwrap();
            assert_eq!(
                artifacts
                    .iter()
                    .map(|file| AnalysisArtifactValue::from_value(*file)
                        .unwrap()
                        .artifact()
                        .clone())
                    .collect::<Vec<_>>(),
                [generated, source]
            );
            assert!(target.get_attr("unknown", scratch).is_none());
        });
        assert_eq!(target.dir_attr(), ["files", "label"]);
        for (providers, has_files) in [
            (
                ProviderCollection::new(vec![ProviderValue::DefaultInfo(DefaultInfo::empty())])
                    .unwrap(),
                true,
            ),
            (
                ProviderCollection::from_values(vec![], false).unwrap(),
                false,
            ),
        ] {
            let other = materializer
                .target(&ConfiguredTargetValue::new(owner.clone(), providers))
                .unwrap();
            let other_target = AnalysisConfiguredTargetValue::from_value(other.to_value()).unwrap();
            assert!(target.equals(other.to_value()).unwrap());
            let mut original_hash = StarlarkHasher::new();
            let mut other_hash = StarlarkHasher::new();
            target.write_hash(&mut original_hash).unwrap();
            other_target.write_hash(&mut other_hash).unwrap();
            assert_eq!(original_hash.finish_small(), other_hash.finish_small());
            assert_eq!(
                other_target.dir_attr().contains(&"files".to_owned()),
                has_files
            );
            Heap::temp(|scratch| {
                let default = alloc_starlark_provider_callable(&heap, "DefaultInfo").unwrap();
                assert_eq!(other_target.is_in(default.to_value()).unwrap(), has_files);
                if has_files {
                    assert!(
                        StarlarkDepset::direct_from_value(
                            other_target.get_attr("files", scratch).unwrap()
                        )
                        .unwrap()
                        .is_empty()
                    );
                } else {
                    assert!(other_target.get_attr("files", scratch).is_none());
                    assert!(other_target.at(default.to_value(), scratch).is_err());
                }
                assert!(
                    other_target
                        .at(scratch.alloc("DefaultInfo"), scratch)
                        .is_err()
                );
            });
        }
    }

    #[test]
    fn null_target_and_artifact_backed_builtin_provider_survive_nested_round_trip() {
        let label = slug_identity_v2::CanonicalLabel::parse("@@//pkg:source.txt").unwrap();
        let file_providers = |artifact: AnalysisArtifact| {
            let files = AnalysisDepset::new(
                DepsetOrder::Default,
                vec![AnalysisValue::artifact(artifact)],
                Vec::new(),
            )
            .unwrap();
            ProviderCollection::new(vec![ProviderValue::DefaultInfo(
                DefaultInfo::from_files(files).unwrap(),
            )])
            .unwrap()
        };
        let source = AnalysisValue::configured_target(ConfiguredTargetValue::new(
            slug_build_api_v2::AnalysisTargetIdentity::null(label.clone()),
            file_providers(AnalysisArtifact::Source(label.clone())),
        ));
        let generated_owner = AnalysisConfiguredTargetKey::new(
            slug_identity_v2::CanonicalLabel::parse("@@//pkg:generator").unwrap(),
            b"configured".as_slice(),
        );
        let generated = AnalysisValue::configured_target(ConfiguredTargetValue::new(
            AnalysisConfiguredTargetKey::new(
                slug_identity_v2::CanonicalLabel::parse("@@//pkg:generated.txt").unwrap(),
                b"configured".as_slice(),
            ),
            file_providers(AnalysisArtifact::Derived {
                owner: generated_owner,
                output: slug_build_api_v2::ActionOutput::new(
                    test_artifact_root(),
                    "pkg/generated.txt",
                    slug_build_api_v2::ActionOutputKind::File,
                ),
            }),
        ));
        let executable = AnalysisValue::provider(
            FilesToRunProvider::single_executable_without_support(AnalysisArtifact::Source(label))
                .to_occurrence(),
        );
        let nested = AnalysisValue::provider(ProviderOccurrence::new(
            ProviderIdentity::user(ProviderId::new("//rules:defs.bzl", "Info").unwrap()),
            [
                ("source_target", source),
                ("generated_target", generated),
                ("executable", executable),
            ],
        ));

        let first_heap = FrozenHeap::new();
        let first = AnalysisValueMaterializer::new(&first_heap)
            .value(&nested)
            .unwrap();
        let lowered = AnalysisValueLowerer::default()
            .lower(first.to_value(), "$nested")
            .unwrap();
        assert!(nested.publication_eq(&lowered));
        drop(first_heap);

        let second_heap = FrozenHeap::new();
        let second = AnalysisValueMaterializer::new(&second_heap)
            .value(&lowered)
            .unwrap();
        for (name, value) in starlark_user_provider_fields(second.to_value()).unwrap().1 {
            if name.ends_with("_target") {
                let target = AnalysisConfiguredTargetValue::from_value(value).unwrap();
                Heap::temp(|scratch| {
                    let files = target.get_attr("files", scratch).unwrap();
                    assert!(
                        files.ptr_eq(field(builtin(target, "DefaultInfo"), "files").to_value())
                    );
                    let retained_files =
                        target.retained.providers().default_info().unwrap().files();
                    assert!(
                        AnalysisValue::depset(retained_files.clone()).publication_eq(
                            &AnalysisValueLowerer::default()
                                .lower(files, "$nested.files")
                                .unwrap(),
                        )
                    );
                });
            }
        }
        let lowered_again = AnalysisValueLowerer::default()
            .lower(second.to_value(), "$nested_again")
            .unwrap();
        assert!(lowered.publication_eq(&lowered_again));
        let AnalysisValueKind::Provider(info) = lowered_again.kind() else {
            panic!("round trip retained the user provider")
        };
        for (field, expected, null_identity) in [
            ("source_target", "pkg/source.txt", true),
            ("generated_target", "pkg/generated.txt", false),
        ] {
            let AnalysisValueKind::ConfiguredTarget(target) = info.field(field).unwrap().kind()
            else {
                panic!("round trip retained the nested {field} Target")
            };
            assert_eq!(
                matches!(
                    target.identity(),
                    slug_build_api_v2::AnalysisTargetIdentity::Null(_)
                ),
                null_identity
            );
            let artifacts = target
                .providers()
                .default_info()
                .expect("round trip retained file DefaultInfo")
                .file_artifacts();
            assert_eq!(artifacts.len(), 1);
            let expected = if null_identity {
                expected.to_owned()
            } else {
                format!("{}/{expected}", test_artifact_root().path())
            };
            assert_eq!(artifacts[0].path().as_ref(), expected);
        }
    }

    #[test]
    fn testing_bootstrap_builtin_keys_rematerialize_through_the_shared_provider_owner() {
        let names = [
            "ExecutionInfo",
            "InstrumentedFilesInfo",
            "AnalysisFailureInfo",
            "AnalysisTestResultInfo",
        ];
        let mut values = vec![ProviderValue::DefaultInfo(DefaultInfo::empty())];
        values.extend(names.into_iter().map(|name| {
            ProviderValue::Occurrence(if name == "InstrumentedFilesInfo" {
                ProviderOccurrence::new(
                    ProviderIdentity::builtin(name),
                    ["instrumented_files", "metadata_files"].map(|field| {
                        (
                            field,
                            AnalysisValue::depset(AnalysisDepset::empty(
                                slug_build_api_v2::DepsetOrder::Default,
                            )),
                        )
                    }),
                )
            } else {
                ProviderOccurrence::empty(ProviderIdentity::builtin(name))
            })
        }));
        let providers = ProviderCollection::new(values).unwrap();
        let retained = ConfiguredTargetValue::new(
            AnalysisConfiguredTargetKey::new(
                slug_identity_v2::CanonicalLabel::parse("@@//:testing").unwrap(),
                [7, 8, 9],
            ),
            providers,
        );
        let heap = FrozenHeap::new();
        let value = AnalysisValueMaterializer::new(&heap)
            .target(&retained)
            .unwrap();
        let target = AnalysisConfiguredTargetValue::from_value(value.to_value()).unwrap();
        assert_eq!(target.providers.len(), names.len() + 1);
        for name in names {
            let token = alloc_starlark_provider_callable(&heap, name).unwrap();
            assert_eq!(
                starlark_provider_identity(token.to_value()),
                Some(ProviderIdentity::builtin(name))
            );
            assert!(target.is_in(token.to_value()).unwrap(), "{name}");
            let expected = target
                .providers
                .get(&ProviderIdentity::builtin(name))
                .unwrap()
                .to_value();
            Heap::temp(|scratch| {
                assert!(
                    target
                        .at(token.to_value(), scratch)
                        .unwrap()
                        .ptr_eq(expected),
                    "{name}"
                );
            });
        }
    }

    #[test]
    fn depset_lowering_dense_packs_local_diamond_and_retains_external_child() {
        let external = AnalysisDepset::new(
            DepsetOrder::Default,
            vec![
                AnalysisValue::string("external-a"),
                AnalysisValue::string("external-b"),
            ],
            Vec::new(),
        )
        .unwrap();
        let lowered = {
            let heap = FrozenHeap::new();
            let direct = |value: &str| {
                StarlarkDepsetSuccessorGen::Direct(heap.alloc_str(value).to_frozen_value())
            };
            let local = |successors, depth| {
                alloc_starlark_depset_parts(
                    &heap,
                    DepsetOrder::Default,
                    Some(CompactString::new("string")),
                    slug_build_api_v2::AnalysisDepsetOccurrence::new(),
                    None,
                    depth,
                    successors,
                )
            };
            let shared = local(vec![direct("shared-a"), direct("shared-b")], 2);
            let left = local(
                vec![
                    StarlarkDepsetSuccessorGen::Transitive(shared),
                    direct("left"),
                ],
                3,
            );
            let right = local(
                vec![
                    StarlarkDepsetSuccessorGen::Transitive(shared),
                    direct("right"),
                ],
                3,
            );
            let external_value = alloc_starlark_depset(&heap, external.dupe(), Vec::new());
            let root = local(
                vec![
                    StarlarkDepsetSuccessorGen::Transitive(left),
                    StarlarkDepsetSuccessorGen::Transitive(right),
                    StarlarkDepsetSuccessorGen::Transitive(external_value),
                    direct("root"),
                ],
                4,
            );
            AnalysisValueLowerer::default()
                .lower(root.to_value(), "$dense")
                .unwrap()
        };

        let AnalysisValueKind::Depset(root) = lowered.kind() else {
            panic!("lowering retained a depset")
        };
        let children = root
            .successors()
            .filter_map(|successor| match successor {
                AnalysisDepsetSuccessor::Transitive(child) => Some(child),
                AnalysisDepsetSuccessor::Direct(_) => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(children.len(), 3);
        assert!(root.shares_store_with(&children[0]));
        assert!(root.shares_store_with(&children[1]));
        assert!(!root.shares_store_with(&children[2]));
        assert!(children[2].shares_occurrence_with(&external));

        let left_shared = children[0]
            .successors()
            .find_map(|successor| match successor {
                AnalysisDepsetSuccessor::Transitive(child) => Some(child),
                AnalysisDepsetSuccessor::Direct(_) => None,
            })
            .unwrap();
        let right_shared = children[1]
            .successors()
            .find_map(|successor| match successor {
                AnalysisDepsetSuccessor::Transitive(child) => Some(child),
                AnalysisDepsetSuccessor::Direct(_) => None,
            })
            .unwrap();
        assert!(left_shared.shares_occurrence_with(&right_shared));
        assert!(left_shared.shares_store_with(root));
        assert_eq!(
            root.to_list()
                .iter()
                .map(|value| value.as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "shared-a",
                "shared-b",
                "left",
                "right",
                "external-a",
                "external-b",
                "root",
            ]
        );
    }

    #[test]
    fn depset_lowering_packs_child_seen_before_parent_in_one_enclosing_value() {
        let lowered = {
            let heap = FrozenHeap::new();
            let child = alloc_starlark_depset_parts(
                &heap,
                DepsetOrder::Default,
                Some(CompactString::new("string")),
                slug_build_api_v2::AnalysisDepsetOccurrence::new(),
                None,
                2,
                vec![
                    StarlarkDepsetSuccessorGen::Direct(heap.alloc_str("child-a").to_frozen_value()),
                    StarlarkDepsetSuccessorGen::Direct(heap.alloc_str("child-b").to_frozen_value()),
                ],
            );
            let parent = alloc_starlark_depset_parts(
                &heap,
                DepsetOrder::Default,
                Some(CompactString::new("string")),
                slug_build_api_v2::AnalysisDepsetOccurrence::new(),
                None,
                3,
                vec![
                    StarlarkDepsetSuccessorGen::Transitive(child),
                    StarlarkDepsetSuccessorGen::Direct(heap.alloc_str("parent").to_frozen_value()),
                ],
            );
            let enclosing = heap.alloc(AllocTuple([child, parent]));
            AnalysisValueLowerer::default()
                .lower(enclosing.to_value(), "$child_parent")
                .unwrap()
        };
        let AnalysisValueKind::Tuple(values) = lowered.kind() else {
            panic!("enclosing value retained its tuple")
        };
        let AnalysisValueKind::Depset(child) = values[0].kind() else {
            panic!("first tuple field retained child depset")
        };
        let AnalysisValueKind::Depset(parent) = values[1].kind() else {
            panic!("second tuple field retained parent depset")
        };
        assert!(child.shares_store_with(parent));
        let parent_child = parent
            .successors()
            .find_map(|successor| match successor {
                AnalysisDepsetSuccessor::Transitive(child) => Some(child),
                AnalysisDepsetSuccessor::Direct(_) => None,
            })
            .unwrap();
        assert!(parent_child.shares_occurrence_with(child));
        assert_eq!(
            parent
                .to_list()
                .iter()
                .map(|value| value.as_str().unwrap())
                .collect::<Vec<_>>(),
            ["child-a", "child-b", "parent"]
        );
    }
    #[test]
    fn empty_header_info_round_trip_preserves_native_type_and_alias_identity() {
        use slug_build_api_v2::CcHeaderInfo;
        let heap = FrozenHeap::new();
        let token = CcHeaderInfo::create(Default::default(), false, &[]).unwrap();
        let a = slug_loading_v2::alloc_frozen_cc_header_info(&heap, token.clone());
        let alias = slug_loading_v2::alloc_frozen_cc_header_info(&heap, token);
        let b = slug_loading_v2::alloc_frozen_cc_header_info(
            &heap,
            CcHeaderInfo::create(Default::default(), false, &[]).unwrap(),
        );
        let source = heap.alloc(AllocTuple([a, alias, b]));
        let retained = AnalysisValueLowerer::default()
            .lower(source.to_value(), "header")
            .unwrap();
        let target_heap = FrozenHeap::new();
        let restored = AnalysisValueMaterializer::new(&target_heap)
            .value(&retained)
            .unwrap();
        let values = TupleRef::from_value(restored.to_value())
            .unwrap()
            .iter()
            .collect::<Vec<_>>();
        assert_eq!(values[0].get_type(), "HeaderInfo");
        assert!(values[0].equals(values[1]).unwrap());
        assert!(!values[0].equals(values[2]).unwrap());
        assert_eq!(
            values[0].get_hashed().unwrap().hash(),
            values[1].get_hashed().unwrap().hash()
        );
        let module = starlark::environment::Module::new();
        assert!(
            values[0]
                .get_attr("header_module", module.heap())
                .unwrap()
                .unwrap()
                .is_none()
        );
        assert_eq!(
            ListRef::from_value(
                values[0]
                    .get_attr("modular_public_headers", module.heap())
                    .unwrap()
                    .unwrap()
            )
            .unwrap()
            .len(),
            0
        );
        let twice = AnalysisValueLowerer::default()
            .lower(restored.to_value(), "restored")
            .unwrap();
        assert_eq!(retained, twice);
        assert!(retained.publication_eq(&twice));
    }
}

#[cfg(test)]
#[path = "analysis_value/container_alias_tests.rs"]
mod container_alias_tests;
