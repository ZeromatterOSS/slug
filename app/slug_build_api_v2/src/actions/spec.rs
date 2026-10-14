/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use fxhash::FxHashSet;
use slug_configuration_v2::CanonicalStringMap;
use slug_configuration_v2::NormalizedAbsoluteBazelPath;
use slug_configuration_v2::NormalizedBazelPath;
use slug_configuration_v2::RetainedActionEnvironment;

use super::args_expansion::ExpansionBudget;
use super::args_expansion::FormatSize;
use super::args_expansion::GroupSize;
use super::args_expansion::check_size;
use super::args_expansion::checked_add;
use super::args_expansion::checked_mul;
use super::args_expansion::gcc_needs_escape;
use super::args_expansion::param_lines_size;
use super::args_expansion::shell_needs_quotes;
use super::args_expansion::strings_size;
use crate::actions::BuildInfoTransformSpec;
use crate::actions::RetainedSpawnResourceSet;
use crate::actions::cpp_compile::CppCompileSpec;
use crate::actions::runfiles_support::RunfilesSupportActionSpec;
use crate::actions::solib_symlink::SolibSymlinkSpec;
use crate::analysis_value::AnalysisArtifact;
use crate::analysis_value::AnalysisDepset;
use crate::analysis_value::AnalysisValueKind;
use crate::analysis_value::AnalysisValueType;
use crate::analysis_value::PublicationEqState;
use crate::args_mapper::ArgsDirectoryContext;
use crate::args_mapper::ArgsExpansionError;
use crate::args_mapper::NoopArgsDirectoryContext;
use crate::args_mapper::RetainedArgsInputs;
use crate::args_mapper::RetainedArgsMapper;
use crate::providers::FilesToRunProvider;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub enum ActionOutputKind {
    File,
    Directory,
    Symlink,
    RunfilesTree,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub enum ActionOutputMetadata {
    Ordinary,
    Constant,
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
struct ActionOutputData {
    root: crate::DerivedArtifactRoot,
    path: CompactString,
    kind: ActionOutputKind,
    metadata: ActionOutputMetadata,
}

#[derive(Debug, Clone, Dupe, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct ActionOutput(Arc<ActionOutputData>);

impl ActionOutput {
    pub fn new(
        root: crate::DerivedArtifactRoot,
        relative: impl Into<String>,
        kind: ActionOutputKind,
    ) -> Self {
        let path = format!("{}/{}", root.path(), relative.into()).into();
        Self(Arc::new(ActionOutputData {
            root,
            path,
            kind,
            metadata: ActionOutputMetadata::Ordinary,
        }))
    }

    pub fn new_constant(root: crate::DerivedArtifactRoot, relative: impl Into<String>) -> Self {
        let path = format!("{}/{}", root.path(), relative.into()).into();
        Self(Arc::new(ActionOutputData {
            root,
            path,
            kind: ActionOutputKind::File,
            metadata: ActionOutputMetadata::Constant,
        }))
    }

    pub fn metadata(&self) -> ActionOutputMetadata {
        self.0.metadata
    }

    pub fn root(&self) -> &crate::DerivedArtifactRoot {
        &self.0.root
    }

    pub fn root_relative_path(&self) -> &str {
        &self.0.path[self.root().path().len() + 1..]
    }

    pub fn path(&self) -> &str {
        &self.0.path
    }

    pub fn kind(&self) -> ActionOutputKind {
        self.0.kind
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct ActionInput {
    path: String,
    digest: Option<String>,
}

impl ActionInput {
    pub fn new(path: impl Into<String>, digest: Option<String>) -> Self {
        Self {
            path: path.into(),
            digest,
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn digest(&self) -> Option<&str> {
        self.digest.as_deref()
    }
}

#[derive(Debug, Clone, Dupe, Allocative)]
pub struct RetainedArtifactInputs(AnalysisDepset);

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RetainedArtifactInputsError {
    value_type: AnalysisValueType,
}

impl RetainedArtifactInputsError {
    pub fn value_type(&self) -> AnalysisValueType {
        self.value_type
    }
}

impl fmt::Display for RetainedArtifactInputsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "action inputs require a depset of File, got depset of {}",
            self.value_type
        )
    }
}

impl Error for RetainedArtifactInputsError {}

impl RetainedArtifactInputs {
    pub fn new(depset: AnalysisDepset) -> Result<Self, RetainedArtifactInputsError> {
        match depset.element_type() {
            AnalysisValueType::Empty | AnalysisValueType::Artifact => Ok(Self(depset)),
            value_type => Err(RetainedArtifactInputsError { value_type }),
        }
    }

    /// Retained producer order; does not flatten inputs or extend visitor loans.
    pub fn order(&self) -> crate::DepsetOrder {
        self.0.order()
    }

    pub fn visit(
        &self,
        mut visitor: impl FnMut(&AnalysisArtifact),
    ) -> Result<(), RetainedArtifactInputsError> {
        let mut invalid = None;
        self.0
            .visit(|value| {
                match value.kind() {
                    AnalysisValueKind::Artifact(artifact) => visitor(artifact),
                    _ => invalid = Some(value.value_type()),
                }
                Ok::<_, Infallible>(())
            })
            .unwrap_or_else(|never| match never {});
        match invalid {
            Some(value_type) => Err(RetainedArtifactInputsError { value_type }),
            None => Ok(()),
        }
    }

    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.0.publication_eq_with(&other.0, state)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub enum RetainedScalarValue {
    String(CompactString),
    Integer(CompactString),
    Artifact(AnalysisArtifact),
}

impl RetainedScalarValue {
    pub fn render(&self) -> String {
        match self {
            Self::String(value) | Self::Integer(value) => value.to_string(),
            Self::Artifact(artifact) => artifact.path().into_owned(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct RetainedScalarArg {
    arg_name: Option<CompactString>,
    value: RetainedScalarValue,
    format: Option<CompactString>,
}

impl RetainedScalarArg {
    pub fn new(
        arg_name: Option<impl Into<CompactString>>,
        value: RetainedScalarValue,
        format: Option<impl Into<CompactString>>,
    ) -> Self {
        Self {
            arg_name: arg_name.map(Into::into),
            value,
            format: format.map(Into::into),
        }
    }

    fn render_group(
        &self,
        budget: &mut ExpansionBudget,
        flag_per_line: bool,
    ) -> Result<Vec<String>, ArgsExpansionError> {
        let value_len = match &self.value {
            RetainedScalarValue::String(value) | RetainedScalarValue::Integer(value) => value.len(),
            RetainedScalarValue::Artifact(value) => value.path().len(),
        };
        let format = self.format.as_deref().map(FormatSize::new).transpose()?;
        let value_size = format.map_or(Ok(value_len), |format| format.size(value_len))?;
        let mut size = GroupSize::default();
        if let Some(name) = &self.arg_name {
            size.add(name.len())?;
        }
        size.add(value_size)?;
        size.charge(budget, flag_per_line)?;
        let mut argv = Vec::with_capacity(2);
        if let Some(name) = &self.arg_name {
            argv.push(name.to_string());
        }
        let value = self.value.render();
        argv.push(match &self.format {
            Some(format) => apply_validated_format_sized(format, &value, value_size),
            None => value,
        });
        Ok(argv)
    }
}

#[derive(Debug, Clone, Dupe, Allocative)]
pub struct RetainedArgsDepset(AnalysisDepset);

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RetainedArgsDepsetError {
    ValueType(AnalysisValueType),
    Directory,
    NonRegularFile,
    FilesToRun,
}

impl fmt::Display for RetainedArgsDepsetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ValueType(value_type) => write!(
                f,
                "Args vector requires strings, integers, or regular Files, got depset of {value_type}"
            ),
            Self::Directory => write!(f, "Args vector directory expansion is not supported"),
            Self::NonRegularFile => write!(f, "Args file dirname mapping requires regular Files"),
            Self::FilesToRun => write!(f, "Args file dirname mapping does not accept FilesToRun"),
        }
    }
}

impl Error for RetainedArgsDepsetError {}

impl RetainedArgsDepset {
    pub fn new(depset: AnalysisDepset) -> Result<Self, RetainedArgsDepsetError> {
        Self::validate(depset, true)
    }

    /// Retain literal Directory paths for a vector with directory expansion disabled.
    pub fn new_unexpanded(depset: AnalysisDepset) -> Result<Self, RetainedArgsDepsetError> {
        Self::validate(depset, false)
    }

    fn validate(
        depset: AnalysisDepset,
        expand_directories: bool,
    ) -> Result<Self, RetainedArgsDepsetError> {
        match depset.element_type() {
            AnalysisValueType::Empty | AnalysisValueType::String | AnalysisValueType::Integer => {
                Ok(Self(depset))
            }
            AnalysisValueType::Artifact => {
                if !expand_directories {
                    return Ok(Self(depset));
                }
                let mut directory = false;
                depset
                    .visit(|value| {
                        if matches!(
                            value.kind(),
                            AnalysisValueKind::Artifact(AnalysisArtifact::Derived { output, .. })
                                if output.kind() == ActionOutputKind::Directory
                        ) {
                            directory = true;
                        }
                        Ok::<_, Infallible>(())
                    })
                    .unwrap_or_else(|never| match never {});
                if directory {
                    Err(RetainedArgsDepsetError::Directory)
                } else {
                    Ok(Self(depset))
                }
            }
            value_type => Err(RetainedArgsDepsetError::ValueType(value_type)),
        }
    }

    fn render(&self) -> Vec<String> {
        let mut values = Vec::new();
        self.0
            .visit(|value| {
                values.push(match value.kind() {
                    AnalysisValueKind::String(value) => value.to_owned(),
                    AnalysisValueKind::Number(value) => {
                        render_analysis_integer(value.as_integer().expect("validated Args integer"))
                    }
                    AnalysisValueKind::Artifact(artifact) => artifact.path().into_owned(),
                    _ => unreachable!("validated Args depset element type"),
                });
                Ok::<_, Infallible>(())
            })
            .unwrap_or_else(|never| match never {});
        values
    }

    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.0.publication_eq_with(&other.0, state)
    }
}

#[derive(Debug, Clone, Allocative)]
pub enum RetainedVectorSource {
    Sequence(Arc<[RetainedScalarValue]>),
    Depset(RetainedArgsDepset),
    Mapped {
        inputs: RetainedArgsInputs,
        mapper: RetainedArgsMapper,
        registration_location: Option<CompactString>,
    },
}
impl RetainedVectorSource {
    fn render(
        &self,
        context: &dyn ArgsDirectoryContext,
        expand_directories: bool,
    ) -> Result<Vec<String>, ArgsExpansionError> {
        match self {
            Self::Sequence(values) => Ok(values.iter().map(RetainedScalarValue::render).collect()),
            Self::Depset(values) => Ok(values.render()),
            Self::Mapped {
                inputs,
                mapper,
                registration_location,
            } => super::args_expansion::expand_mapped(
                inputs,
                mapper,
                registration_location.as_deref(),
                context,
                expand_directories,
            ),
        }
    }
    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        match (self, other) {
            (Self::Sequence(l), Self::Sequence(r)) => l == r,
            (Self::Depset(l), Self::Depset(r)) => l.publication_eq_with(r, state),
            (
                Self::Mapped {
                    inputs: l,
                    mapper: lm,
                    registration_location: ll,
                },
                Self::Mapped {
                    inputs: r,
                    mapper: rm,
                    registration_location: rl,
                },
            ) => ll == rl && l.publication_eq_with(r, state) && lm.publication_eq_with(rm, state),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct RetainedVectorOptions {
    pub arg_name: Option<CompactString>,
    pub format_each: Option<CompactString>,
    pub before_each: Option<CompactString>,
    pub join_with: Option<CompactString>,
    pub format_joined: Option<CompactString>,
    pub omit_if_empty: bool,
    pub uniquify: bool,
    pub expand_directories: bool,
    pub terminate_with: Option<CompactString>,
}

#[derive(Debug, Clone, Allocative)]
pub struct RetainedVectorArg {
    source: RetainedVectorSource,
    options: RetainedVectorOptions,
}

impl RetainedVectorArg {
    /// The immutable vector source retained at publication.
    pub fn source(&self) -> &RetainedVectorSource {
        &self.source
    }

    pub fn new(source: RetainedVectorSource, options: RetainedVectorOptions) -> Self {
        Self { source, options }
    }

    fn render_values(
        &self,
        context: &dyn ArgsDirectoryContext,
    ) -> Result<Vec<String>, ArgsExpansionError> {
        let mut values = self
            .source
            .render(context, self.options.expand_directories)?;
        if self.options.uniquify {
            let mut seen = FxHashSet::default();
            values.retain(|value| seen.insert(value.clone()));
        }
        strings_size(values.iter().map(String::as_str))?;
        Ok(values)
    }

    fn formatted_sizes(&self, values: &[String]) -> Result<Vec<usize>, ArgsExpansionError> {
        let format = self
            .options
            .format_each
            .as_deref()
            .map(FormatSize::new)
            .transpose()?;
        let mut bytes = 0;
        values
            .iter()
            .map(|value| {
                let size = format.map_or(Ok(value.len()), |format| format.size(value.len()))?;
                bytes = checked_add(bytes, size)?;
                check_size(0, bytes)?;
                Ok(size)
            })
            .collect()
    }

    fn format_values(&self, values: &mut [String], sizes: &[usize]) {
        if let Some(format) = &self.options.format_each {
            for (value, size) in values.iter_mut().zip(sizes) {
                *value = apply_validated_format_sized(format, value, *size);
            }
        }
    }

    fn render_add_all(
        &self,
        context: &dyn ArgsDirectoryContext,
        budget: &mut ExpansionBudget,
        flag_per_line: bool,
    ) -> Result<Vec<String>, ArgsExpansionError> {
        let mut values = self.render_values(context)?;
        if values.is_empty() && self.options.omit_if_empty {
            return Ok(Vec::new());
        }
        let sizes = self.formatted_sizes(&values)?;
        let mut size = GroupSize::default();
        if let Some(name) = &self.options.arg_name {
            size.add(name.len())?;
        }
        for value_size in &sizes {
            if let Some(before) = &self.options.before_each {
                size.add(before.len())?;
            }
            size.add(*value_size)?;
        }
        if let Some(terminate) = &self.options.terminate_with {
            size.add(terminate.len())?;
        }
        size.charge(budget, flag_per_line)?;
        self.format_values(&mut values, &sizes);
        let mut group = Vec::new();
        if let Some(name) = &self.options.arg_name {
            group.push(name.to_string());
        }
        for value in values {
            if let Some(before) = &self.options.before_each {
                group.push(before.to_string());
            }
            group.push(value);
        }
        if let Some(terminate) = &self.options.terminate_with {
            group.push(terminate.to_string());
        }
        Ok(group)
    }

    fn render_add_joined(
        &self,
        context: &dyn ArgsDirectoryContext,
        budget: &mut ExpansionBudget,
        flag_per_line: bool,
    ) -> Result<Vec<String>, ArgsExpansionError> {
        let mut values = self.render_values(context)?;
        if values.is_empty() && self.options.omit_if_empty {
            return Ok(Vec::new());
        }
        let sizes = self.formatted_sizes(&values)?;
        let delimiter = self.options.join_with.as_deref().unwrap_or_default();
        let bytes = sizes
            .iter()
            .try_fold(0, |sum, size| checked_add(sum, *size))?;
        let joined_size = checked_add(
            bytes,
            checked_mul(delimiter.len(), values.len().saturating_sub(1))?,
        )?;
        check_size(0, joined_size)?;
        let final_size = self
            .options
            .format_joined
            .as_deref()
            .map(FormatSize::new)
            .transpose()?
            .map_or(Ok(joined_size), |format| format.size(joined_size))?;
        let mut size = GroupSize::default();
        if let Some(name) = &self.options.arg_name {
            size.add(name.len())?;
        }
        size.add(final_size)?;
        size.charge(budget, flag_per_line)?;
        self.format_values(&mut values, &sizes);
        let mut group = Vec::with_capacity(2);
        if let Some(name) = &self.options.arg_name {
            group.push(name.to_string());
        }
        let joined = values.join(delimiter);
        group.push(match &self.options.format_joined {
            Some(format) => apply_validated_format_sized(format, &joined, final_size),
            None => joined,
        });
        Ok(group)
    }

    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.options == other.options && self.source.publication_eq_with(&other.source, state)
    }
}

#[derive(Debug, Clone, Allocative)]
pub enum RetainedArgCall {
    Scalar(RetainedScalarArg),
    AddAll(RetainedVectorArg),
    AddJoined(RetainedVectorArg),
}

impl RetainedArgCall {
    fn render_group(
        &self,
        context: &dyn ArgsDirectoryContext,
        budget: &mut ExpansionBudget,
        flag_per_line: bool,
    ) -> Result<Vec<String>, ArgsExpansionError> {
        match self {
            Self::Scalar(value) => value.render_group(budget, flag_per_line),
            Self::AddAll(value) => value.render_add_all(context, budget, flag_per_line),
            Self::AddJoined(value) => value.render_add_joined(context, budget, flag_per_line),
        }
    }

    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        match (self, other) {
            (Self::Scalar(left), Self::Scalar(right)) => left == right,
            (Self::AddAll(left), Self::AddAll(right))
            | (Self::AddJoined(left), Self::AddJoined(right)) => {
                left.publication_eq_with(right, state)
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Allocative, Dupe)]
pub enum RetainedParamFileFormat {
    Shell,
    Multiline,
    FlagPerLine,
    Gcc,
}

#[derive(Debug, Clone, Allocative, Dupe)]
pub struct RetainedArgsRecipe {
    calls: Arc<[RetainedArgCall]>,
    write_format: RetainedParamFileFormat,
}

impl RetainedArgsRecipe {
    /// Calls in their original declaration order.
    pub fn calls(&self) -> &[RetainedArgCall] {
        &self.calls
    }

    pub fn new(
        calls: impl Into<Arc<[RetainedArgCall]>>,
        write_format: RetainedParamFileFormat,
    ) -> Self {
        Self {
            calls: calls.into(),
            write_format,
        }
    }

    pub fn render(&self) -> Result<Vec<String>, ArgsExpansionError> {
        self.render_with_context(&NoopArgsDirectoryContext)
    }
    pub fn render_with_context(
        &self,
        context: &dyn ArgsDirectoryContext,
    ) -> Result<Vec<String>, ArgsExpansionError> {
        self.render_with_budget(context, &mut ExpansionBudget::default())
    }
    pub(super) fn render_with_budget(
        &self,
        context: &dyn ArgsDirectoryContext,
        budget: &mut ExpansionBudget,
    ) -> Result<Vec<String>, ArgsExpansionError> {
        let mut argv = Vec::new();
        let flag_per_line = self.write_format == RetainedParamFileFormat::FlagPerLine;
        for call in self.calls.iter() {
            let group = call.render_group(context, budget, flag_per_line)?;
            if flag_per_line {
                argv.extend(flag_per_line_group(group));
            } else {
                argv.extend(group);
            }
        }
        Ok(argv)
    }
    pub fn render_write_content(&self) -> Result<String, ArgsExpansionError> {
        self.render_write_content_with_context(&NoopArgsDirectoryContext)
    }
    pub fn render_write_content_with_context(
        &self,
        context: &dyn ArgsDirectoryContext,
    ) -> Result<String, ArgsExpansionError> {
        let lines = self.render_with_context(context)?;
        let (_, bytes) = self.measure_param_lines(&lines)?;
        Ok(self.serialize_param_lines(&lines, bytes))
    }
    // Deferred callbacks run once. Consumers retain these bounded words for a
    // complete-command serialization preflight before constructing file bytes.
    pub(super) fn prepare_virtual_param_file(
        &self,
        context: &dyn ArgsDirectoryContext,
        budget: &mut ExpansionBudget,
    ) -> Result<(Vec<String>, Vec<String>), ArgsExpansionError> {
        let argv = self.render_with_budget(context, budget)?;
        Ok(
            if self.write_format == RetainedParamFileFormat::FlagPerLine {
                argv.into_iter().partition(|arg| arg.starts_with("--"))
            } else {
                (argv, Vec::new())
            },
        )
    }
    pub(super) fn measure_param_lines(
        &self,
        lines: &[String],
    ) -> Result<(usize, usize), ArgsExpansionError> {
        param_lines_size(lines, self.write_format)
    }
    pub(super) fn serialize_param_lines(&self, lines: &[String], bytes: usize) -> String {
        let mut result = String::with_capacity(bytes);
        for line in lines {
            match self.write_format {
                RetainedParamFileFormat::Shell => push_shell_escaped(&mut result, line),
                RetainedParamFileFormat::Gcc => push_gcc_escaped(&mut result, line),
                RetainedParamFileFormat::Multiline | RetainedParamFileFormat::FlagPerLine => {
                    result.push_str(line)
                }
            }
            result.push('\n');
        }
        result
    }

    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.write_format == other.write_format
            && self.calls.len() == other.calls.len()
            && self
                .calls
                .iter()
                .zip(other.calls.iter())
                .all(|(left, right)| left.publication_eq_with(right, state))
    }
}

fn flag_per_line_group(group: Vec<String>) -> Vec<String> {
    if group.len() < 2 {
        return group;
    }
    let first = &group[0];
    let rest = group[1..].join(" ");
    vec![if first.is_empty() {
        rest
    } else {
        format!("{first}={rest}")
    }]
}

impl PartialEq for RetainedArgsRecipe {
    fn eq(&self, other: &Self) -> bool {
        self.publication_eq_with(other, &mut PublicationEqState::default())
    }
}

impl Eq for RetainedArgsRecipe {}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct RetainedSpawnParamFilePolicy {
    flag_format: CompactString,
    use_always: bool,
}

impl RetainedSpawnParamFilePolicy {
    pub fn new(flag_format: impl Into<CompactString>, use_always: bool) -> Self {
        Self {
            flag_format: flag_format.into(),
            use_always,
        }
    }

    pub fn flag_format(&self) -> &str {
        &self.flag_format
    }

    pub fn use_always(&self) -> bool {
        self.use_always
    }
}

#[derive(Debug, Clone, Allocative)]
pub struct RetainedSpawnArgsSnapshot {
    recipe: RetainedArgsRecipe,
    param_file: Option<RetainedSpawnParamFilePolicy>,
}

impl RetainedSpawnArgsSnapshot {
    pub fn new(
        recipe: RetainedArgsRecipe,
        param_file: Option<RetainedSpawnParamFilePolicy>,
    ) -> Self {
        Self { recipe, param_file }
    }

    pub fn recipe(&self) -> &RetainedArgsRecipe {
        &self.recipe
    }

    pub fn param_file(&self) -> Option<&RetainedSpawnParamFilePolicy> {
        self.param_file.as_ref()
    }

    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.param_file == other.param_file && self.recipe.publication_eq_with(&other.recipe, state)
    }
}

#[derive(Debug, Clone, Allocative)]
pub enum RetainedCommandLineSegment {
    LiteralRun(Arc<[CompactString]>),
    ArgsSnapshot(RetainedSpawnArgsSnapshot),
}

#[derive(Debug, Clone, Allocative, Dupe)]
pub struct RetainedCommandLine(Arc<[RetainedCommandLineSegment]>);

impl RetainedCommandLine {
    pub fn new(segments: impl Into<Arc<[RetainedCommandLineSegment]>>) -> Self {
        Self(segments.into())
    }

    pub fn segments(&self) -> &[RetainedCommandLineSegment] {
        &self.0
    }

    pub fn render(&self) -> Result<Vec<String>, ArgsExpansionError> {
        self.render_with_context(&NoopArgsDirectoryContext)
    }
    pub fn render_with_context(
        &self,
        context: &dyn ArgsDirectoryContext,
    ) -> Result<Vec<String>, ArgsExpansionError> {
        self.render_with_budget(context, &mut ExpansionBudget::default())
    }
    fn render_with_budget(
        &self,
        context: &dyn ArgsDirectoryContext,
        budget: &mut ExpansionBudget,
    ) -> Result<Vec<String>, ArgsExpansionError> {
        let mut argv = Vec::new();
        for segment in self.0.iter() {
            match segment {
                RetainedCommandLineSegment::LiteralRun(values) => {
                    let (words, bytes) = strings_size(values.iter().map(|value| value.as_str()))?;
                    budget.charge(words, bytes)?;
                    argv.extend(values.iter().map(ToString::to_string));
                }
                RetainedCommandLineSegment::ArgsSnapshot(snapshot) => {
                    argv.extend(snapshot.recipe.render_with_budget(context, budget)?);
                }
            }
        }
        Ok(argv)
    }

    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.0.len() == other.0.len()
            && self
                .0
                .iter()
                .zip(other.0.iter())
                .all(|(left, right)| match (left, right) {
                    (
                        RetainedCommandLineSegment::LiteralRun(left),
                        RetainedCommandLineSegment::LiteralRun(right),
                    ) => left == right,
                    (
                        RetainedCommandLineSegment::ArgsSnapshot(left),
                        RetainedCommandLineSegment::ArgsSnapshot(right),
                    ) => left.publication_eq_with(right, state),
                    _ => false,
                })
    }
}

impl PartialEq for RetainedCommandLine {
    fn eq(&self, other: &Self) -> bool {
        self.publication_eq_with(other, &mut PublicationEqState::default())
    }
}

impl Eq for RetainedCommandLine {}

#[derive(Debug, Clone, Allocative)]
pub enum SpawnExecutable {
    Path(NormalizedBazelPath),
    Artifact(AnalysisArtifact),
    FilesToRun(FilesToRunProvider),
}

#[derive(Debug, Clone, Allocative)]
pub enum RetainedSpawnInvocation {
    Executable(SpawnExecutable),
    Shell {
        shell_executable: CompactString,
        command: CompactString,
        pad_dollar_zero: bool,
    },
}

impl RetainedSpawnInvocation {
    pub fn render_prefix(&self) -> Vec<String> {
        match self {
            Self::Executable(executable) => vec![executable.render()],
            Self::Shell {
                shell_executable,
                command,
                pad_dollar_zero,
            } => {
                let mut prefix = vec![
                    shell_executable.to_string(),
                    "-c".to_owned(),
                    command.to_string(),
                ];
                if *pad_dollar_zero {
                    prefix.push(String::new());
                }
                prefix
            }
        }
    }

    pub(super) fn render_prefix_with_budget(
        &self,
        budget: &mut ExpansionBudget,
    ) -> Result<Vec<String>, ArgsExpansionError> {
        match self {
            Self::Executable(executable) => {
                let bytes = match executable {
                    SpawnExecutable::Path(path) => path.as_str().len(),
                    SpawnExecutable::Artifact(artifact) => artifact.path().len(),
                    SpawnExecutable::FilesToRun(provider) => provider
                        .executable
                        .as_ref()
                        .expect("FilesToRun executable validated before retention")
                        .path()
                        .len(),
                };
                budget.charge(1, bytes)?;
            }
            Self::Shell {
                shell_executable,
                command,
                pad_dollar_zero,
            } => {
                budget.charge(
                    3 + usize::from(*pad_dollar_zero),
                    checked_add(checked_add(shell_executable.len(), 2)?, command.len())?,
                )?;
            }
        }
        Ok(self.render_prefix())
    }

    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        match (self, other) {
            (Self::Executable(left), Self::Executable(right)) => {
                left.publication_eq_with(right, state)
            }
            (
                Self::Shell {
                    shell_executable: left_shell,
                    command: left_command,
                    pad_dollar_zero: left_pad,
                },
                Self::Shell {
                    shell_executable: right_shell,
                    command: right_command,
                    pad_dollar_zero: right_pad,
                },
            ) => {
                left_shell == right_shell && left_command == right_command && left_pad == right_pad
            }
            _ => false,
        }
    }
}

impl SpawnExecutable {
    pub fn render(&self) -> String {
        match self {
            Self::Path(path) => path.as_str().to_owned(),
            Self::Artifact(artifact) => artifact.path().into_owned(),
            Self::FilesToRun(provider) => provider
                .executable
                .as_ref()
                .expect("Spawn FilesToRun executable was validated before publication")
                .path()
                .into_owned(),
        }
    }

    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        match (self, other) {
            (Self::Path(left), Self::Path(right)) => left == right,
            (Self::Artifact(left), Self::Artifact(right)) => left == right,
            (Self::FilesToRun(left), Self::FilesToRun(right)) => {
                left.publication_eq_with(right, state)
            }
            _ => false,
        }
    }
}

impl PartialEq for SpawnExecutable {
    fn eq(&self, other: &Self) -> bool {
        self.publication_eq_with(other, &mut PublicationEqState::default())
    }
}

impl Eq for SpawnExecutable {}

impl PartialEq for RetainedSpawnInvocation {
    fn eq(&self, other: &Self) -> bool {
        self.publication_eq_with(other, &mut PublicationEqState::default())
    }
}

impl Eq for RetainedSpawnInvocation {}

#[derive(Debug, Clone, Allocative)]
pub enum ArtifactInputSource {
    Direct(AnalysisArtifact),
    Depset(RetainedArtifactInputs),
    FilesToRun(FilesToRunProvider),
}

impl ArtifactInputSource {
    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        match (self, other) {
            (Self::Direct(left), Self::Direct(right)) => left == right,
            (Self::Depset(left), Self::Depset(right)) => left.publication_eq_with(right, state),
            (Self::FilesToRun(left), Self::FilesToRun(right)) => {
                left.publication_eq_with(right, state)
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Allocative, Dupe)]
pub struct ArtifactInputs(Arc<[ArtifactInputSource]>);

impl ArtifactInputs {
    pub fn new(inputs: impl Into<Arc<[ArtifactInputSource]>>) -> Self {
        Self(inputs.into())
    }

    pub fn sources(&self) -> &[ArtifactInputSource] {
        &self.0
    }

    pub fn visit(
        &self,
        mut visitor: impl FnMut(&AnalysisArtifact),
    ) -> Result<(), RetainedArtifactInputsError> {
        for source in self.0.iter() {
            match source {
                ArtifactInputSource::Direct(artifact) => visitor(artifact),
                ArtifactInputSource::Depset(inputs) => inputs.visit(&mut visitor)?,
                ArtifactInputSource::FilesToRun(provider) => {
                    RetainedArtifactInputs::new(provider.files().dupe())?.visit(&mut visitor)?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.0.len() == other.0.len()
            && self
                .0
                .iter()
                .zip(other.0.iter())
                .all(|(left, right)| left.publication_eq_with(right, state))
    }
}

#[derive(Debug, Clone, Allocative)]
pub struct SpawnSpec {
    invocation: RetainedSpawnInvocation,
    command_line: RetainedCommandLine,
    inputs: ArtifactInputs,
    tools: ArtifactInputs,
    outputs: Arc<[ActionOutput]>,
    unused_inputs_list: Option<AnalysisArtifact>,
    environment: RetainedActionEnvironment,
    execution_requirements: CanonicalStringMap,
    mnemonic: CompactString,
    progress_message: Option<CompactString>,
    resource_set: Option<RetainedSpawnResourceSet>,
}

impl SpawnSpec {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        invocation: RetainedSpawnInvocation,
        command_line: RetainedCommandLine,
        inputs: ArtifactInputs,
        tools: ArtifactInputs,
        outputs: impl Into<Arc<[ActionOutput]>>,
        unused_inputs_list: Option<AnalysisArtifact>,
        environment: RetainedActionEnvironment,
        execution_requirements: CanonicalStringMap,
        mnemonic: impl Into<CompactString>,
        progress_message: Option<impl Into<CompactString>>,
    ) -> Self {
        Self {
            invocation,
            command_line,
            inputs,
            tools,
            outputs: outputs.into(),
            unused_inputs_list,
            environment,
            execution_requirements,
            mnemonic: mnemonic.into(),
            progress_message: progress_message.map(Into::into),
            resource_set: None,
        }
    }

    pub fn with_resource_set(mut self, resource_set: RetainedSpawnResourceSet) -> Self {
        self.resource_set = Some(resource_set);
        self
    }

    pub fn resource_set(&self) -> Option<RetainedSpawnResourceSet> {
        self.resource_set
    }

    pub fn invocation(&self) -> &RetainedSpawnInvocation {
        &self.invocation
    }
    pub fn command_line(&self) -> &RetainedCommandLine {
        &self.command_line
    }
    pub fn inputs(&self) -> &ArtifactInputs {
        &self.inputs
    }
    pub fn tools(&self) -> &ArtifactInputs {
        &self.tools
    }
    pub fn outputs(&self) -> &[ActionOutput] {
        &self.outputs
    }
    pub fn unused_inputs_list(&self) -> Option<&AnalysisArtifact> {
        self.unused_inputs_list.as_ref()
    }
    pub fn environment(&self) -> &RetainedActionEnvironment {
        &self.environment
    }
    pub fn execution_requirements(&self) -> &CanonicalStringMap {
        &self.execution_requirements
    }
    pub fn mnemonic(&self) -> &str {
        &self.mnemonic
    }
    pub fn progress_message(&self) -> Option<&str> {
        self.progress_message.as_deref()
    }
    pub fn render_argv(&self) -> Result<Vec<String>, ArgsExpansionError> {
        self.render_argv_with_context(&NoopArgsDirectoryContext)
    }
    pub fn render_argv_with_context(
        &self,
        context: &dyn ArgsDirectoryContext,
    ) -> Result<Vec<String>, ArgsExpansionError> {
        let mut budget = ExpansionBudget::default();
        let mut argv = self.invocation.render_prefix_with_budget(&mut budget)?;
        argv.extend(self.command_line.render_with_budget(context, &mut budget)?);
        Ok(argv)
    }

    pub(crate) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.invocation
            .publication_eq_with(&other.invocation, state)
            && self.outputs == other.outputs
            && self.unused_inputs_list == other.unused_inputs_list
            && self.environment == other.environment
            && self.execution_requirements == other.execution_requirements
            && self.mnemonic == other.mnemonic
            && self.progress_message == other.progress_message
            && self.resource_set == other.resource_set
            && self
                .command_line
                .publication_eq_with(&other.command_line, state)
            && self.inputs.publication_eq_with(&other.inputs, state)
            && self.tools.publication_eq_with(&other.tools, state)
    }
}

impl PartialEq for SpawnSpec {
    fn eq(&self, other: &Self) -> bool {
        self.publication_eq_with(other, &mut PublicationEqState::default())
    }
}

impl Eq for SpawnSpec {}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub enum SymlinkTarget {
    Artifact {
        input: AnalysisArtifact,
        require_executable: bool,
        use_exec_root_for_source: bool,
    },
    AbsolutePath {
        target: NormalizedAbsoluteBazelPath,
    },
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct SymlinkSpec {
    output: ActionOutput,
    target: SymlinkTarget,
    progress_message: Option<CompactString>,
}

impl SymlinkSpec {
    pub fn new(
        output: ActionOutput,
        target: SymlinkTarget,
        progress_message: Option<impl Into<CompactString>>,
    ) -> Self {
        Self {
            output,
            target,
            progress_message: progress_message.map(Into::into),
        }
    }
    pub fn output(&self) -> &ActionOutput {
        &self.output
    }
    pub fn target(&self) -> &SymlinkTarget {
        &self.target
    }
    pub fn progress_message(&self) -> Option<&str> {
        self.progress_message.as_deref()
    }
}

#[derive(Debug, Clone, Allocative)]
pub struct ArgsWriteSpec {
    output: ActionOutput,
    recipe: RetainedArgsRecipe,
    is_executable: bool,
    execution_requirements: CanonicalStringMap,
}

impl ArgsWriteSpec {
    pub fn new(output: ActionOutput, recipe: RetainedArgsRecipe, is_executable: bool) -> Self {
        Self {
            output,
            recipe,
            is_executable,
            execution_requirements: CanonicalStringMap::default(),
        }
    }

    pub fn output(&self) -> &ActionOutput {
        &self.output
    }

    pub fn recipe(&self) -> &RetainedArgsRecipe {
        &self.recipe
    }

    pub fn is_executable(&self) -> bool {
        self.is_executable
    }

    pub fn execution_requirements(&self) -> &CanonicalStringMap {
        &self.execution_requirements
    }

    pub fn render_content(&self) -> Result<String, ArgsExpansionError> {
        self.recipe.render_write_content()
    }
}

impl ArgsWriteSpec {
    pub(crate) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.output == other.output
            && self.is_executable == other.is_executable
            && self.execution_requirements == other.execution_requirements
            && self.recipe.publication_eq_with(&other.recipe, state)
    }
}
impl PartialEq for ArgsWriteSpec {
    fn eq(&self, other: &Self) -> bool {
        self.publication_eq_with(other, &mut PublicationEqState::default())
    }
}
impl Eq for ArgsWriteSpec {}

fn render_analysis_integer(value: &crate::analysis_value::AnalysisInteger) -> String {
    if value.magnitude().is_empty() {
        return "0".to_owned();
    }
    let mut digits = vec![0u8];
    for byte in value.magnitude() {
        let mut carry = u16::from(*byte);
        for digit in &mut digits {
            let next = u16::from(*digit) * 256 + carry;
            *digit = (next % 10) as u8;
            carry = next / 10;
        }
        while carry != 0 {
            digits.push((carry % 10) as u8);
            carry /= 10;
        }
    }
    let mut result = String::with_capacity(digits.len() + usize::from(value.is_negative()));
    if value.is_negative() {
        result.push('-');
    }
    result.extend(digits.iter().rev().map(|digit| char::from(b'0' + *digit)));
    result
}

// Bazel 9.2 GccParamFileEscaper.java:37-60. Only these ASCII characters
// require a prefix; Unicode whitespace and shell metacharacters stay literal.
fn push_gcc_escaped(result: &mut String, value: &str) {
    if value.is_empty() {
        result.push_str("''");
        return;
    }
    for character in value.chars() {
        if gcc_needs_escape(character) {
            result.push('\\');
        }
        result.push(character);
    }
}
fn gcc_escape(value: &str) -> String {
    let mut result = String::new();
    push_gcc_escaped(&mut result, value);
    result
}
fn push_shell_escaped(result: &mut String, value: &str) {
    if !shell_needs_quotes(value) {
        result.push_str(value);
        return;
    }
    result.push('\'');
    for ch in value.chars() {
        if ch == '\'' {
            result.push_str("'\\''");
        } else {
            result.push(ch);
        }
    }
    result.push('\'');
}
fn shell_escape(value: &str) -> String {
    let mut result = String::new();
    push_shell_escaped(&mut result, value);
    result
}

pub(super) fn apply_validated_format(format: &str, value: &str) -> String {
    apply_validated_format_sized(format, value, format.len() + value.len())
}
pub(super) fn apply_validated_format_sized(format: &str, value: &str, size: usize) -> String {
    let mut result = String::with_capacity(size);
    let mut chars = format.chars();
    while let Some(character) = chars.next() {
        if character != '%' {
            result.push(character);
            continue;
        }
        match chars.next() {
            Some('%') => result.push('%'),
            Some('s') => result.push_str(value),
            _ => unreachable!("format strings are validated before retention"),
        }
    }
    result
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub enum ParamFileFormat {
    Multiline,
    ShellQuoted,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct ParamFile {
    path: String,
    args: Vec<String>,
    format: ParamFileFormat,
}

impl ParamFile {
    pub fn new(path: impl Into<String>, args: Vec<String>, format: ParamFileFormat) -> Self {
        Self {
            path: path.into(),
            args,
            format,
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }

    pub fn format(&self) -> ParamFileFormat {
        self.format
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub enum ActionKind {
    Write {
        content: String,
        is_executable: bool,
    },
    WriteJson {
        content: String,
    },
    ExpandTemplate {
        template: ActionInput,
        substitutions: BTreeMap<String, String>,
    },
    Run,
    RunShell {
        command: String,
    },
    Symlink {
        target_path: String,
    },
    Spawn,
    ArtifactSymlink,
    AbsoluteSymlink,
    SolibSymlink,
    CppCompile,
    BuildInfoTransform,
    ArgsWrite,
    RepoMappingManifest,
    SourceSymlinkManifest,
    SymlinkTree,
    RunfilesTree,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
struct LegacyActionSpec {
    kind: ActionKind,
    mnemonic: String,
    argv: Vec<String>,
    env: BTreeMap<String, String>,
    execution_requirements: BTreeMap<String, String>,
    inputs: Vec<ActionInput>,
    tools: Vec<ActionInput>,
    outputs: Vec<ActionOutput>,
    param_files: Vec<ParamFile>,
    progress_message: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
enum ActionPayload {
    Legacy(LegacyActionSpec),
    Spawn(SpawnSpec),
    Symlink(SymlinkSpec),
    SolibSymlink(SolibSymlinkSpec),
    CppCompile(CppCompileSpec),
    BuildInfoTransform(BuildInfoTransformSpec),
    ArgsWrite(ArgsWriteSpec),
    RunfilesSupport(RunfilesSupportActionSpec),
}

#[derive(Debug, Clone, Allocative)]
pub struct ActionSpec {
    payload: ActionPayload,
    exec_properties: BTreeMap<String, String>,
    exec_group: Option<String>,
}

static SPAWN_KIND: ActionKind = ActionKind::Spawn;
static ARTIFACT_SYMLINK_KIND: ActionKind = ActionKind::ArtifactSymlink;
static ABSOLUTE_SYMLINK_KIND: ActionKind = ActionKind::AbsoluteSymlink;
static SOLIB_SYMLINK_KIND: ActionKind = ActionKind::SolibSymlink;
static CPP_COMPILE_KIND: ActionKind = ActionKind::CppCompile;
static BUILD_INFO_TRANSFORM_KIND: ActionKind = ActionKind::BuildInfoTransform;
static ARGS_WRITE_KIND: ActionKind = ActionKind::ArgsWrite;
static REPO_MAPPING_MANIFEST_KIND: ActionKind = ActionKind::RepoMappingManifest;
static SOURCE_SYMLINK_MANIFEST_KIND: ActionKind = ActionKind::SourceSymlinkManifest;
static SYMLINK_TREE_KIND: ActionKind = ActionKind::SymlinkTree;
static RUNFILES_TREE_KIND: ActionKind = ActionKind::RunfilesTree;
static EMPTY_STRING_MAP: std::sync::LazyLock<BTreeMap<String, String>> =
    std::sync::LazyLock::new(BTreeMap::new);

impl ActionSpec {
    pub fn new(kind: ActionKind, mnemonic: impl Into<String>, outputs: Vec<ActionOutput>) -> Self {
        Self {
            payload: ActionPayload::Legacy(LegacyActionSpec {
                kind,
                mnemonic: mnemonic.into(),
                argv: Vec::new(),
                env: BTreeMap::new(),
                execution_requirements: BTreeMap::new(),
                inputs: Vec::new(),
                tools: Vec::new(),
                outputs,
                param_files: Vec::new(),
                progress_message: None,
            }),
            exec_properties: BTreeMap::new(),
            exec_group: None,
        }
    }

    pub fn spawn(spec: SpawnSpec) -> Self {
        Self::typed(ActionPayload::Spawn(spec))
    }

    pub fn symlink(spec: SymlinkSpec) -> Self {
        Self::typed(ActionPayload::Symlink(spec))
    }

    pub fn solib_symlink(spec: SolibSymlinkSpec) -> Self {
        Self::typed(ActionPayload::SolibSymlink(spec))
    }

    pub fn cpp_compile(spec: CppCompileSpec) -> Self {
        Self::typed(ActionPayload::CppCompile(spec))
    }
    pub fn build_info_transform(spec: BuildInfoTransformSpec) -> Self {
        Self::typed(ActionPayload::BuildInfoTransform(spec))
    }

    pub fn args_write(spec: ArgsWriteSpec) -> Self {
        Self::typed(ActionPayload::ArgsWrite(spec))
    }

    pub fn runfiles_support(spec: RunfilesSupportActionSpec) -> Self {
        Self::typed(ActionPayload::RunfilesSupport(spec))
    }

    fn typed(payload: ActionPayload) -> Self {
        Self {
            payload,
            exec_properties: BTreeMap::new(),
            exec_group: None,
        }
    }

    pub fn kind(&self) -> &ActionKind {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.kind,
            ActionPayload::Spawn(_) => &SPAWN_KIND,
            ActionPayload::Symlink(spec) => match spec.target() {
                SymlinkTarget::Artifact { .. } => &ARTIFACT_SYMLINK_KIND,
                SymlinkTarget::AbsolutePath { .. } => &ABSOLUTE_SYMLINK_KIND,
            },
            ActionPayload::SolibSymlink(_) => &SOLIB_SYMLINK_KIND,
            ActionPayload::CppCompile(_) => &CPP_COMPILE_KIND,
            ActionPayload::BuildInfoTransform(_) => &BUILD_INFO_TRANSFORM_KIND,
            ActionPayload::ArgsWrite(_) => &ARGS_WRITE_KIND,
            ActionPayload::RunfilesSupport(spec) => match spec {
                RunfilesSupportActionSpec::RepoMappingManifest { .. } => {
                    &REPO_MAPPING_MANIFEST_KIND
                }
                RunfilesSupportActionSpec::SourceSymlinkManifest { .. } => {
                    &SOURCE_SYMLINK_MANIFEST_KIND
                }
                RunfilesSupportActionSpec::SymlinkTree { .. } => &SYMLINK_TREE_KIND,
                RunfilesSupportActionSpec::RunfilesTree { .. } => &RUNFILES_TREE_KIND,
            },
        }
    }

    pub fn mnemonic(&self) -> &str {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.mnemonic,
            ActionPayload::Spawn(spec) => spec.mnemonic(),
            ActionPayload::Symlink(_) => "Symlink",
            ActionPayload::SolibSymlink(_) => "SolibSymlink",
            ActionPayload::CppCompile(_) => "CppCompile",
            ActionPayload::BuildInfoTransform(_) => "TranslateBuildInfo",
            ActionPayload::ArgsWrite(_) => "FileWrite",
            ActionPayload::RunfilesSupport(spec) => spec.mnemonic(),
        }
    }

    pub fn argv(&self) -> &[String] {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.argv,
            _ => &[],
        }
    }

    pub fn env(&self) -> &BTreeMap<String, String> {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.env,
            _ => &EMPTY_STRING_MAP,
        }
    }

    pub fn execution_requirements(&self) -> &BTreeMap<String, String> {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.execution_requirements,
            _ => &EMPTY_STRING_MAP,
        }
    }

    pub fn inputs(&self) -> &[ActionInput] {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.inputs,
            _ => &[],
        }
    }

    pub fn tools(&self) -> &[ActionInput] {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.tools,
            _ => &[],
        }
    }

    pub fn outputs(&self) -> &[ActionOutput] {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.outputs,
            ActionPayload::Spawn(spec) => spec.outputs(),
            ActionPayload::Symlink(spec) => std::slice::from_ref(spec.output()),
            ActionPayload::SolibSymlink(spec) => std::slice::from_ref(spec.output()),
            ActionPayload::CppCompile(spec) => spec.outputs(),
            ActionPayload::BuildInfoTransform(spec) => std::slice::from_ref(spec.output()),
            ActionPayload::ArgsWrite(spec) => std::slice::from_ref(spec.output()),
            ActionPayload::RunfilesSupport(spec) => std::slice::from_ref(spec.output()),
        }
    }

    pub fn param_files(&self) -> &[ParamFile] {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.param_files,
            _ => &[],
        }
    }

    pub fn progress_message(&self) -> Option<&str> {
        match &self.payload {
            ActionPayload::Legacy(spec) => spec.progress_message.as_deref(),
            ActionPayload::Spawn(spec) => spec.progress_message(),
            ActionPayload::Symlink(spec) => spec.progress_message(),
            ActionPayload::SolibSymlink(_) => None,
            ActionPayload::CppCompile(_) => None,
            ActionPayload::BuildInfoTransform(spec) => Some(spec.progress_message()),
            ActionPayload::ArgsWrite(_) => None,
            ActionPayload::RunfilesSupport(_) => None,
        }
    }

    pub fn exec_properties(&self) -> &BTreeMap<String, String> {
        &self.exec_properties
    }

    pub fn exec_group(&self) -> Option<&str> {
        self.exec_group.as_deref()
    }

    pub fn with_argv(mut self, argv: Vec<String>) -> Self {
        self.legacy_mut().argv = argv;
        self
    }

    pub fn with_env(mut self, env: BTreeMap<String, String>) -> Self {
        self.legacy_mut().env = env;
        self
    }

    pub fn with_execution_requirements(
        mut self,
        execution_requirements: BTreeMap<String, String>,
    ) -> Self {
        self.legacy_mut().execution_requirements = execution_requirements;
        self
    }

    pub fn with_inputs(mut self, inputs: Vec<ActionInput>) -> Self {
        self.legacy_mut().inputs = inputs;
        self
    }

    pub fn with_tools(mut self, tools: Vec<ActionInput>) -> Self {
        self.legacy_mut().tools = tools;
        self
    }

    pub fn with_param_files(mut self, param_files: Vec<ParamFile>) -> Self {
        self.legacy_mut().param_files = param_files;
        self
    }

    pub fn with_progress_message(mut self, progress_message: impl Into<String>) -> Self {
        self.legacy_mut().progress_message = Some(progress_message.into());
        self
    }

    pub fn with_exec_properties(mut self, exec_properties: BTreeMap<String, String>) -> Self {
        self.exec_properties = exec_properties;
        self
    }

    pub fn with_exec_group(mut self, exec_group: impl Into<String>) -> Self {
        self.exec_group = Some(exec_group.into());
        self
    }

    pub fn spawn_spec(&self) -> Option<&SpawnSpec> {
        match &self.payload {
            ActionPayload::Spawn(spec) => Some(spec),
            _ => None,
        }
    }

    pub fn symlink_spec(&self) -> Option<&SymlinkSpec> {
        match &self.payload {
            ActionPayload::Symlink(spec) => Some(spec),
            _ => None,
        }
    }

    pub fn solib_symlink_spec(&self) -> Option<&SolibSymlinkSpec> {
        match &self.payload {
            ActionPayload::SolibSymlink(spec) => Some(spec),
            _ => None,
        }
    }

    pub fn args_write_spec(&self) -> Option<&ArgsWriteSpec> {
        match &self.payload {
            ActionPayload::ArgsWrite(spec) => Some(spec),
            _ => None,
        }
    }

    pub fn runfiles_support_spec(&self) -> Option<&RunfilesSupportActionSpec> {
        match &self.payload {
            ActionPayload::RunfilesSupport(spec) => Some(spec),
            _ => None,
        }
    }

    pub fn is_typed_payload(&self) -> bool {
        !matches!(&self.payload, ActionPayload::Legacy(_))
    }

    pub fn cpp_compile_spec(&self) -> Option<&CppCompileSpec> {
        match &self.payload {
            ActionPayload::CppCompile(spec) => Some(spec),
            _ => None,
        }
    }
    pub fn build_info_transform_spec(&self) -> Option<&BuildInfoTransformSpec> {
        match &self.payload {
            ActionPayload::BuildInfoTransform(spec) => Some(spec),
            _ => None,
        }
    }

    /// Legacy/Spawn preview. CppCompile has a separate fallible typed expansion.
    pub fn render_argv(&self) -> Result<Vec<String>, ArgsExpansionError> {
        if self.kind() == &ActionKind::BuildInfoTransform {
            return Err(ArgsExpansionError::Unsupported(
                "BuildInfoTransform execution is not admitted".into(),
            ));
        }
        if self.kind() == &ActionKind::CppCompile {
            return Err(ArgsExpansionError::Unsupported(
                "CppCompile generic argv preview is not admitted".into(),
            ));
        }
        match &self.payload {
            ActionPayload::Legacy(spec) => Ok(spec.argv.clone()),
            ActionPayload::Spawn(spec) => spec.render_argv(),
            ActionPayload::CppCompile(_) => Err(ArgsExpansionError::Unsupported(
                "CppCompile generic argv preview is not admitted".into(),
            )),
            _ => Ok(Vec::new()),
        }
    }

    fn legacy_mut(&mut self) -> &mut LegacyActionSpec {
        match &mut self.payload {
            ActionPayload::Legacy(spec) => spec,
            _ => panic!("legacy action builders cannot mutate typed action payloads"),
        }
    }
}

impl ActionSpec {
    pub(crate) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.exec_properties == other.exec_properties
            && self.exec_group == other.exec_group
            && match (&self.payload, &other.payload) {
                (ActionPayload::Spawn(l), ActionPayload::Spawn(r)) => {
                    l.publication_eq_with(r, state)
                }
                (ActionPayload::CppCompile(l), ActionPayload::CppCompile(r)) => {
                    l.publication_eq_with(r, state)
                }
                (ActionPayload::ArgsWrite(l), ActionPayload::ArgsWrite(r)) => {
                    l.publication_eq_with(r, state)
                }
                (ActionPayload::RunfilesSupport(l), ActionPayload::RunfilesSupport(r)) => {
                    l.publication_eq_with(r, state)
                }
                (ActionPayload::Legacy(l), ActionPayload::Legacy(r)) => l == r,
                (ActionPayload::Symlink(l), ActionPayload::Symlink(r)) => l == r,
                (ActionPayload::SolibSymlink(l), ActionPayload::SolibSymlink(r)) => l == r,
                (ActionPayload::BuildInfoTransform(l), ActionPayload::BuildInfoTransform(r)) => {
                    l == r
                }
                _ => false,
            }
    }
}
impl PartialEq for ActionSpec {
    fn eq(&self, other: &Self) -> bool {
        self.publication_eq_with(other, &mut PublicationEqState::default())
    }
}
impl Eq for ActionSpec {}
