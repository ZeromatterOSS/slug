/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::cell::Ref;
use std::cell::RefCell;
use std::cell::RefMut;
use std::convert::Infallible;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::marker::PhantomData;
use std::sync::Arc;

use allocative::Allocative;
use display_container::display_pair;
use display_container::fmt_container;
use display_container::iter_display_chain;
use dupe::Dupe;
use either::Either;
use gazebo::prelude::*;
use indexmap::IndexSet;
use slug_artifact::artifact::artifact_type::Artifact;
use slug_artifact::artifact::artifact_type::OutputArtifact;
use slug_error::BuckErrorContext;
use slug_fs::paths::RelativePathBuf;
use serde::Serialize;
use serde::Serializer;
use starlark::any::ProvidesStaticType;
use starlark::coerce::Coerce;
use starlark::coerce::coerce;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::starlark_complex_value;
use starlark::typing::Ty;
use starlark::values::AllocStaticSimple;
use starlark::values::AllocValue;
use starlark::values::Demand;
use starlark::values::Freeze;
use starlark::values::FreezeResult;
use starlark::values::Freezer;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::StringValue;
use starlark::values::ThinBoxSliceFrozenValue;
use starlark::values::Trace;
use starlark::values::UnpackValue;
use starlark::values::Value;
use starlark::values::ValueLifetimeless;
use starlark::values::ValueLike;
use starlark::values::ValueOf;
use starlark::values::list::ListRef;
use starlark::values::list::UnpackList;
use starlark::values::starlark_value;
use starlark::values::starlark_value_as_type::StarlarkValueAsType;
use starlark::values::tuple::TupleRef;
use starlark::values::tuple::UnpackTuple;
use starlark::values::type_repr::StarlarkTypeRepr;
use static_assertions::assert_eq_size;

use crate::artifact_groups::ArtifactGroup;
use crate::artifact_groups::DepsetArtifactGroup;
use crate::interpreter::rule_defs::artifact::associated::AssociatedArtifacts;
use crate::interpreter::rule_defs::artifact::starlark_declared_artifact::StarlarkDeclaredArtifact;
use crate::interpreter::rule_defs::artifact::starlark_output_artifact::StarlarkOutputArtifact;
use crate::interpreter::rule_defs::artifact_tagging::ArtifactTag;
use crate::interpreter::rule_defs::cc_common::cc_frozen_list_items;
use crate::interpreter::rule_defs::cmd_args::ArtifactPathMapper;
use crate::interpreter::rule_defs::cmd_args::command_line_arg_like_type::command_line_arg_like_impl;
use crate::interpreter::rule_defs::cmd_args::options::CommandLineOptions;
use crate::interpreter::rule_defs::cmd_args::options::CommandLineOptionsRef;
use crate::interpreter::rule_defs::cmd_args::options::CommandLineOptionsTrait;
use crate::interpreter::rule_defs::cmd_args::options::FrozenCommandLineOptions;
use crate::interpreter::rule_defs::cmd_args::options::QuoteStyle;
use crate::interpreter::rule_defs::cmd_args::options::RelativeOrigin;
use crate::interpreter::rule_defs::cmd_args::regex::CmdArgsRegex;
use crate::interpreter::rule_defs::cmd_args::traits::CommandLineArgLike;
use crate::interpreter::rule_defs::cmd_args::traits::CommandLineArtifactVisitor;
use crate::interpreter::rule_defs::cmd_args::traits::CommandLineBuilder;
use crate::interpreter::rule_defs::cmd_args::traits::CommandLineContext;
use crate::interpreter::rule_defs::cmd_args::traits::SimpleCommandLineArtifactVisitor;
use crate::interpreter::rule_defs::cmd_args::traits::WriteToFileMacroVisitor;
use crate::interpreter::rule_defs::cmd_args::value::CommandLineArg;
use crate::interpreter::rule_defs::cmd_args::value::FrozenCommandLineArg;
use crate::interpreter::rule_defs::cmd_args::value_as::ValueAsCommandLineLike;

/// Format for param file entries (used during Starlark evaluation).
#[derive(Debug, Clone, Copy, Dupe, Default, Trace, Freeze, Allocative)]
pub enum ParamFileFormat {
    #[default]
    Multiline,
    FlagPerLine,
    Shell,
}

/// Param file configuration. Stored as plain Rust types (not Starlark values)
/// since param_file_arg is always a string literal set at analysis time.
#[derive(Debug, Clone, Trace, Allocative)]
pub(crate) struct ParamFileData {
    pub(crate) param_file_arg: String,
    pub(crate) use_always: bool,
    pub(crate) format: ParamFileFormat,
}

/// Frozen param file configuration (same as ParamFileData since it contains no Starlark values).
#[derive(Debug, Allocative, Clone)]
pub struct FrozenParamFileData {
    pub param_file_arg: String,
    pub use_always: bool,
    pub format: ParamFileFormat,
}

impl Freeze for ParamFileData {
    type Frozen = FrozenParamFileData;
    fn freeze(self, _freezer: &Freezer) -> FreezeResult<FrozenParamFileData> {
        Ok(FrozenParamFileData {
            param_file_arg: self.param_file_arg,
            use_always: self.use_always,
            format: self.format,
        })
    }
}

#[derive(Debug, slug_error::Error)]
pub enum CommandLineError {
    #[error("Artifact(s) {0:?} cannot be used with ignore_artifacts as they are content-based")]
    #[slug(input)]
    ContentBasedIgnoreArtifacts(IndexSet<String>),
    #[error("Unknown param file format `{0}`. Expected one of: multiline, flag_per_line, shell")]
    #[slug(input)]
    UnknownParamFileFormat(String),
}

#[derive(Debug, Clone, Trace, Freeze, Allocative)]
enum DepsetCommandLineArgMode {
    AddAll,
    AddJoined { join_with: String },
}

#[derive(
    Debug,
    Clone,
    Trace,
    Coerce,
    Freeze,
    ProvidesStaticType,
    NoSerialize,
    Allocative
)]
#[repr(C)]
struct DepsetCommandLineArgGen<V: ValueLifetimeless> {
    depset: V,
    mode: DepsetCommandLineArgMode,
}

starlark_complex_value!(DepsetCommandLineArg);

impl<'v, V: ValueLike<'v>> Display for DepsetCommandLineArgGen<V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.mode {
            DepsetCommandLineArgMode::AddAll => {
                write!(f, "<depset command line {}>", self.depset.to_value())
            }
            DepsetCommandLineArgMode::AddJoined { join_with } => write!(
                f,
                "<depset command line {} joined with {:?}>",
                self.depset.to_value(),
                join_with
            ),
        }
    }
}

#[starlark_value(type = "DepsetCommandLineArg")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for DepsetCommandLineArgGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn CommandLineArgLike>(self);
    }
}

impl<'v, V: ValueLike<'v>> CommandLineArgLike<'v> for DepsetCommandLineArgGen<V> {
    fn register_me(&self) {}

    fn add_to_command_line(
        &self,
        cli: &mut dyn CommandLineBuilder,
        context: &mut dyn CommandLineContext,
        artifact_path_mapping: &dyn ArtifactPathMapper,
    ) -> slug_error::Result<()> {
        let values = crate::interpreter::rule_defs::depset::depset_to_list_without_heap(
            self.depset.to_value(),
        )?;
        match &self.mode {
            DepsetCommandLineArgMode::AddAll => {
                for value in values {
                    ValueAsCommandLineLike::unpack_value_err(value)?
                        .0
                        .add_to_command_line(cli, context, artifact_path_mapping)?;
                }
            }
            DepsetCommandLineArgMode::AddJoined { join_with } => {
                let mut rendered = Vec::new();
                for value in values {
                    ValueAsCommandLineLike::unpack_value_err(value)?
                        .0
                        .add_to_command_line(&mut rendered, context, artifact_path_mapping)?;
                }
                cli.push_arg(rendered.join(join_with));
            }
        }
        Ok(())
    }

    fn visit_artifacts(
        &self,
        visitor: &mut dyn CommandLineArtifactVisitor<'v>,
    ) -> slug_error::Result<()> {
        let depset_value = self.depset.to_value();
        match crate::interpreter::rule_defs::depset::depset_element_type_name(depset_value)?
            .as_deref()
        {
            None | Some("string" | "str") => return Ok(()),
            Some("File") => {
                if let Some(frozen) = depset_value.unpack_frozen() {
                    let has_content_based_path =
                        crate::interpreter::rule_defs::depset::depset_artifact_group_has_content_based_path(
                            depset_value,
                        )?;
                    visitor.visit_input(
                        ArtifactGroup::Depset(Arc::new(DepsetArtifactGroup::new(
                            frozen,
                            has_content_based_path,
                        ))),
                        vec![],
                    );
                    return Ok(());
                }
            }
            _ => {}
        }

        for value in
            crate::interpreter::rule_defs::depset::depset_to_list_without_heap(depset_value)?
        {
            ValueAsCommandLineLike::unpack_value_err(value)?
                .0
                .visit_artifacts(visitor)?;
        }
        Ok(())
    }

    fn contains_arg_attr(&self) -> bool {
        false
    }

    fn visit_write_to_file_macros(
        &self,
        visitor: &mut dyn WriteToFileMacroVisitor,
        artifact_path_mapping: &dyn ArtifactPathMapper,
    ) -> slug_error::Result<()> {
        for value in crate::interpreter::rule_defs::depset::depset_to_list_without_heap(
            self.depset.to_value(),
        )? {
            ValueAsCommandLineLike::unpack_value_err(value)?
                .0
                .visit_write_to_file_macros(visitor, artifact_path_mapping)?;
        }
        Ok(())
    }
}

fn depset_command_line_arg<'v>(
    heap: Heap<'v>,
    depset: Value<'v>,
    mode: DepsetCommandLineArgMode,
) -> Value<'v> {
    heap.alloc_complex(DepsetCommandLineArgGen { depset, mode })
}

fn can_defer_depset_command_line(value: Value) -> starlark::Result<bool> {
    if !crate::interpreter::rule_defs::depset::is_depset_value(value) {
        return Ok(false);
    }
    Ok(matches!(
        crate::interpreter::rule_defs::depset::depset_element_type_name(value)?.as_deref(),
        None | Some("string" | "str" | "File" | "OutputArtifact")
    ))
}

fn append_map_each_result<'v>(result: &mut Vec<Value<'v>>, mapped: Value<'v>) {
    if mapped.is_none() {
        return;
    }
    if let Some(items) = cc_frozen_list_items(mapped) {
        result.extend(items);
        return;
    }
    if let Some(list) = ListRef::from_value(mapped) {
        result.extend(list.iter());
    } else if let Some(tuple) = TupleRef::from_value(mapped) {
        result.extend(tuple.iter());
    } else {
        result.push(mapped);
    }
}

fn eval_map_each<'v>(
    eval: &mut Evaluator<'v, '_, '_>,
    map_each: Value<'v>,
    item: Value<'v>,
) -> starlark::Result<Value<'v>> {
    match eval.eval_function(map_each, &[item], &[]) {
        Ok(mapped) => Ok(mapped),
        Err(err)
            if err
                .to_string()
                .contains("Missing parameter `tree_expander`") =>
        {
            let tree_expander = eval.heap().alloc(ArgsTreeExpander).to_value();
            eval.eval_function(map_each, &[item, tree_expander], &[])
        }
        Err(err) => Err(err),
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct ArgsTreeExpander;

impl Display for ArgsTreeExpander {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "<tree_expander>")
    }
}

starlark_simple_value!(ArgsTreeExpander);

#[starlark_value(type = "tree_expander")]
impl<'v> StarlarkValue<'v> for ArgsTreeExpander {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(args_tree_expander_methods)
    }
}

#[starlark_module]
fn args_tree_expander_methods(builder: &mut MethodsBuilder) {
    /// Expands a tree artifact to its children.
    ///
    /// Slug does not have analysis-time tree contents, so this conservative shim
    /// returns an empty expansion. Non-tree artifacts are never expanded by
    /// Bazel's module-map callback path.
    fn expand<'v>(
        this: &ArgsTreeExpander,
        artifact: Value<'v>,
    ) -> starlark::Result<Vec<Value<'v>>> {
        let _ = (this, artifact);
        Ok(Vec::new())
    }
}

/// Fields of `cmd_args`. Abstract mutable and frozen versions.
trait Fields<'v> {
    fn items(&self) -> &[CommandLineArg<'v>];
    fn hidden(&self) -> &[CommandLineArg<'v>];
    fn options(&self) -> Option<&dyn CommandLineOptionsTrait<'v>>;
}

/// Wrapper because we cannot implement traits for traits.
struct FieldsRef<'v, F: Fields<'v>>(F, PhantomData<Value<'v>>);

/// There's no good reason for a user to write `cmd_args` as JSON in analysis or BXL.
///
/// This implementation exists for operations such as:
///
/// ```ignore
/// slug cquery :slug --providers
/// ```
///
/// which must not fail if a provider contains `cmd_args` (D34887765).
impl<'v, F: Fields<'v>> Serialize for FieldsRef<'v, F> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        /// Make sure mutable and frozen `cmd_args` are serialized identically
        /// by routing through this struct.
        #[derive(Serialize)]
        struct Mirror<'v, 'a> {
            items: &'a [CommandLineArg<'v>],
            hidden: &'a [CommandLineArg<'v>],
            options: Option<CommandLineOptionsRef<'v, 'a>>,
        }

        Mirror {
            items: self.0.items(),
            hidden: self.0.hidden(),
            options: self.0.options().map(|x| x.to_command_line_options()),
        }
        .serialize(serializer)
    }
}

impl<'v, F: Fields<'v>> Display for FieldsRef<'v, F> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        fmt_container(
            f,
            "cmd_args(",
            ")",
            iter_display_chain(
                self.0.items(),
                iter_display_chain(
                    Some(self.0.hidden())
                        .filter(|x| !x.is_empty())
                        .map(|hidden| {
                            struct Wrapper<'a, 'v>(&'a [CommandLineArg<'v>]);
                            impl<'a, 'v> Display for Wrapper<'a, 'v> {
                                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                                    fmt_container(f, "[", "]", self.0.iter())
                                }
                            }
                            display_pair("hidden", "=", Wrapper(hidden))
                        }),
                    self.0
                        .options()
                        .map(|o| o.to_command_line_options())
                        .unwrap_or_default()
                        .iter_fields_display()
                        .map(|(k, v)| display_pair(k, "=", v)),
                ),
            ),
        )
    }
}

impl<'v, F: Fields<'v>> FieldsRef<'v, F> {
    fn copy(&self) -> StarlarkCmdArgs<'v> {
        StarlarkCmdArgs(RefCell::new(StarlarkCommandLineData {
            items: self.0.items().to_vec(),
            hidden: self.0.hidden().to_vec(),
            options: self
                .0
                .options()
                .map(|x| Box::new(x.to_command_line_options().to_owned())),
            param_file: None, // param_file is not copied (it's a top-level property)
        }))
    }

    fn ignore_artifacts(&self) -> bool {
        self.0
            .options()
            .map(|o| o.ignore_artifacts())
            .unwrap_or_default()
    }

    fn is_concat(&self) -> bool {
        if let Some(x) = &self.0.options() {
            x.delimiter().is_some()
        } else {
            false
        }
    }

    fn relative_to_path<C>(
        &self,
        ctx: &C,
        artifact_path_mapping: &dyn ArtifactPathMapper,
    ) -> slug_error::Result<Option<RelativePathBuf>>
    where
        C: CommandLineContext + ?Sized,
    {
        match &self.0.options() {
            None => Ok(None),
            Some(options) => options
                .to_command_line_options()
                .relative_to_path(ctx, artifact_path_mapping),
        }
    }
}

impl<'v, F: Fields<'v>> CommandLineArgLike<'v> for FieldsRef<'v, F> {
    fn register_me(&self) {
        command_line_arg_like_impl!(StarlarkCmdArgs::starlark_type_repr());
    }

    fn add_to_command_line(
        &self,
        cli: &mut dyn CommandLineBuilder,
        context: &mut dyn CommandLineContext,
        artifact_path_mapping: &dyn ArtifactPathMapper,
    ) -> slug_error::Result<()> {
        match self.0.options() {
            None => {
                for item in self.0.items() {
                    item.as_command_line_arg().add_to_command_line(
                        cli,
                        context,
                        artifact_path_mapping,
                    )?;
                }
                Ok(())
            }
            Some(options) => options.to_command_line_options().wrap_builder(
                cli,
                context,
                |cli, context| {
                    for item in self.0.items() {
                        item.as_command_line_arg().add_to_command_line(
                            cli,
                            context,
                            artifact_path_mapping,
                        )?;
                    }
                    Ok(())
                },
                artifact_path_mapping,
            ),
        }
    }

    fn visit_artifacts(
        &self,
        visitor: &mut dyn CommandLineArtifactVisitor<'v>,
    ) -> slug_error::Result<()> {
        if !self.ignore_artifacts() {
            fn visit_items<'a>(
                visitor: &mut dyn CommandLineArtifactVisitor<'a>,
                items: &[CommandLineArg<'a>],
            ) -> slug_error::Result<()> {
                for item in items {
                    visitor.push_frame()?;
                    item.as_command_line_arg().visit_artifacts(visitor)?;
                    visitor.pop_frame();
                }

                Ok(())
            }

            visit_items(visitor, self.0.items())?;
            if !visitor.skip_hidden() {
                visit_items(visitor, self.0.hidden())?;
            }
        } else {
            struct IgnoredArtifactsVisitor {
                content_based_artifacts: IndexSet<String>,
            }

            impl IgnoredArtifactsVisitor {
                fn new() -> Self {
                    Self {
                        content_based_artifacts: IndexSet::new(),
                    }
                }
            }

            impl<'v> CommandLineArtifactVisitor<'v> for IgnoredArtifactsVisitor {
                fn visit_input(&mut self, input: ArtifactGroup, _tags: Vec<&ArtifactTag>) {
                    if input.uses_content_based_path() {
                        self.content_based_artifacts.insert(input.to_string());
                    }
                }

                fn visit_declared_artifact(
                    &mut self,
                    declared_artifact: slug_artifact::artifact::artifact_type::DeclaredArtifact,
                    _tags: Vec<&ArtifactTag>,
                ) -> slug_error::Result<()> {
                    if declared_artifact.has_content_based_path() {
                        self.content_based_artifacts
                            .insert(declared_artifact.to_string());
                    }

                    Ok(())
                }

                fn visit_declared_output(
                    &mut self,
                    _artifact: OutputArtifact<'v>,
                    _tags: Vec<&ArtifactTag>,
                ) {
                }

                fn visit_frozen_output(&mut self, _artifact: Artifact, _tags: Vec<&ArtifactTag>) {}
            }
            let mut ignored_artifacts_visitor = IgnoredArtifactsVisitor::new();
            for item in self.0.items().iter().chain(self.0.hidden().iter()) {
                ignored_artifacts_visitor.push_frame()?;
                item.as_command_line_arg()
                    .visit_artifacts(&mut ignored_artifacts_visitor)?;
                ignored_artifacts_visitor.pop_frame();
            }
            if !ignored_artifacts_visitor.content_based_artifacts.is_empty() {
                return Err(CommandLineError::ContentBasedIgnoreArtifacts(
                    ignored_artifacts_visitor.content_based_artifacts,
                )
                .into());
            }
        }
        Ok(())
    }

    fn contains_arg_attr(&self) -> bool {
        self.0
            .items()
            .iter()
            .any(|x| x.as_command_line_arg().contains_arg_attr())
            || self
                .0
                .hidden()
                .iter()
                .any(|x| x.as_command_line_arg().contains_arg_attr())
    }

    fn visit_write_to_file_macros(
        &self,
        visitor: &mut dyn WriteToFileMacroVisitor,
        artifact_path_mapping: &dyn ArtifactPathMapper,
    ) -> slug_error::Result<()> {
        visitor.set_current_relative_to_path(&|ctx| {
            self.relative_to_path(ctx, artifact_path_mapping)
        })?;

        for item in self.0.items() {
            item.as_command_line_arg()
                .visit_write_to_file_macros(visitor, artifact_path_mapping)?;
        }
        for item in self.0.hidden() {
            item.as_command_line_arg()
                .visit_write_to_file_macros(visitor, artifact_path_mapping)?;
        }
        Ok(())
    }
}

/// Starlark object returned by `cmd_args()`
/// A container for all of the args and nested command lines that a users adds to `ctx.args()`
///
/// This allows more efficient iterative argument building, including O(1) insertion of
/// `CommandLine` / `FrozenCommandLine` args.
///
/// When frozen, a `FrozenCommandLine` is created.
///
/// `items` contains strings, artifacts, command line args (frozen and not), but does not
///         contain any builders.
#[derive(Debug, Default, Clone, Trace, ProvidesStaticType, Allocative)]
#[repr(C)]
pub struct StarlarkCommandLineData<'v> {
    items: Vec<CommandLineArg<'v>>,
    hidden: Vec<CommandLineArg<'v>>,
    options: Option<Box<CommandLineOptions<'v>>>,
    param_file: Option<Box<ParamFileData>>,
}

#[derive(Debug, Default, Clone, Trace, ProvidesStaticType, Allocative)]
pub struct StarlarkCmdArgs<'v>(RefCell<StarlarkCommandLineData<'v>>);

impl<'v> Serialize for StarlarkCmdArgs<'v> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        FieldsRef(self.0.borrow(), PhantomData).serialize(serializer)
    }
}

#[derive(Debug, ProvidesStaticType, Allocative)]
pub struct FrozenStarlarkCmdArgs {
    // Elements are `FrozenCommandLineArg`s
    items: ThinBoxSliceFrozenValue<'static>,
    hidden: ThinBoxSliceFrozenValue<'static>,
    options: FrozenCommandLineOptions,
    param_file: Option<Box<FrozenParamFileData>>,
}

impl Serialize for FrozenStarlarkCmdArgs {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        FieldsRef(self, PhantomData).serialize(serializer)
    }
}

impl<'a, 'v> Fields<'v> for Ref<'a, StarlarkCommandLineData<'v>> {
    fn items(&self) -> &[CommandLineArg<'v>] {
        &self.items
    }

    fn hidden(&self) -> &[CommandLineArg<'v>] {
        &self.hidden
    }

    fn options(&self) -> Option<&dyn CommandLineOptionsTrait<'v>> {
        match &self.options {
            None => None,
            Some(x) => Some(&**x),
        }
    }
}

impl<'v> Fields<'v> for FrozenStarlarkCmdArgs {
    fn items(&self) -> &[CommandLineArg<'v>] {
        coerce(FrozenCommandLineArg::slice_from_frozen_value_unchecked(
            &self.items,
        ))
    }

    fn hidden(&self) -> &[CommandLineArg<'v>] {
        coerce(FrozenCommandLineArg::slice_from_frozen_value_unchecked(
            &self.hidden,
        ))
    }

    fn options(&self) -> Option<&dyn CommandLineOptionsTrait<'v>> {
        if self.options.is_empty() {
            None
        } else {
            Some(&self.options)
        }
    }
}

impl<'a, 'v, F: Fields<'v>> Fields<'v> for &'a F {
    fn items(&self) -> &[CommandLineArg<'v>] {
        (*self).items()
    }

    fn hidden(&self) -> &[CommandLineArg<'v>] {
        (*self).hidden()
    }

    fn options(&self) -> Option<&dyn CommandLineOptionsTrait<'v>> {
        (*self).options()
    }
}

impl<'v, A: Fields<'v>, B: Fields<'v>> Fields<'v> for Either<A, B> {
    fn items(&self) -> &[CommandLineArg<'v>] {
        match self {
            Either::Left(x) => x.items(),
            Either::Right(x) => x.items(),
        }
    }

    fn hidden(&self) -> &[CommandLineArg<'v>] {
        match self {
            Either::Left(x) => x.hidden(),
            Either::Right(x) => x.hidden(),
        }
    }

    fn options(&self) -> Option<&dyn CommandLineOptionsTrait<'v>> {
        match self {
            Either::Left(x) => x.options(),
            Either::Right(x) => x.options(),
        }
    }
}

// These types show up a lot in the frozen heaps, so make sure they don't regress
assert_eq_size!(StarlarkCmdArgs<'static>, [usize; 9]);
assert_eq_size!(FrozenStarlarkCmdArgs, [usize; 4]);
assert_eq_size!(CommandLineOptions<'static>, [usize; 10]);

impl<'v> Display for StarlarkCmdArgs<'v> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.0.try_borrow() {
            Ok(x) => Display::fmt(&FieldsRef(x, PhantomData), f),
            Err(_) => write!(f, "<cmd_args borrowed>"),
        }
    }
}

impl Display for FrozenStarlarkCmdArgs {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&FieldsRef(self, PhantomData), f)
    }
}

impl<'v> StarlarkCommandLineData<'v> {
    fn options_mut(&mut self) -> &mut CommandLineOptions<'v> {
        if self.options.is_none() {
            self.options = Some(Box::default());
        }
        self.options.as_mut().unwrap()
    }

    fn param_file_mut(&mut self, arg: &str) -> &mut ParamFileData {
        if self.param_file.is_none() {
            self.param_file = Some(Box::new(ParamFileData {
                param_file_arg: arg.to_owned(),
                use_always: false,
                format: ParamFileFormat::default(),
            }));
        }
        self.param_file.as_mut().unwrap()
    }
}

impl<'v> StarlarkCmdArgs<'v> {
    pub(crate) fn is_concat(&self) -> bool {
        FieldsRef(self.0.borrow(), PhantomData).is_concat()
    }
}

impl FrozenStarlarkCmdArgs {
    pub(crate) fn is_concat(&self) -> bool {
        FieldsRef(self, PhantomData).is_concat()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn param_file(&self) -> Option<&FrozenParamFileData> {
        self.param_file.as_deref()
    }

    /// Top-level items (each may be a string, artifact, or nested cmd_args).
    /// Used by callers that need to render items individually to track per-item
    /// boundaries (e.g. per-Args paramfile materialization).
    pub fn top_level_items(&self) -> &[FrozenCommandLineArg] {
        FrozenCommandLineArg::slice_from_frozen_value_unchecked(&self.items)
    }

    /// Returns true if this cmd_args has any options (delimiter, prepend, etc.)
    /// that affect how items concatenate at the top level.
    pub fn has_options(&self) -> bool {
        !self.options.is_empty()
    }

    /// Search for param_file config in this object or nested cmd_args items.
    /// In Bazel, use_param_file can be on an individual Args object within a list.
    /// Buck2 only checks the top-level. This method searches nested items too.
    pub fn param_file_or_nested(&self) -> Option<&FrozenParamFileData> {
        if let Some(pf) = self.param_file.as_deref() {
            return Some(pf);
        }
        // Check nested items (frozen values that might be cmd_args with param_file)
        for item in self.items.iter() {
            if let Some(nested) = item.downcast_ref::<FrozenStarlarkCmdArgs>() {
                if let Some(pf) = nested.param_file.as_deref() {
                    return Some(pf);
                }
            }
        }
        None
    }
}

impl<'v> StarlarkCmdArgs<'v> {
    pub fn is_empty(&self) -> bool {
        self.0.borrow().items.is_empty()
    }
}

#[starlark_value(type = "cmd_args")]
impl<'v> StarlarkValue<'v> for StarlarkCmdArgs<'v> {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(cmd_args_methods)
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn CommandLineArgLike>(self);
    }

    fn try_freeze_directly(&self, _freezer: &Freezer<'_>) -> Option<FreezeResult<FrozenValue>> {
        let StarlarkCommandLineData {
            items,
            hidden,
            options,
            param_file,
        } = &*self.0.borrow();
        if items.is_empty() && hidden.is_empty() && options.is_none() && param_file.is_none() {
            static EMPTY: AllocStaticSimple<FrozenStarlarkCmdArgs> =
                AllocStaticSimple::alloc(FrozenStarlarkCmdArgs {
                    items: ThinBoxSliceFrozenValue::empty(),
                    hidden: ThinBoxSliceFrozenValue::empty(),
                    options: FrozenCommandLineOptions::empty(),
                    param_file: None,
                });
            Some(Ok(EMPTY.unpack().to_frozen_value()))
        } else {
            None
        }
    }
}

#[starlark_value(type = "cmd_args")]
impl<'v> StarlarkValue<'v> for FrozenStarlarkCmdArgs {
    type Canonical = StarlarkCmdArgs<'v>;

    fn get_methods() -> Option<&'static Methods> {
        // We return the same methods for frozen command lines, even though some of them fail,
        // so the methods remain consistent during freezing
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(cmd_args_methods)
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn CommandLineArgLike<'v>>(self);
    }
}

impl<'v> AllocValue<'v> for StarlarkCmdArgs<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex(self)
    }
}

impl<'v> CommandLineArgLike<'v> for StarlarkCmdArgs<'v> {
    fn register_me(&self) {
        command_line_arg_like_impl!(StarlarkCmdArgs::starlark_type_repr());
    }

    fn add_to_command_line(
        &self,
        cli: &mut dyn CommandLineBuilder,
        context: &mut dyn CommandLineContext,
        artifact_path_mapping: &dyn ArtifactPathMapper,
    ) -> slug_error::Result<()> {
        FieldsRef(self.0.borrow(), PhantomData).add_to_command_line(
            cli,
            context,
            artifact_path_mapping,
        )
    }

    fn visit_artifacts(
        &self,
        visitor: &mut dyn CommandLineArtifactVisitor<'v>,
    ) -> slug_error::Result<()> {
        FieldsRef(self.0.borrow(), PhantomData).visit_artifacts(visitor)
    }

    fn contains_arg_attr(&self) -> bool {
        FieldsRef(self.0.borrow(), PhantomData).contains_arg_attr()
    }

    fn visit_write_to_file_macros(
        &self,
        visitor: &mut dyn WriteToFileMacroVisitor,
        artifact_path_mapping: &dyn ArtifactPathMapper,
    ) -> slug_error::Result<()> {
        FieldsRef(self.0.borrow(), PhantomData)
            .visit_write_to_file_macros(visitor, artifact_path_mapping)
    }
}

impl<'v> CommandLineArgLike<'v> for FrozenStarlarkCmdArgs {
    fn register_me(&self) {
        command_line_arg_like_impl!(FrozenStarlarkCmdArgs::starlark_type_repr());
    }

    fn add_to_command_line(
        &self,
        cli: &mut dyn CommandLineBuilder,
        context: &mut dyn CommandLineContext,
        artifact_path_mapping: &dyn ArtifactPathMapper,
    ) -> slug_error::Result<()> {
        FieldsRef(self, PhantomData).add_to_command_line(cli, context, artifact_path_mapping)
    }

    fn visit_artifacts(
        &self,
        visitor: &mut dyn CommandLineArtifactVisitor<'v>,
    ) -> slug_error::Result<()> {
        FieldsRef(self, PhantomData).visit_artifacts(visitor)
    }

    fn contains_arg_attr(&self) -> bool {
        FieldsRef(self, PhantomData).contains_arg_attr()
    }

    fn visit_write_to_file_macros(
        &self,
        visitor: &mut dyn WriteToFileMacroVisitor,
        artifact_path_mapping: &dyn ArtifactPathMapper,
    ) -> slug_error::Result<()> {
        FieldsRef(self, PhantomData).visit_write_to_file_macros(visitor, artifact_path_mapping)
    }
}

impl<'v> Freeze for StarlarkCmdArgs<'v> {
    type Frozen = FrozenStarlarkCmdArgs;
    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        let StarlarkCommandLineData {
            items,
            hidden,
            options,
            param_file,
        } = self.0.into_inner();

        let items = ThinBoxSliceFrozenValue::from_iter(
            items
                .freeze(freezer)?
                .into_iter()
                .map(|a| a.to_frozen_value()),
        );
        let hidden = ThinBoxSliceFrozenValue::from_iter(
            hidden
                .freeze(freezer)?
                .into_iter()
                .map(|a| a.to_frozen_value()),
        );
        let options = options
            .try_map(|options| (*options).freeze(freezer))?
            .unwrap_or_default();
        let param_file = param_file.map(|pf| {
            Box::new(FrozenParamFileData {
                param_file_arg: pf.param_file_arg,
                use_always: pf.use_always,
                format: pf.format,
            })
        });

        Ok(FrozenStarlarkCmdArgs {
            items,
            hidden,
            options,
            param_file,
        })
    }
}

impl<'v> StarlarkCmdArgs<'v> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub fn try_from_value(value: Value<'v>) -> slug_error::Result<Self> {
        Self::try_from_value_typed(StarlarkCommandLineValueUnpack::unpack_value_err(value)?)
    }

    pub fn try_from_value_typed(
        value: StarlarkCommandLineValueUnpack<'v>,
    ) -> slug_error::Result<Self> {
        let mut builder = Self::new();
        builder.0.get_mut().add_value_typed(value)?;
        Ok(builder)
    }
}

#[derive(UnpackValue, StarlarkTypeRepr)]
pub enum StarlarkCommandLineValueUnpack<'v> {
    // This should be `list[Self]`, but we cannot express it.
    List(&'v ListRef<'v>),
    CommandLineArg(CommandLineArg<'v>),
}

impl<'v> StarlarkCommandLineData<'v> {
    fn add_value(&mut self, value: Value<'v>) -> slug_error::Result<()> {
        self.add_value_typed(StarlarkCommandLineValueUnpack::unpack_value_err(value)?)
    }

    fn add_value_typed(
        &mut self,
        value: StarlarkCommandLineValueUnpack<'v>,
    ) -> slug_error::Result<()> {
        match value {
            StarlarkCommandLineValueUnpack::List(values) => self.add_values(values.content())?,
            StarlarkCommandLineValueUnpack::CommandLineArg(value) => self.items.push(value),
        }
        Ok(())
    }

    /// Check the types of a list of values, and modify `data` accordingly
    ///
    /// The values must be one of: CommandLineArgLike or a list thereof.
    fn add_values(&mut self, values: &[Value<'v>]) -> slug_error::Result<()> {
        self.items.reserve(values.len());
        for value in values {
            self.add_value(*value)?
        }
        Ok(())
    }

    fn add_from_iterator(
        &mut self,
        values: impl Iterator<Item = Value<'v>>,
    ) -> slug_error::Result<()> {
        let (lower, upper) = values.size_hint();
        self.items.reserve(upper.unwrap_or(lower));
        values
            .into_iter()
            .try_for_each(|value| self.add_value(value))?;
        Ok(())
    }

    /// Add values to the artifact that don't show up on the command line, but do for dependency
    fn add_hidden(&mut self, value: StarlarkCommandLineValueUnpack<'v>) -> slug_error::Result<()> {
        match value {
            StarlarkCommandLineValueUnpack::List(values) => {
                for value in values.content() {
                    self.add_hidden(StarlarkCommandLineValueUnpack::unpack_value_err(*value)?)?
                }
            }
            StarlarkCommandLineValueUnpack::CommandLineArg(arg) => {
                self.hidden.push(arg);
            }
        }
        Ok(())
    }
}

struct StarlarkCommandLineMut<'v> {
    value: Value<'v>,
    borrow: RefMut<'v, StarlarkCommandLineData<'v>>,
}

impl<'v> StarlarkTypeRepr for StarlarkCommandLineMut<'v> {
    type Canonical = <StarlarkCmdArgs<'v> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        StarlarkCmdArgs::starlark_type_repr()
    }
}

impl<'v> UnpackValue<'v> for StarlarkCommandLineMut<'v> {
    type Error = Infallible;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        Ok(value.downcast_ref::<StarlarkCmdArgs>().map(|v| Self {
            value,
            borrow: v.0.borrow_mut(),
        }))
    }
}

impl<'v> AllocValue<'v> for StarlarkCommandLineMut<'v> {
    fn alloc_value(self, _heap: Heap<'v>) -> Value<'v> {
        self.value
    }
}

fn cmd_args<'v>(x: Value<'v>) -> FieldsRef<'v, impl Fields<'v>> {
    if let Some(x) = x.downcast_ref::<StarlarkCmdArgs>() {
        FieldsRef(Either::Left(x.0.borrow()), PhantomData)
    } else if let Some(x) = x.downcast_ref::<FrozenStarlarkCmdArgs>() {
        FieldsRef(Either::Right(x), PhantomData)
    } else {
        unreachable!("This parameter must always be a type of command args")
    }
}

/// The `cmd_args` type is created by `cmd_args()` and is consumed by `ctx.actions.run`.
/// The type is a mutable collection of strings and `artifact` values.
/// In general, command lines, artifacts, strings, `RunInfo` and lists thereof can be added to or used to construct a `cmd_args` value.
/// All these methods operate mutably on `cmd` and return that value too.
// TODO(nga): `cmd_args` should be immutable, so that all parameters should be
//   either set in constructor, or operations like `hidden` should return a copy
//   rather than modify this. https://fburl.com/workplace/ihkplvbn
#[starlark_module]
fn cmd_args_methods(builder: &mut MethodsBuilder) {
    /// A list of arguments to be added to the command line, which may including `cmd_args`, artifacts, strings, `RunInfo` or lists thereof.
    /// Note that this operation mutates the input `cmd_args`.
    /// Bazel compatibility: supports `format` named parameter with `%s` placeholder.
    fn add<'v>(
        mut this: StarlarkCommandLineMut<'v>,
        heap: Heap<'v>,
        args: &Arguments<'v, '_>,
    ) -> starlark::Result<StarlarkCommandLineMut<'v>> {
        // Extract optional 'format' named parameter (Bazel compatibility)
        let named = args.names_map()?;
        let format_str = named.get("format").and_then(|v| v.unpack_str());

        let values: Vec<Value<'v>> = args.positions(heap)?.collect();
        match (values.len(), format_str) {
            // 2-arg form with format: add("--flag", value, format="%s")
            // In Bazel, format only applies to the value (second arg), not the flag name.
            (2, Some(fmt)) => {
                // Add arg_name as-is
                this.borrow.add_from_iterator(std::iter::once(values[0]))?;
                // Format and add value
                let val_str = if let Ok(Some(path_val)) = values[1].get_attr("path", heap) {
                    path_val.to_str()
                } else {
                    values[1].to_str()
                };
                let formatted = fmt.replace("%s", &val_str);
                this.borrow
                    .add_from_iterator(std::iter::once(heap.alloc_str(&formatted).to_value()))?;
            }
            // 1-arg form with format: add(value, format="%s")
            (1, Some(fmt)) => {
                let val_str = if let Ok(Some(path_val)) = values[0].get_attr("path", heap) {
                    path_val.to_str()
                } else {
                    values[0].to_str()
                };
                let formatted = fmt.replace("%s", &val_str);
                this.borrow
                    .add_from_iterator(std::iter::once(heap.alloc_str(&formatted).to_value()))?;
            }
            // No format: add all positional args as-is
            _ => {
                this.borrow.add_from_iterator(values.into_iter())?;
            }
        }
        Ok(this)
    }

    /// Bazel-compatible: add all values from a list or depset.
    ///
    /// Supports both 1-arg form: `add_all(values, ...)` and
    /// 2-arg form: `add_all("--flag", values, ...)` where the flag is added once before all values.
    ///
    /// Supports `before_each` to add a string before each element,
    /// `format_each` to format each element, and `map_each` to transform elements.
    fn add_all<'v>(
        mut this: StarlarkCommandLineMut<'v>,
        #[starlark(require = pos, default = starlark::values::none::NoneType)]
        arg_name_or_values: Value<'v>,
        #[starlark(require = pos, default = starlark::values::none::NoneType)] maybe_values: Value<
            'v,
        >,
        #[starlark(require = named, default = "")] before_each: &str,
        #[starlark(require = named, default = "")] format_each: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] map_each: Value<
            'v,
        >,
        #[starlark(require = named, default = false)] uniquify: bool,
        #[starlark(require = named, default = false)] expand_directories: bool,
        #[starlark(require = named, default = false)] terminate_with: Value<'v>,
        #[starlark(require = named, default = false)] allow_closure: bool,
        // Bazel default is True: if empty, omit the arg_name prefix too
        #[starlark(require = named, default = true)] omit_if_empty: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkCommandLineMut<'v>> {
        let _ = (expand_directories, allow_closure);
        let heap = eval.heap();

        // Determine if 2-arg form (arg_name, values) or 1-arg form (values)
        let (arg_name, values) = if !maybe_values.is_none() {
            // 2-arg form: add_all("--flag", values, ...)
            let name = arg_name_or_values.unpack_str().unwrap_or("");
            (Some(name.to_owned()), maybe_values)
        } else {
            // 1-arg form: add_all(values, ...)
            (None, arg_name_or_values)
        };

        // If values is None, nothing to add
        if values.is_none() {
            return Ok(this);
        }

        let can_defer_depset = before_each.is_empty()
            && format_each.is_empty()
            && map_each.is_none()
            && !uniquify
            && terminate_with.unpack_str().is_none()
            && can_defer_depset_command_line(values)?;
        if can_defer_depset {
            if omit_if_empty && crate::interpreter::rule_defs::depset::depset_is_empty(values)? {
                return Ok(this);
            }
            if let Some(ref name) = arg_name {
                if !name.is_empty() {
                    this.borrow.add_value(heap.alloc_str(name).to_value())?;
                }
            }
            if !crate::interpreter::rule_defs::depset::depset_is_empty(values)? {
                this.borrow.add_value(depset_command_line_arg(
                    heap,
                    values,
                    DepsetCommandLineArgMode::AddAll,
                ))?;
            }
            return Ok(this);
        }

        // Collect values from list, tuple, or depset.
        let items: Vec<Value<'v>> = if let Some(list) = ListRef::from_value(values) {
            list.iter().collect()
        } else if let Some(tuple) = TupleRef::from_value(values) {
            tuple.iter().collect()
        } else if crate::interpreter::rule_defs::depset::is_depset_value(values) {
            crate::interpreter::rule_defs::depset::depset_to_list(values, heap)?
        } else if let Ok(iter) = values.iterate(heap) {
            iter.collect()
        } else {
            vec![values]
        };

        // Apply map_each callback if provided
        let has_map_each = !map_each.is_none();
        let mapped_items: Vec<Value<'v>> = if has_map_each {
            let mut result = Vec::new();
            for item in &items {
                let mapped = eval_map_each(eval, map_each, *item)?;
                append_map_each_result(&mut result, mapped);
            }
            result
        } else {
            items
        };

        // Deduplicate if requested
        let final_items = if uniquify {
            let mut seen = std::collections::HashSet::new();
            mapped_items
                .into_iter()
                .filter(|item| seen.insert(item.to_str().to_string()))
                .collect::<Vec<_>>()
        } else {
            mapped_items
        };

        let items_were_added = !final_items.is_empty();

        if omit_if_empty && !items_were_added {
            return Ok(this);
        }

        // In 2-arg form, add the arg_name once before all values
        if let Some(ref name) = arg_name {
            if !name.is_empty() {
                let s = heap.alloc_str(name).to_value();
                this.borrow.add_value(s)?;
            }
        }

        for item in final_items {
            if !before_each.is_empty() {
                let s = heap.alloc_str(before_each).to_value();
                this.borrow.add_value(s)?;
            }
            if !format_each.is_empty() {
                let val_str = if let Ok(Some(path_val)) = item.get_attr("path", heap) {
                    path_val.to_str()
                } else {
                    item.to_str()
                };
                let formatted = format_each.replace("%s", &val_str);
                let s = heap.alloc_str(&formatted).to_value();
                this.borrow.add_value(s)?;
            } else {
                this.borrow.add_value(item)?;
            }
        }

        // Add terminate_with string after the last item, if provided and items were added
        if items_were_added {
            if let Some(term_str) = terminate_with.unpack_str() {
                let s = heap.alloc_str(term_str).to_value();
                this.borrow.add_value(s)?;
            }
        }

        Ok(this)
    }

    /// Bazel-compatible: add all values joined with a separator.
    /// Supports both 1-arg form: add_joined(values, join_with=...)
    /// and 2-arg form: add_joined("--flag", values, join_with=...)
    fn add_joined<'v>(
        mut this: StarlarkCommandLineMut<'v>,
        #[starlark(require = pos, default = starlark::values::none::NoneType)]
        arg_name_or_values: Value<'v>,
        #[starlark(require = pos, default = starlark::values::none::NoneType)] maybe_values: Value<
            'v,
        >,
        #[starlark(require = named, default = ",")] join_with: &str,
        #[starlark(require = named, default = "")] format_each: &str,
        #[starlark(require = named, default = "")] format_joined: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] map_each: Value<
            'v,
        >,
        // Bazel default is True: if empty, omit the arg_name prefix too
        #[starlark(require = named, default = true)] omit_if_empty: bool,
        #[starlark(require = named, default = false)] uniquify: bool,
        #[starlark(require = named, default = false)] expand_directories: bool,
        #[starlark(require = named, default = false)] allow_closure: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkCommandLineMut<'v>> {
        let _ = (expand_directories, allow_closure);
        let heap = eval.heap();
        // Determine if 2-arg form (arg_name, values) or 1-arg form (values)
        let (arg_name, values) = if !maybe_values.is_none() {
            // 2-arg form: add_joined("--flag", values, ...)
            let name = arg_name_or_values.unpack_str().unwrap_or("");
            (Some(name.to_owned()), maybe_values)
        } else {
            // 1-arg form: add_joined(values, ...)
            (None, arg_name_or_values)
        };

        if values.is_none() {
            return Ok(this);
        }

        let can_defer_depset = format_each.is_empty()
            && format_joined.is_empty()
            && map_each.is_none()
            && !uniquify
            && can_defer_depset_command_line(values)?;
        if can_defer_depset {
            if omit_if_empty && crate::interpreter::rule_defs::depset::depset_is_empty(values)? {
                return Ok(this);
            }
            if let Some(name) = arg_name {
                if !name.is_empty() {
                    this.borrow.add_value(heap.alloc_str(&name).to_value())?;
                }
            }
            this.borrow.add_value(depset_command_line_arg(
                heap,
                values,
                DepsetCommandLineArgMode::AddJoined {
                    join_with: join_with.to_owned(),
                },
            ))?;
            return Ok(this);
        }

        let raw_items: Vec<Value<'v>> = if let Some(list) = ListRef::from_value(values) {
            list.iter().collect()
        } else if let Some(tuple) = TupleRef::from_value(values) {
            tuple.iter().collect()
        } else if crate::interpreter::rule_defs::depset::is_depset_value(values) {
            crate::interpreter::rule_defs::depset::depset_to_list(values, heap)?
        } else if let Ok(iter) = values.iterate(heap) {
            iter.collect()
        } else {
            vec![values]
        };

        // Apply map_each if provided
        let mapped_items: Vec<Value<'v>> = if !map_each.is_none() {
            let mut result = Vec::new();
            for item in &raw_items {
                let mapped = eval_map_each(eval, map_each, *item)?;
                append_map_each_result(&mut result, mapped);
            }
            result
        } else {
            raw_items
        };

        let items: Vec<String> = mapped_items
            .iter()
            .map(|v| -> starlark::Result<String> {
                ValueAsCommandLineLike::unpack_value_err(*v)?;
                let val_str = if let Ok(Some(path_val)) = v.get_attr("path", heap) {
                    path_val.to_str()
                } else {
                    v.to_str()
                };
                Ok(if !format_each.is_empty() {
                    format_each.replace("%s", &val_str)
                } else {
                    val_str
                })
            })
            .collect::<starlark::Result<_>>()?;

        // Deduplicate if requested
        let items = if uniquify {
            let mut seen = std::collections::HashSet::new();
            items
                .into_iter()
                .filter(|item| seen.insert(item.clone()))
                .collect()
        } else {
            items
        };

        if omit_if_empty && items.is_empty() {
            return Ok(this);
        }

        let joined = items.join(join_with);
        let result = if !format_joined.is_empty() {
            format_joined.replace("%s", &joined)
        } else {
            joined
        };

        // If arg_name is provided, add it as a separate argument (Bazel behavior)
        if let Some(name) = arg_name {
            if !name.is_empty() {
                let s = heap.alloc_str(&name).to_value();
                this.borrow.add_value(s)?;
            }
        }
        let s = heap.alloc_str(&result).to_value();
        this.borrow.add_value(s)?;
        Ok(this)
    }

    /// Set the format for entries in a param file.
    ///
    /// Supported formats:
    /// - `"multiline"`: one argument per line (default)
    /// - `"flag_per_line"`: flag name and value on separate lines
    /// - `"shell"`: shell-quoted arguments
    fn set_param_file_format<'v>(
        mut this: StarlarkCommandLineMut<'v>,
        #[starlark(require = pos)] format: &str,
    ) -> starlark::Result<StarlarkCommandLineMut<'v>> {
        let fmt = match format {
            "multiline" => ParamFileFormat::Multiline,
            "flag_per_line" => ParamFileFormat::FlagPerLine,
            "shell" => ParamFileFormat::Shell,
            other => {
                let e: slug_error::Error =
                    CommandLineError::UnknownParamFileFormat(other.to_owned()).into();
                return Err(e.into());
            }
        };
        if let Some(pf) = this.borrow.param_file.as_mut() {
            pf.format = fmt;
        }
        // If no param_file has been set yet, format setting is ignored (Bazel behavior)
        Ok(this)
    }

    /// Configure this `cmd_args` to use a param file for arguments.
    ///
    /// When the argument list is long (or when `use_always=True`), the arguments will be
    /// written to a temporary file and replaced on the command line with a single argument
    /// derived from `param_file_arg` (where `%s` is replaced by the file path).
    ///
    /// Example:
    /// ```python
    /// args = cmd_args()
    /// args.use_param_file("@%s", use_always=True)
    /// ```
    fn use_param_file<'v>(
        mut this: StarlarkCommandLineMut<'v>,
        #[starlark(require = pos, default = "")] positional_param_file_arg: &str,
        #[starlark(require = named, default = "")] param_file_arg: &str,
        #[starlark(require = named, default = false)] use_always: bool,
    ) -> starlark::Result<StarlarkCommandLineMut<'v>> {
        // Accept param_file_arg as either positional or named
        let arg = if !positional_param_file_arg.is_empty() {
            positional_param_file_arg
        } else {
            param_file_arg
        };
        if !arg.is_empty() {
            let pf = this.borrow.param_file_mut(arg);
            pf.use_always = use_always;
        }
        Ok(this)
    }

    /// Make all artifact paths relative to a given location. Typically used when the command
    /// you are running changes directory.
    ///
    /// By default, the paths are relative to the artifacts themselves (equivalent to
    /// `parent = 0`). Use `parent` to make the paths relative to an ancestor directory.
    /// For example `parent = 1` would make all paths relative to the containing dirs
    /// of any artifacts in the `cmd_args`.
    ///
    /// ```python
    /// dir = symlinked_dir(...)
    /// script = [
    ///     cmd_args(cmd_args(dir, format = "cd {}"),
    ///     original_script.relative_to(dir)
    /// ]
    /// ```
    fn relative_to<'v>(
        mut this: StarlarkCommandLineMut<'v>,
        #[starlark(require = pos)] directory: ValueOf<'v, RelativeOrigin<'v>>,
        #[starlark(require = named, default = 0u32)] parent: u32,
    ) -> starlark::Result<StarlarkCommandLineMut<'v>> {
        this.borrow.options_mut().relative_to = Some((directory.as_unchecked(), parent));
        Ok(this)
    }

    /// Returns a copy of the `cmd_args` such that any modifications to the original or the returned value will not impact each other.
    /// Note that this is a shallow copy, so any inner `cmd_args` can still be modified.
    fn copy<'v>(this: Value<'v>) -> starlark::Result<StarlarkCmdArgs<'v>> {
        Ok(cmd_args(this).copy())
    }

    /// Collect all the inputs (including hidden) referenced by this command line.
    ///
    /// The returned collection is opaque and primarily useful for:
    /// - Checking if the command has any artifact dependencies
    /// - Comparing input sets between different `cmd_args` objects
    ///
    /// The collection supports `len()` and equality comparisons but cannot be iterated.
    ///
    /// Example:
    /// ```python
    /// def _impl(ctx):
    ///     cmd = cmd_args(ctx.attrs.srcs)
    ///
    ///     # Check if command has any inputs
    ///     if len(cmd.inputs) > 0:
    ///         pass
    ///
    ///     # Compare input sets
    ///     other_cmd = cmd_args(ctx.attrs.headers, hidden = ctx.attrs.resources)
    ///     if cmd.inputs == other_cmd.inputs:
    ///         pass
    /// ```
    #[starlark(attribute)]
    fn inputs<'v>(this: Value<'v>) -> starlark::Result<StarlarkCommandLineInputs> {
        let mut visitor = SimpleCommandLineArtifactVisitor::new();
        cmd_args(this).visit_artifacts(&mut visitor)?;
        Ok(StarlarkCommandLineInputs {
            inputs: visitor.inputs,
        })
    }

    /// Collect all the outputs (including hidden) referenced by this command line.
    #[starlark(attribute)]
    fn outputs<'v>(
        this: Value<'v>,
        heap: Heap<'_>,
    ) -> starlark::Result<Vec<StarlarkOutputArtifact<'v>>> {
        let mut visitor = SimpleCommandLineArtifactVisitor::new();
        cmd_args(this).visit_artifacts(&mut visitor)?;
        let mut outputs =
            Vec::with_capacity(visitor.declared_outputs.len() + visitor.frozen_outputs.len());
        for out in visitor.declared_outputs {
            let declared = heap.alloc_typed(StarlarkDeclaredArtifact::new(
                None,
                (*out).dupe(),
                AssociatedArtifacts::new(),
            ));
            outputs.push(StarlarkOutputArtifact::new(declared));
        }
        // FIXME(JakobDegen): We should probably not be allowing people to get an `OutputArtifact`
        // for an artifact declared in a downstream action??
        for out in visitor.frozen_outputs {
            let declared = heap.alloc_typed(StarlarkDeclaredArtifact::new(
                None,
                (*out
                    .allocate_new_output_artifact_for(heap)
                    .with_internal_error(|| {
                        "Expecting artifact to be output artifact".to_owned()
                    })?)
                .dupe(),
                AssociatedArtifacts::new(),
            ));
            outputs.push(StarlarkOutputArtifact::new(declared));
        }
        Ok(outputs)
    }
}

#[starlark_module]
pub fn register_cmd_args(builder: &mut GlobalsBuilder) {
    #[starlark(as_type = FrozenStarlarkCmdArgs)]
    /// The `cmd_args` type is created by this function and is consumed by `ctx.actions.run`.
    /// The type is a mutable collection of strings and artifact values.
    /// In general, command lines, artifacts, strings, `RunInfo` and lists thereof can be added to or used to construct a `cmd_args` value.
    ///
    /// The arguments are:
    ///
    /// * `*args` - a list of things to add to the command line, each of which must be coercible to a command line. Further items can be added with `cmd.add`.
    /// * `format` - a string that provides a format to apply to the argument. for example, `cmd_args(x, format="--args={}")` would prepend `--args=` before `x`, or if `x` was a list, before each element in `x`.
    /// * `delimiter` - added between arguments to join them together. For example, `cmd_args(["--args=",x], delimiter="")` would produce a single argument to the underlying tool.
    /// * `prepend` - added as a separate argument before each argument.
    /// * `quote` - indicates whether quoting is to be applied to each argument. The only current valid value is `"shell"`.
    /// * `ignore_artifacts` - if `True`, artifacts paths are used, but artifacts are not pulled.
    /// * `hidden` - artifacts not present on the command line, but added as dependencies.
    /// * `absolute_prefix` and `absolute_suffix` - added to the start and end of each artifact.
    /// * `parent` - for all the artifacts use their `parent`th directory (e.g. `parent = 1` for the directory the artifact is located, `parent = 2` for that directory's parent, etc.).
    /// * `relative_to` - make all artifact paths relative to a given location.
    /// * `replace_regex` - replaces arguments with a regular expression.
    ///
    /// ### `ignore_artifacts`
    ///
    /// `ignore_artifacts=True` makes `cmd_args` to have no declared dependencies.
    /// Allows you to reference the path of an artifact _without_ introducing dependencies on it.
    ///
    /// As an example where this can be useful, consider passing a dependency that is only accessed at runtime, but whose path
    /// must be baked into the binary. As an example:
    ///
    /// ```python
    /// resources = cmd_args(resource_file, format = "-DFOO={}", ignore_artifacts=True)
    /// ctx.actions.run(cmd_args("gcc", "-c", source_file, resources))
    /// ```
    ///
    /// Note that `ignore_artifacts` sets all artifacts referenced by this `cmd_args` to be ignored, including those added afterwards,
    /// so generally create a special `cmd_args` and scope it quite tightly.
    ///
    /// If you actually do use the inputs referenced by this command,
    /// you will either error out due to missing dependencies (if running actions remotely)
    /// or have untracked dependencies that will fail to rebuild when it should.
    ///
    /// ### `hidden`
    ///
    /// Things to add to the command line which do not show up but are added as dependencies.
    /// The values can be anything normally permissible to pass to `add`.
    ///
    /// Typically used if the command you are running implicitly depends on files that are not
    /// passed on the command line, e.g. headers in the case of a C compilation.
    ///
    /// ### `absolute_prefix` and `absolute_suffix`
    ///
    /// Adds a prefix to the start or end of every artifact.
    ///
    /// Prefix is often used if you have a `$ROOT` variable
    /// in a shell script and want to use it to make files absolute.
    ///
    /// Suffix is often used in conjunction with `absolute_prefix`
    /// to wrap artifacts in function calls.
    ///
    /// ```python
    /// cmd_args(script, absolute_prefix = "$ROOT/")
    /// cmd_args(script, absolute_prefix = "call", absolute_suffix = ")")
    /// ```
    ///
    /// ### `parent`
    ///
    /// For all the artifacts use their parent directory.
    ///
    /// Typically used when the file name is passed one way, and the directory another,
    /// e.g. `cmd_args(artifact, format="-L{}", parent=1)`.
    ///
    /// ### `relative_to=dir` or `relative_to=(dir, parent)`
    ///
    /// Make all artifact paths relative to a given location. Typically used when the command
    /// you are running changes directory.
    ///
    /// By default, the paths are relative to the artifacts themselves (equivalent to
    /// parent equals to `0`). Use `parent` to make the paths relative to an ancestor directory.
    /// For example parent equals to `1` would make all paths relative to the containing dirs
    /// of any artifacts in the `cmd_args`.
    ///
    /// ```python
    /// dir = symlinked_dir(...)
    /// script = [
    ///     cmd_args(dir, format = "cd {}", relative_to=dir),
    /// ]
    /// ```
    ///
    /// ### `replace_regex`
    ///
    /// Replaces all parts matching pattern regular expression (or regular expressions)
    /// in each argument with replacement strings.
    fn cmd_args<'v>(
        #[starlark(args)] args: UnpackTuple<StarlarkCommandLineValueUnpack<'v>>,
        hidden: Option<StarlarkCommandLineValueUnpack<'v>>,
        delimiter: Option<StringValue<'v>>,
        format: Option<StringValue<'v>>,
        prepend: Option<StringValue<'v>>,
        quote: Option<&str>,
        #[starlark(default = false)] ignore_artifacts: bool,
        absolute_prefix: Option<StringValue<'v>>,
        absolute_suffix: Option<StringValue<'v>>,
        #[starlark(default = 0)] parent: u32,
        relative_to: Option<
            Either<ValueOf<'v, RelativeOrigin<'v>>, (ValueOf<'v, RelativeOrigin<'v>>, u32)>,
        >,
        #[starlark(default = Either::Right(UnpackList::default()))] replace_regex: Either<
            (CmdArgsRegex<'v>, StringValue<'v>),
            UnpackList<(CmdArgsRegex<'v>, StringValue<'v>)>,
        >,
    ) -> starlark::Result<StarlarkCmdArgs<'v>> {
        let quote = quote.try_map(QuoteStyle::parse)?;
        let mut builder = StarlarkCommandLineData::default();
        if delimiter.is_some()
            || format.is_some()
            || prepend.is_some()
            || quote.is_some()
            || ignore_artifacts
            || absolute_prefix.is_some()
            || absolute_suffix.is_some()
            || parent != 0
            || relative_to.is_some()
        {
            let opts = builder.options_mut();
            opts.delimiter = delimiter;
            opts.format = format;
            opts.prepend = prepend;
            opts.quote = quote;
            opts.ignore_artifacts = ignore_artifacts;
            opts.absolute_prefix = absolute_prefix;
            opts.absolute_suffix = absolute_suffix;
            opts.parent = parent;
            opts.relative_to = relative_to.map(|either| {
                let (relative_to, parent) = either.map_left(|o| (o, 0)).into_inner();
                (relative_to.as_unchecked(), parent)
            });
        }
        let replace_regex: Vec<(CmdArgsRegex, StringValue)> = replace_regex
            .map_left(|x| vec![x])
            .map_right(|x| x.items)
            .into_inner();
        if !replace_regex.is_empty() {
            for (pattern, _replacement) in &replace_regex {
                pattern.validate()?;
            }
            builder.options_mut().replacements = Some(Box::new(replace_regex));
        }
        for v in args.items {
            builder.add_value_typed(v)?;
        }
        if let Some(hidden) = hidden {
            builder.add_hidden(hidden)?;
        }
        Ok(StarlarkCmdArgs(RefCell::new(builder)))
    }
}

/// A wrapper for a [StarlarkCmdArgs]'s inputs. This is an opaque type that only allows
/// debug-printing and querying the length to tell if any inputs exist.
#[derive(Debug, PartialEq, ProvidesStaticType, NoSerialize, Allocative)]
pub struct StarlarkCommandLineInputs {
    pub inputs: IndexSet<ArtifactGroup>,
}

starlark_simple_value!(StarlarkCommandLineInputs);

impl Display for StarlarkCommandLineInputs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt_container(f, "command_line_inputs(", ")", self.inputs.iter())
    }
}

#[starlark_value(type = "CommandLineInputs")]
impl<'v> StarlarkValue<'v> for StarlarkCommandLineInputs {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(command_line_inputs_methods)
    }

    fn length(&self) -> starlark::Result<i32> {
        self.inputs
            .len()
            .try_into()
            .map_err(starlark::Error::new_other)
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        if let Some(other) = other.downcast_ref::<Self>() {
            Ok(self.inputs == other.inputs)
        } else {
            Ok(false)
        }
    }
}

/// An opaque collection of input artifacts referenced by a `cmd_args` object.
///
/// Returned by the [`.inputs`](../cmd_args/#cmd_argsinputs) attribute. Supports `len()` and equality comparisons.
/// See the [`.inputs`](../cmd_args/#cmd_argsinputs) attribute documentation for usage examples.
#[starlark_module]
fn command_line_inputs_methods(_builder: &mut MethodsBuilder) {
    // No methods currently - this type only supports len() and equality via StarlarkValue trait
}

#[starlark_module]
pub(crate) fn register_command_line_inputs(globals: &mut GlobalsBuilder) {
    const CommandLineInputs: StarlarkValueAsType<StarlarkCommandLineInputs> =
        StarlarkValueAsType::new();
}
