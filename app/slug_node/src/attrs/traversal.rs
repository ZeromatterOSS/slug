/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use dupe::Dupe;
use slug_core::configuration::transition::id::TransitionId;
use slug_core::package::source_path::SourcePathRef;
use slug_core::plugins::PluginKind;
use slug_core::provider::label::ProvidersLabel;
use slug_core::target::label::label::TargetLabel;

use crate::attrs::attr_type::configuration_dep::ConfigurationDepKind;

pub trait CoercedAttrTraversal<'a> {
    fn dep(&mut self, dep: &ProvidersLabel) -> slug_error::Result<()>;
    fn exec_dep(&mut self, dep: &'a ProvidersLabel) -> slug_error::Result<()> {
        self.dep(dep)
    }

    fn toolchain_dep(&mut self, dep: &'a ProvidersLabel) -> slug_error::Result<()> {
        self.dep(dep)
    }

    fn transition_dep(
        &mut self,
        dep: &'a ProvidersLabel,
        _tr: &Arc<TransitionId>,
    ) -> slug_error::Result<()> {
        self.dep(dep)
    }

    fn split_transition_dep(
        &mut self,
        dep: &'a ProvidersLabel,
        _tr: &Arc<TransitionId>,
    ) -> slug_error::Result<()> {
        self.dep(dep)
    }

    fn configuration_dep(
        &mut self,
        dep: &ProvidersLabel,
        _kind: ConfigurationDepKind,
    ) -> slug_error::Result<()> {
        self.dep(dep)
    }

    fn plugin_dep(&mut self, dep: &'a TargetLabel, _kind: &PluginKind) -> slug_error::Result<()> {
        let p = ProvidersLabel::default_for(dep.dupe());
        self.dep(&p)
    }

    fn input(&mut self, input: SourcePathRef) -> slug_error::Result<()>;

    fn label(&mut self, _label: &'a ProvidersLabel) -> slug_error::Result<()> {
        Ok(())
    }
}
