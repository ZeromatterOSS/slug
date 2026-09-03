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
use slug_build_api_v2::AnalysisDepsetSuccessor;
use slug_build_api_v2::AnalysisNumber;
use slug_build_api_v2::AnalysisTargetIdentity;
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::AnalysisValueKind;
use slug_build_api_v2::ConfiguredTargetValue;
use slug_build_api_v2::Depset;
use slug_build_api_v2::FilesToRunProvider;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::ProviderIdentity;
use slug_build_api_v2::ProviderOccurrence;
use slug_build_api_v2::ProviderValue;
use slug_build_api_v2::RetainedRunfiles;
use slug_build_api_v2::RunfilesSymlink;
use slug_build_api_v2::RunfilesSymlinkDepset;
use slug_loading_v2::provider::StarlarkDepset;
use slug_loading_v2::provider::StarlarkDepsetSuccessorGen;
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
use starlark::starlark_module;
use starlark::values::FrozenHeap;
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
use starlark::values::list::ListRef;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::starlark_value;
use starlark::values::structs::AllocStruct;
use starlark::values::structs::StructRef;
use starlark::values::tuple::AllocTuple;
use starlark::values::tuple::TupleRef;
use starlark_map::small_map::SmallMap;

use crate::key::ConfiguredNodeKey;

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
    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other)
            .is_some_and(|other| self.retained.is_empty() && other.retained.is_empty()))
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
        (name == "label").then(|| {
            slug_loading_v2::provider::alloc_starlark_label(
                heap,
                self.retained.identity().label().clone(),
            )
        })
    }

    fn dir_attr(&self) -> Vec<String> {
        vec!["label".to_owned()]
    }
}

pub(crate) struct AnalysisValueMaterializer<'a> {
    heap: &'a FrozenHeap,
    depsets: HashMap<AnalysisDepset, FrozenValue>,
    file_depsets: HashMap<(usize, u32), FrozenValue>,
    symlink_depsets: HashMap<(usize, u32), FrozenValue>,
}

impl<'a> AnalysisValueMaterializer<'a> {
    pub(crate) fn new(heap: &'a FrozenHeap) -> Self {
        Self {
            heap,
            depsets: HashMap::default(),
            file_depsets: HashMap::default(),
            symlink_depsets: HashMap::default(),
        }
    }

    fn provider(&mut self, occurrence: &ProviderOccurrence) -> Result<FrozenValue, String> {
        if let Some(value) = FilesToRunProvider::from_occurrence(occurrence) {
            return Ok(self.files_to_run(&value));
        }
        let fields = occurrence
            .fields()
            .iter()
            .map(|(name, value)| Ok((name.clone(), self.value(value)?)))
            .collect::<Result<Vec<_>, String>>()?;
        Ok(if occurrence.identity().is_builtin("ToolchainInfo") {
            StarlarkToolchainInfo::alloc(self.heap, fields)
        } else if let Some(id) = occurrence.identity().user_id() {
            alloc_starlark_user_provider(self.heap, id.dupe(), fields)
        } else {
            self.heap.alloc(BuiltinProviderView {
                identity: occurrence.identity().clone(),
                fields: fields.into_iter().collect(),
            })
        })
    }

    fn artifact_option(&self, value: Option<&AnalysisArtifact>) -> FrozenValue {
        value
            .map(|value| self.heap.alloc(AnalysisArtifactValue::new(value.clone())))
            .unwrap_or_else(FrozenValue::new_none)
    }

    fn string_map(&self, values: impl IntoIterator<Item = (String, String)>) -> FrozenValue {
        self.heap
            .alloc(AllocDict(values.into_iter().map(|(key, value)| {
                (
                    self.heap.alloc_str(&key).to_frozen_value(),
                    self.heap.alloc_str(&value).to_frozen_value(),
                )
            })))
    }

    fn file_depset(&mut self, value: &Depset<String>) -> FrozenValue {
        if let Some(found) = self.file_depsets.get(&value.node_key()) {
            return *found;
        }
        let successors = value
            .successors()
            .map(|successor| match successor {
                slug_build_api_v2::DepsetSuccessor::Direct(value) => {
                    StarlarkDepsetSuccessorGen::Direct(self.heap.alloc_str(value).to_frozen_value())
                }
                slug_build_api_v2::DepsetSuccessor::Transitive(value) => {
                    StarlarkDepsetSuccessorGen::Transitive(self.file_depset(&value))
                }
            })
            .collect();
        let empty = value
            .is_empty()
            .then(|| AnalysisDepset::empty(value.order()));
        let result = alloc_starlark_depset_parts(
            self.heap,
            value.order(),
            (!value.is_empty()).then(|| CompactString::new("string")),
            empty
                .as_ref()
                .map(AnalysisDepset::occurrence)
                .unwrap_or_default(),
            empty,
            value.depth(),
            successors,
        );
        self.file_depsets.insert(value.node_key(), result);
        result
    }

    fn symlink_depset(&mut self, value: &RunfilesSymlinkDepset) -> FrozenValue {
        let mut stack = vec![(value.clone(), false)];
        while let Some((current, exiting)) = stack.pop() {
            if self.symlink_depsets.contains_key(&current.node_key()) {
                continue;
            }
            if !exiting {
                stack.push((current.clone(), true));
                for successor in current.successors() {
                    if let slug_build_api_v2::DepsetSuccessor::Transitive(child) = successor
                        && !self.symlink_depsets.contains_key(&child.node_key())
                    {
                        stack.push((child, false));
                    }
                }
                continue;
            }
            let successors = current
                .successors()
                .map(|successor| match successor {
                    slug_build_api_v2::DepsetSuccessor::Direct(value) => {
                        let entry = self.heap.alloc(StarlarkRunfilesSymlink {
                            retained: value.clone(),
                            path: self.heap.alloc_str(&value.path).to_frozen_value(),
                            target_file: self
                                .heap
                                .alloc(AnalysisArtifactValue::new(value.artifact.clone())),
                        });
                        StarlarkDepsetSuccessorGen::Direct(entry)
                    }
                    slug_build_api_v2::DepsetSuccessor::Transitive(child) => {
                        StarlarkDepsetSuccessorGen::Transitive(
                            *self
                                .symlink_depsets
                                .get(&child.node_key())
                                .expect("child materialized first"),
                        )
                    }
                })
                .collect();
            let result = alloc_starlark_depset_parts(
                self.heap,
                current.order(),
                (!current.is_empty()).then(|| CompactString::new("SymlinkEntry")),
                Default::default(),
                None,
                current.depth(),
                successors,
            );
            self.symlink_depsets.insert(current.node_key(), result);
        }
        *self
            .symlink_depsets
            .get(&value.node_key())
            .expect("root materialized")
    }

    fn runfiles(&mut self, value: &RetainedRunfiles) -> Result<FrozenValue, String> {
        Ok(self.heap.alloc(StarlarkRunfiles {
            retained: value.clone(),
            files: self.depset(&value.files)?,
            symlinks: self.symlink_depset(&value.symlinks),
            root_symlinks: self.symlink_depset(&value.root_symlinks),
            empty_filenames: self.file_depset(&value.empty_filenames),
        }))
    }

    fn files_to_run(&self, value: &FilesToRunProvider) -> FrozenValue {
        self.heap.alloc(StarlarkFilesToRunProvider {
            retained: value.clone(),
            executable: self.artifact_option(value.executable.as_ref()),
            runfiles_manifest: self.artifact_option(value.runfiles_manifest()),
            repo_mapping_manifest: self.artifact_option(value.repo_mapping_manifest()),
        })
    }

    fn builtin(
        &mut self,
        identity: &ProviderIdentity,
        value: &ProviderValue,
    ) -> Result<FrozenValue, String> {
        let mut fields = SmallMap::new();
        match value {
            ProviderValue::DefaultInfo(info) => {
                fields.insert("files".into(), self.depset(info.files())?);
                fields.insert(
                    "default_runfiles".into(),
                    self.runfiles(&info.default_runfiles)?,
                );
                fields.insert("data_runfiles".into(), self.runfiles(&info.data_runfiles)?);
                fields.insert("files_to_run".into(), self.files_to_run(&info.files_to_run));
            }
            ProviderValue::OutputGroupInfo(info) => {
                for (name, files) in &info.groups {
                    fields.insert(name.as_str().into(), self.file_depset(files));
                }
            }
            ProviderValue::RunEnvironmentInfo(info) => {
                fields.insert(
                    "environment".into(),
                    self.string_map(info.environment.clone()),
                );
                fields.insert(
                    "inherited_environment".into(),
                    self.heap.alloc(
                        info.inherited_environment
                            .iter()
                            .map(|value| self.heap.alloc_str(value).to_frozen_value())
                            .collect::<Vec<_>>(),
                    ),
                );
            }
            ProviderValue::FilesToRunProvider(info) => {
                return Ok(self.files_to_run(info));
            }
            ProviderValue::PlatformInfo(info) => {
                fields.insert(
                    "label".into(),
                    self.heap.alloc_str(&info.label).to_frozen_value(),
                );
                fields.insert(
                    "constraints".into(),
                    self.string_map(info.constraints.clone()),
                );
                fields.insert(
                    "exec_properties".into(),
                    self.string_map(info.exec_properties.clone()),
                );
            }
            ProviderValue::Occurrence(_) => unreachable!("occurrences use the shared classes"),
        }
        Ok(self.heap.alloc(BuiltinProviderView {
            identity: identity.clone(),
            fields,
        }))
    }

    fn target(&mut self, target: &ConfiguredTargetValue) -> Result<FrozenValue, String> {
        let mut providers = SmallMap::new();
        for (identity, provider) in target.providers().iter() {
            let value = match provider {
                ProviderValue::Occurrence(occurrence) => self.provider(occurrence)?,
                provider => self.builtin(identity, provider)?,
            };
            providers.insert(identity.clone(), value);
        }
        Ok(self.heap.alloc(AnalysisConfiguredTargetValue {
            retained: target.clone(),
            providers,
        }))
    }

    fn depset(&mut self, value: &AnalysisDepset) -> Result<FrozenValue, String> {
        let mut stack = vec![(value.clone(), false)];
        while let Some((current, exiting)) = stack.pop() {
            if self.depsets.contains_key(&current) {
                continue;
            }
            if !exiting {
                stack.push((current.clone(), true));
                for successor in current.successors() {
                    if let AnalysisDepsetSuccessor::Transitive(child) = successor
                        && !self.depsets.contains_key(&child)
                    {
                        stack.push((child, false));
                    }
                }
                continue;
            }
            let successors = current
                .successors()
                .map(|successor| match successor {
                    AnalysisDepsetSuccessor::Direct(value) => {
                        self.value(value).map(StarlarkDepsetSuccessorGen::Direct)
                    }
                    AnalysisDepsetSuccessor::Transitive(child) => {
                        Ok(StarlarkDepsetSuccessorGen::Transitive(
                            *self.depsets.get(&child).expect("child materialized first"),
                        ))
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            let result = alloc_starlark_depset(self.heap, current.clone(), successors);
            self.depsets.insert(current, result);
        }
        Ok(*self.depsets.get(value).expect("root materialized"))
    }

    pub(crate) fn value(&mut self, value: &AnalysisValue) -> Result<FrozenValue, String> {
        Ok(match value.kind() {
            AnalysisValueKind::None => FrozenValue::new_none(),
            AnalysisValueKind::Boolean(value) => FrozenValue::new_bool(value),
            AnalysisValueKind::Number(AnalysisNumber::Integer(value)) => {
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
                self.heap.alloc(f64::from_bits(*bits))
            }
            AnalysisValueKind::String(value) => self.heap.alloc_str(value).to_frozen_value(),
            AnalysisValueKind::Label(value) => {
                alloc_frozen_starlark_label(self.heap, value.clone())
            }
            AnalysisValueKind::ConfiguredTarget(value) => self.target(value)?,
            AnalysisValueKind::Artifact(value) => {
                self.heap.alloc(AnalysisArtifactValue::new(value.clone()))
            }
            AnalysisValueKind::List(values) => self.heap.alloc(
                values
                    .iter()
                    .map(|value| self.value(value))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            AnalysisValueKind::Tuple(values) => self.heap.alloc(AllocTuple(
                values
                    .iter()
                    .map(|value| self.value(value))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            AnalysisValueKind::Dictionary(values) => self.heap.alloc(AllocDict(
                values
                    .iter()
                    .map(|(key, value)| Ok((self.value(key)?, self.value(value)?)))
                    .collect::<Result<Vec<_>, String>>()?,
            )),
            AnalysisValueKind::Struct(fields) => self.heap.alloc(AllocStruct(
                fields
                    .iter()
                    .map(|(name, value)| Ok((name.to_string(), self.value(value)?)))
                    .collect::<Result<Vec<_>, String>>()?,
            )),
            AnalysisValueKind::Provider(value) => self.provider(value)?,
            AnalysisValueKind::Depset(value) => self.depset(value)?,
        })
    }

    pub(crate) fn configured_dependency(
        &mut self,
        key: &ConfiguredNodeKey,
        providers: ProviderCollection,
    ) -> Result<FrozenValue, String> {
        let Some(configured) = key.configured_target() else {
            return Ok(self
                .heap
                .alloc(AnalysisArtifactValue::new(AnalysisArtifact::Source(
                    key.label().clone(),
                ))));
        };
        self.target(&ConfiguredTargetValue::new(
            AnalysisConfiguredTargetKey::new(
                configured.label().clone(),
                configured.configuration().complete_identity_bytes(),
            ),
            providers,
        ))
    }

    pub(crate) fn configured_dependency_target(
        &mut self,
        key: &ConfiguredNodeKey,
        providers: ProviderCollection,
    ) -> Result<FrozenValue, String> {
        let identity = key.configured_target().map_or_else(
            || AnalysisTargetIdentity::null(key.label().clone()),
            |configured| {
                AnalysisConfiguredTargetKey::new(
                    configured.label().clone(),
                    configured.configuration().complete_identity_bytes(),
                )
                .into()
            },
        );
        self.target(&ConfiguredTargetValue::new(identity, providers))
    }
}

#[derive(Default)]
pub(crate) struct AnalysisValueLowerer<'v> {
    visiting: HashSet<ValueIdentity<'v>>,
    memo: HashMap<ValueIdentity<'v>, AnalysisValue>,
    lower_depth: usize,
}

impl<'v> AnalysisValueLowerer<'v> {
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
            if let Some((_, _, _, retained, _, _)) = StarlarkDepset::parts_from_value(value) {
                if retained.is_none() {
                    roots.push(value);
                }
                continue;
            }
            let provider_fields =
                if let Some(fields) = StarlarkToolchainInfo::fields_from_value(value) {
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
                if local_ids.contains_key(&identity) {
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
        if StarlarkDepset::parts_from_value(value).is_some() {
            return self.lower_depset(value, path);
        }
        let provider = if let Some(fields) = StarlarkToolchainInfo::fields_from_value(value) {
            Some((ProviderIdentity::builtin("ToolchainInfo"), fields))
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
            return Ok(AnalysisValue::provider(ProviderOccurrence::new(
                identity, fields,
            )));
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
mod tests {
    use std::collections::BTreeMap;

    use slug_build_api_v2::DefaultInfo;
    use slug_build_api_v2::DepsetOrder;
    use slug_build_api_v2::FilesToRunProvider;
    use slug_build_api_v2::OutputGroupInfo;
    use slug_build_api_v2::PlatformInfo;
    use slug_build_api_v2::ProviderId;
    use slug_build_api_v2::RunEnvironmentInfo;
    use slug_loading_v2::provider::DeclarationOnlyAppleProviderKey;
    use slug_loading_v2::provider::DeclarationOnlyAppleProviderKind;
    use slug_loading_v2::provider::alloc_starlark_provider_callable;
    use slug_loading_v2::provider::starlark_provider_identity;
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
            ProviderValue::OutputGroupInfo(OutputGroupInfo::new(BTreeMap::from([(
                "validation".to_owned(),
                Depset::from_direct(
                    slug_build_api_v2::DepsetOrder::Default,
                    vec!["pkg/validation.txt".to_owned()],
                )
                .unwrap(),
            )]))),
            ProviderValue::RunEnvironmentInfo(RunEnvironmentInfo {
                environment: BTreeMap::from([("KEY".to_owned(), "value".to_owned())]),
                inherited_environment: vec!["PATH".to_owned()],
            }),
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
        let output_groups = builtin(target, "OutputGroupInfo");
        assert_eq!(output_groups.dir_attr(), ["validation"]);
        assert_eq!(
            StarlarkDepset::direct_from_value(field(output_groups, "validation").to_value())
                .unwrap()[0]
                .unpack_str(),
            Some("pkg/validation.txt")
        );
        let run_environment = builtin(target, "RunEnvironmentInfo");
        let environment =
            DictRef::from_value(field(run_environment, "environment").to_value()).unwrap();
        assert_eq!(environment.len(), 1);
        let inherited =
            ListRef::from_value(field(run_environment, "inherited_environment").to_value())
                .unwrap();
        assert_eq!(inherited[0].unpack_str(), Some("PATH"));

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

        let second_heap = FrozenHeap::new();
        let second = AnalysisValueMaterializer::new(&second_heap)
            .value(&lowered)
            .unwrap();
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
            ProviderValue::Occurrence(ProviderOccurrence::empty(ProviderIdentity::builtin(name)))
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
    fn declaration_only_apple_keys_reject_target_operations_before_lookup() {
        let occurrence = |name| {
            ProviderValue::Occurrence(ProviderOccurrence::empty(ProviderIdentity::builtin(name)))
        };
        let providers = ProviderCollection::new(vec![
            ProviderValue::DefaultInfo(DefaultInfo::empty()),
            occurrence("ObjcInfo"),
            occurrence("XcodeVersionInfo"),
        ])
        .unwrap();
        let retained = ConfiguredTargetValue::new(
            AnalysisConfiguredTargetKey::new(
                slug_identity_v2::CanonicalLabel::parse("@@//:apple").unwrap(),
                [4, 5, 6],
            ),
            providers,
        );
        let heap = FrozenHeap::new();
        let value = AnalysisValueMaterializer::new(&heap)
            .target(&retained)
            .unwrap();
        let target = AnalysisConfiguredTargetValue::from_value(value.to_value()).unwrap();
        for kind in [
            DeclarationOnlyAppleProviderKind::ObjcInfo,
            DeclarationOnlyAppleProviderKind::XcodeVersionInfo,
        ] {
            let key = heap.alloc(DeclarationOnlyAppleProviderKey(kind));
            let expected = format!(
                "apple_common.{} is declaration-only; configured-target membership and indexing are unsupported",
                kind.names().1
            );
            assert_eq!(
                target.is_in(key.to_value()).unwrap_err().to_string(),
                expected
            );
            Heap::temp(|scratch| {
                assert_eq!(
                    target.at(key.to_value(), scratch).unwrap_err().to_string(),
                    expected
                )
            });
        }
        let present = alloc_starlark_provider_callable(&heap, "DefaultInfo").unwrap();
        let absent = alloc_starlark_provider_callable(&heap, "ToolchainInfo").unwrap();
        assert!(target.is_in(present.to_value()).unwrap());
        assert!(!target.is_in(absent.to_value()).unwrap());
        assert!(!target.is_in(Value::new_none()).unwrap());
        Heap::temp(|scratch| {
            assert!(target.at(present.to_value(), scratch).is_ok());
            assert!(target.at(absent.to_value(), scratch).is_err());
            assert!(target.at(Value::new_none(), scratch).is_err());
        });
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
}
