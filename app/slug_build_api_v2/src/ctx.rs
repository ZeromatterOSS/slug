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

use slug_identity_v2::CanonicalLabel;

use crate::actions::ActionOutput;
use crate::actions::CtxActions;
use crate::attrs::AttributeMap;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ResolvedCommand {
    command: String,
    inputs: Vec<String>,
}

impl ResolvedCommand {
    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn inputs(&self) -> &[String] {
        &self.inputs
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RuleContext {
    label: CanonicalLabel,
    attrs: AttributeMap,
    file_attrs: BTreeMap<String, String>,
    files_attrs: BTreeMap<String, Vec<String>>,
    executable_attrs: BTreeMap<String, String>,
    outputs: BTreeMap<String, ActionOutput>,
    actions: CtxActions,
    fragments: BTreeMap<String, String>,
    toolchains: BTreeMap<String, String>,
    exec_groups: BTreeMap<String, String>,
    vars: BTreeMap<String, String>,
    locations: BTreeMap<String, String>,
}

impl RuleContext {
    pub fn builder(label: CanonicalLabel) -> RuleContextBuilder {
        RuleContextBuilder::new(label)
    }

    pub fn label(&self) -> &CanonicalLabel {
        &self.label
    }

    pub fn attr(&self) -> &AttributeMap {
        &self.attrs
    }

    pub fn file(&self, name: &str) -> Option<&str> {
        self.file_attrs.get(name).map(String::as_str)
    }

    pub fn files(&self, name: &str) -> Option<&[String]> {
        self.files_attrs.get(name).map(Vec::as_slice)
    }

    pub fn executable(&self, name: &str) -> Option<&str> {
        self.executable_attrs.get(name).map(String::as_str)
    }

    pub fn output(&self, name: &str) -> Option<&ActionOutput> {
        self.outputs.get(name)
    }

    pub fn actions(&self) -> &CtxActions {
        &self.actions
    }

    pub fn actions_mut(&mut self) -> &mut CtxActions {
        &mut self.actions
    }

    pub fn fragment(&self, name: &str) -> Option<&str> {
        self.fragments.get(name).map(String::as_str)
    }

    pub fn toolchain(&self, key: &str) -> Option<&str> {
        self.toolchains.get(key).map(String::as_str)
    }

    pub fn exec_group(&self, name: &str) -> Option<&str> {
        self.exec_groups.get(name).map(String::as_str)
    }

    pub fn var(&self, name: &str) -> Option<&str> {
        self.vars.get(name).map(String::as_str)
    }

    pub fn expand_location(&self, value: &str) -> Result<String, String> {
        let mut expanded = value.to_owned();
        for (label, path) in &self.locations {
            expanded = expanded.replace(&format!("$(location {label})"), path);
        }
        if expanded.contains("$(location ") {
            return Err(format!("unresolved location in command: {value}"));
        }
        Ok(expanded)
    }

    pub fn resolve_command(
        &self,
        command: &str,
        tools: Vec<String>,
    ) -> Result<ResolvedCommand, String> {
        Ok(ResolvedCommand {
            command: self.expand_location(command)?,
            inputs: tools,
        })
    }
}

#[derive(Debug, Clone)]
pub struct RuleContextBuilder {
    ctx: RuleContext,
}

impl RuleContextBuilder {
    pub fn new(label: CanonicalLabel) -> Self {
        Self {
            ctx: RuleContext {
                label,
                attrs: AttributeMap::new(),
                file_attrs: BTreeMap::new(),
                files_attrs: BTreeMap::new(),
                executable_attrs: BTreeMap::new(),
                outputs: BTreeMap::new(),
                actions: CtxActions::new(),
                fragments: BTreeMap::new(),
                toolchains: BTreeMap::new(),
                exec_groups: BTreeMap::new(),
                vars: BTreeMap::new(),
                locations: BTreeMap::new(),
            },
        }
    }

    pub fn attrs(mut self, attrs: AttributeMap) -> Self {
        self.ctx.attrs = attrs;
        self
    }

    pub fn file(mut self, name: impl Into<String>, file: impl Into<String>) -> Self {
        self.ctx.file_attrs.insert(name.into(), file.into());
        self
    }

    pub fn files(mut self, name: impl Into<String>, files: Vec<String>) -> Self {
        self.ctx.files_attrs.insert(name.into(), files);
        self
    }

    pub fn executable(mut self, name: impl Into<String>, file: impl Into<String>) -> Self {
        self.ctx.executable_attrs.insert(name.into(), file.into());
        self
    }

    pub fn output(mut self, name: impl Into<String>, output: ActionOutput) -> Self {
        self.ctx.outputs.insert(name.into(), output);
        self
    }

    pub fn fragment(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.ctx.fragments.insert(name.into(), value.into());
        self
    }

    pub fn toolchain(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.ctx.toolchains.insert(key.into(), value.into());
        self
    }

    pub fn exec_group(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.ctx.exec_groups.insert(name.into(), value.into());
        self
    }

    pub fn var(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.ctx.vars.insert(name.into(), value.into());
        self
    }

    pub fn location(mut self, label: impl Into<String>, path: impl Into<String>) -> Self {
        self.ctx.locations.insert(label.into(), path.into());
        self
    }

    pub fn build(self) -> RuleContext {
        self.ctx
    }
}
