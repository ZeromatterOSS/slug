/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::hash::Hash;

use allocative::Allocative;
use derive_more::Display;
use dupe::Dupe;
use pagable::Pagable;
use serde::Serialize;
use serde::Serializer;
use slug_core::cells::CellAliasResolver;
use slug_core::cells::name::CellName;
use slug_core::provider::label::ConfiguredProvidersLabel;
use slug_core::provider::label::NonDefaultProvidersName;
use slug_core::provider::label::ProvidersLabel;
use slug_core::provider::label::ProvidersName;
use slug_core::target::label::label::TargetLabel;
use slug_core::target::name::TargetNameRef;
use starlark::any::ProvidesStaticType;
use starlark::collections::StarlarkHasher;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::Freeze;
use starlark::values::Heap;
use starlark::values::StarlarkValue;
use starlark::values::StringValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::none::NoneOr;
use starlark::values::starlark_value;
use starlark::values::starlark_value_as_type::StarlarkValueAsType;

use crate::types::cell_path::StarlarkCellPath;
use crate::types::cell_root::CellRoot;
use crate::types::package_path::StarlarkPackagePath;
use crate::types::project_root::StarlarkProjectRoot;
use crate::types::target_label::StarlarkConfiguredTargetLabel;
use crate::types::target_label::StarlarkTargetLabel;

impl StarlarkConfiguredProvidersLabel {
    pub fn label(&self) -> &ConfiguredProvidersLabel {
        &self.label
    }
}

/// Container for `ConfiguredProvidersLabel` that gives users access to things like package, cell, etc. This can also be properly stringified by our forthcoming `CommandLine` object
#[derive(Clone, Debug, Display, Trace, Freeze, ProvidesStaticType, Allocative)]
#[display("{}", label)]
#[repr(C)]
pub struct StarlarkConfiguredProvidersLabel {
    #[freeze(identity)]
    label: ConfiguredProvidersLabel,
    /// Active analysis-time resolver used only for Bazel-visible repo strings.
    ///
    /// The resolver is deliberately ignored for Starlark identity: two label
    /// values with the same configured label compare/hash the same way.
    #[freeze(identity)]
    #[trace(unsafe_ignore)]
    cell_alias_resolver: Option<CellAliasResolver>,
    #[freeze(identity)]
    #[trace(unsafe_ignore)]
    root_cell_name: Option<CellName>,
}

starlark_simple_value!(StarlarkConfiguredProvidersLabel);

impl Serialize for StarlarkConfiguredProvidersLabel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.label.serialize(serializer)
    }
}

impl StarlarkConfiguredProvidersLabel {
    pub fn new(label: ConfiguredProvidersLabel) -> Self {
        StarlarkConfiguredProvidersLabel {
            label,
            cell_alias_resolver: None,
            root_cell_name: None,
        }
    }

    pub fn new_with_cell_alias_resolver(
        label: ConfiguredProvidersLabel,
        cell_alias_resolver: Option<CellAliasResolver>,
    ) -> Self {
        Self::new_with_cell_alias_resolver_and_root(label, cell_alias_resolver, None)
    }

    pub fn new_with_cell_alias_resolver_and_root(
        label: ConfiguredProvidersLabel,
        cell_alias_resolver: Option<CellAliasResolver>,
        root_cell_name: Option<CellName>,
    ) -> Self {
        StarlarkConfiguredProvidersLabel {
            label,
            cell_alias_resolver,
            root_cell_name,
        }
    }

    pub fn inner(&self) -> &ConfiguredProvidersLabel {
        &self.label
    }

    pub fn cell_alias_resolver(&self) -> Option<&CellAliasResolver> {
        self.cell_alias_resolver.as_ref()
    }

    pub fn root_cell_name(&self) -> Option<&CellName> {
        self.root_cell_name.as_ref()
    }

    fn bazel_workspace_name(&self) -> String {
        let cell = self.label.target().pkg().cell_name().as_str();
        if self.is_root_workspace_name(cell) {
            String::new()
        } else {
            self.cell_alias_resolver
                .as_ref()
                .and_then(|resolver| resolver.resolve_declared_or_runtime_alias(cell))
                .map(|cell| cell.as_str().to_owned())
                .unwrap_or_else(|| cell.to_owned())
        }
    }

    fn is_root_workspace_name(&self, cell: &str) -> bool {
        if self
            .root_cell_name
            .as_ref()
            .is_some_and(|root| root.as_str() == cell)
        {
            return true;
        }

        #[cfg(test)]
        {
            if self.root_cell_name.is_none() && slug_core::cells::is_root_cell_name(cell) {
                return true;
            }
        }

        false
    }
}

#[starlark_value(type = "Label")]
impl<'v> StarlarkValue<'v> for StarlarkConfiguredProvidersLabel
where
    Self: ProvidesStaticType<'v>,
{
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(configured_label_methods)
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(match StarlarkConfiguredProvidersLabel::from_value(other) {
            Some(other) => self.label == other.label,
            None => false,
        })
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.label.hash(hasher);
        Ok(())
    }
}

/// A label is used to represent a configured target.
#[starlark_module]
fn configured_label_methods(builder: &mut MethodsBuilder) {
    /// For the label `fbcode//slug/hello:world (ovr_config//platform/linux:x86_64-fbcode-46b26edb4b80a905)` this gives back `slug/hello`
    #[starlark(attribute)]
    fn package<'v>(
        this: &'v StarlarkConfiguredProvidersLabel,
        heap: Heap<'v>,
    ) -> starlark::Result<StringValue<'v>> {
        Ok(heap.alloc_str_intern(this.label.target().pkg().cell_relative_path().as_str()))
    }

    /// For the label `fbcode//slug/hello:world (ovr_config//platform/linux:x86_64-fbcode-46b26edb4b80a905)` this gives back `world`
    #[starlark(attribute)]
    fn name<'v>(this: &'v StarlarkConfiguredProvidersLabel) -> starlark::Result<&'v str> {
        Ok(this.label.target().name().as_str())
    }

    #[starlark(attribute)]
    fn sub_target<'v>(
        this: &'v StarlarkConfiguredProvidersLabel,
    ) -> starlark::Result<NoneOr<Vec<&'v str>>> {
        Ok(match this.label.name() {
            ProvidersName::Default => NoneOr::None,
            ProvidersName::NonDefault(flavor) => match flavor.as_ref() {
                NonDefaultProvidersName::Named(names) => {
                    NoneOr::Other(names.iter().map(|p| p.as_str()).collect())
                }
                NonDefaultProvidersName::UnrecognizedFlavor(_) => {
                    unreachable!(
                        "This should have been an error when looking up the corresponding analysis (`{}`)",
                        this.label
                    )
                }
            },
        })
    }

    /// For the label `fbcode//slug/hello:world (ovr_config//platform/linux:x86_64-fbcode-46b26edb4b80a905)` this gives back `fbcode//slug/hello`
    #[starlark(attribute)]
    fn path<'v>(this: &StarlarkConfiguredProvidersLabel) -> starlark::Result<StarlarkCellPath> {
        Ok(StarlarkCellPath(this.label.target().pkg().to_cell_path()))
    }

    /// For the label `fbcode//slug/hello:world (ovr_config//platform/linux:x86_64-fbcode-46b26edb4b80a905)` this gives back `fbcode`
    #[starlark(attribute)]
    fn cell<'v>(this: &'v StarlarkConfiguredProvidersLabel) -> starlark::Result<&'v str> {
        Ok(this.label.target().pkg().cell_name().as_str())
    }

    /// Returns the workspace name (Bazel compatibility).
    /// For the main repository, returns an empty string.
    /// For external repositories, returns the repository name.
    #[starlark(attribute)]
    fn workspace_name<'v>(
        this: &'v StarlarkConfiguredProvidersLabel,
        heap: Heap<'v>,
    ) -> starlark::Result<StringValue<'v>> {
        Ok(heap.alloc_str_intern(&this.bazel_workspace_name()))
    }

    /// Returns the canonical repo name (Bazel compatibility).
    /// Modern replacement for `workspace_name`.
    /// For the main repository, returns an empty string.
    /// For external repositories, returns the repository name.
    #[starlark(attribute)]
    fn repo_name<'v>(
        this: &'v StarlarkConfiguredProvidersLabel,
        heap: Heap<'v>,
    ) -> starlark::Result<StringValue<'v>> {
        Ok(heap.alloc_str_intern(&this.bazel_workspace_name()))
    }

    /// Returns the execution root-relative path for the workspace (Bazel compatibility).
    /// For the main repository, returns "" (empty string).
    /// For external repositories, returns "external/<repo_name>".
    #[starlark(attribute)]
    fn workspace_root<'v>(
        this: &StarlarkConfiguredProvidersLabel,
        heap: Heap<'v>,
    ) -> starlark::Result<StringValue<'v>> {
        let workspace_name = this.bazel_workspace_name();
        if workspace_name.is_empty() {
            Ok(heap.alloc_str_intern(""))
        } else {
            Ok(heap.alloc_str_intern(&format!("external/{workspace_name}")))
        }
    }

    /// Returns the PackagePath for this configured providers label.
    #[starlark(attribute)]
    fn package_path<'v>(
        this: &StarlarkConfiguredProvidersLabel,
    ) -> starlark::Result<StarlarkPackagePath> {
        Ok(StarlarkPackagePath::new(this.label.target().pkg().dupe()))
    }

    /// Obtain a reference to this target label's cell root. This can be used as if it were an
    /// artifact in places that expect one, such as `cmd_args().relative_to`.
    #[starlark(attribute)]
    fn cell_root<'v>(this: &StarlarkConfiguredProvidersLabel) -> starlark::Result<CellRoot> {
        Ok(CellRoot::new(this.label.target().pkg().cell_name()))
    }

    /// Obtain a reference to the project's root. This can be used as if it were an artifact in
    /// places that expect one, such as `cmd_args().relative_to`.
    #[starlark(attribute)]
    fn project_root<'v>(
        this: &StarlarkConfiguredProvidersLabel,
    ) -> starlark::Result<StarlarkProjectRoot> {
        Ok(StarlarkProjectRoot)
    }

    /// For the label `fbcode//slug/hello:world (ovr_config//platform/linux:x86_64-fbcode-46b26edb4b80a905)` this returns the unconfigured underlying target label (`fbcode//slug/hello:world`)
    fn raw_target(
        this: &StarlarkConfiguredProvidersLabel,
    ) -> starlark::Result<StarlarkTargetLabel> {
        Ok(StarlarkTargetLabel::new(
            (*this.label.target().unconfigured()).dupe(),
        ))
    }

    /// Returns the underlying configured target label, dropping the sub target
    fn configured_target(
        this: &StarlarkConfiguredProvidersLabel,
    ) -> starlark::Result<StarlarkConfiguredTargetLabel> {
        Ok(StarlarkConfiguredTargetLabel::new(
            (*this.label.target()).dupe(),
        ))
    }

    /// Resolves a label string relative to this label's package.
    /// If the given string is an absolute label (starts with // or @), it is
    /// returned as-is (as a string). If it starts with ":", it's resolved
    /// relative to this label's package.
    fn relative<'v>(
        this: &StarlarkConfiguredProvidersLabel,
        rel_name: &str,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        if rel_name.starts_with("//") || rel_name.starts_with('@') {
            // Absolute label - return as Label with same config
            // For now, return as string since we can't parse arbitrary labels
            // without a cell resolver
            Ok(heap.alloc(rel_name))
        } else {
            // Relative label - resolve against this package
            let name = if let Some(stripped) = rel_name.strip_prefix(':') {
                stripped
            } else {
                rel_name
            };
            let target_name = TargetNameRef::new(name).map_err(|e| {
                starlark::Error::from(starlark::values::ValueError::IncorrectParameterTypeNamed(
                    format!("Invalid target name '{}': {}", name, e),
                ))
            })?;
            let new_target = TargetLabel::new(this.label.target().pkg(), target_name);
            let configured = new_target.configure_pair(this.label.target().cfg_pair().dupe());
            let new_label = ConfiguredProvidersLabel::new(configured, ProvidersName::Default);
            Ok(heap.alloc(
                StarlarkConfiguredProvidersLabel::new_with_cell_alias_resolver_and_root(
                    new_label,
                    this.cell_alias_resolver.clone(),
                    this.root_cell_name.clone(),
                ),
            ))
        }
    }

    /// Returns a new Label in the same package with a different target name.
    /// Equivalent to `Label("//pkg:new_name")` but preserves configuration.
    fn same_package_label<'v>(
        this: &StarlarkConfiguredProvidersLabel,
        name: &str,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let target_name = TargetNameRef::new(name).map_err(|e| {
            starlark::Error::from(starlark::values::ValueError::IncorrectParameterTypeNamed(
                format!("Invalid target name '{}': {}", name, e),
            ))
        })?;
        let new_target = TargetLabel::new(this.label.target().pkg(), target_name);
        let configured = new_target.configure_pair(this.label.target().cfg_pair().dupe());
        let new_label = ConfiguredProvidersLabel::new(configured, ProvidersName::Default);
        Ok(heap.alloc(
            StarlarkConfiguredProvidersLabel::new_with_cell_alias_resolver_and_root(
                new_label,
                this.cell_alias_resolver.clone(),
                this.root_cell_name.clone(),
            ),
        ))
    }
}

impl StarlarkProvidersLabel {
    pub fn label(&self) -> &ProvidersLabel {
        &self.label
    }
}

/// Container for `ProvidersLabel` that gives users access to things like package, cell, etc.
#[derive(
    Clone,
    Debug,
    Display,
    Trace,
    Freeze,
    ProvidesStaticType,
    Allocative,
    Serialize,
    Pagable
)]
#[display("{}", label)]
#[repr(C)]
#[serde(transparent)]
pub struct StarlarkProvidersLabel {
    #[freeze(identity)]
    label: ProvidersLabel,
}

starlark_simple_value!(StarlarkProvidersLabel);

impl StarlarkProvidersLabel {
    pub fn new(label: ProvidersLabel) -> Self {
        StarlarkProvidersLabel { label }
    }
}

#[starlark_value(type = "ProvidersLabel")]
impl<'v> StarlarkValue<'v> for StarlarkProvidersLabel
where
    Self: ProvidesStaticType<'v>,
{
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(label_methods)
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        if let Some(other) = StarlarkProvidersLabel::from_value(other) {
            Ok(self.label == other.label)
        } else {
            Ok(false)
        }
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.label.hash(hasher);
        Ok(())
    }
}

#[starlark_module]
fn label_methods(builder: &mut MethodsBuilder) {
    #[starlark(attribute)]
    fn name<'v>(this: &'v StarlarkProvidersLabel) -> starlark::Result<&'v str> {
        Ok(this.label.target().name().as_str())
    }

    #[starlark(attribute)]
    fn sub_target<'v>(this: &'v StarlarkProvidersLabel) -> starlark::Result<NoneOr<Vec<&'v str>>> {
        Ok(match this.label.name() {
            ProvidersName::Default => NoneOr::None,
            ProvidersName::NonDefault(flavor) => match flavor.as_ref() {
                NonDefaultProvidersName::Named(names) => {
                    NoneOr::Other(names.iter().map(|p| p.as_str()).collect())
                }
                NonDefaultProvidersName::UnrecognizedFlavor(_) => {
                    unreachable!(
                        "This should have been an error when looking up the corresponding analysis (`{}`)",
                        this.label
                    )
                }
            },
        })
    }

    #[starlark(attribute)]
    fn path<'v>(this: &StarlarkProvidersLabel) -> starlark::Result<StarlarkCellPath> {
        Ok(StarlarkCellPath(this.label.target().pkg().to_cell_path()))
    }

    #[starlark(attribute)]
    fn cell<'v>(this: &'v StarlarkProvidersLabel) -> starlark::Result<&'v str> {
        let cell = this.label.target().pkg().cell_name().as_str();
        Ok(cell)
    }

    #[starlark(attribute)]
    fn package<'v>(
        this: &'v StarlarkProvidersLabel,
        heap: Heap<'v>,
    ) -> starlark::Result<StringValue<'v>> {
        Ok(heap.alloc_str_intern(this.label.target().pkg().cell_relative_path().as_str()))
    }

    /// Returns the PackagePath for this providers label.
    #[starlark(attribute)]
    fn package_path<'v>(this: &StarlarkProvidersLabel) -> starlark::Result<StarlarkPackagePath> {
        Ok(StarlarkPackagePath::new(this.label.target().pkg().dupe()))
    }

    /// Returns the unconfigured underlying target label.
    fn raw_target(this: &StarlarkProvidersLabel) -> starlark::Result<StarlarkTargetLabel> {
        Ok(StarlarkTargetLabel::new((*this.label.target()).dupe()))
    }
}

#[starlark_module]
pub fn register_providers_label(globals: &mut GlobalsBuilder) {
    // TODO(nga): remove this alias.
    const Label: StarlarkValueAsType<StarlarkConfiguredProvidersLabel> = StarlarkValueAsType::new();
    const ProvidersLabel: StarlarkValueAsType<StarlarkProvidersLabel> = StarlarkValueAsType::new();
    const ConfiguredProvidersLabel: StarlarkValueAsType<StarlarkConfiguredProvidersLabel> =
        StarlarkValueAsType::new();
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use slug_core::cells::BzlmodRuntimeCellInstallSnapshot;
    use slug_core::cells::BzlmodRuntimeDynamicAlias;
    use slug_core::cells::CellAliasResolver;
    use slug_core::cells::name::CellName;
    use slug_core::configuration::data::ConfigurationData;
    use slug_core::provider::label::ConfiguredProvidersLabel;
    use slug_core::provider::label::NonDefaultProvidersName;
    use slug_core::provider::label::ProviderName;
    use slug_core::provider::label::ProvidersLabel;
    use slug_core::provider::label::ProvidersName;
    use slug_core::target::configured_target_label::ConfiguredTargetLabel;
    use slug_core::target::label::label::TargetLabel;
    use slug_util::arc_str::ArcSlice;
    use starlark::assert::Assert;
    use starlark::environment::GlobalsBuilder;
    use starlark::starlark_module;

    use crate::types::configured_providers_label::StarlarkConfiguredProvidersLabel;
    use crate::types::configured_providers_label::StarlarkProvidersLabel;

    #[starlark_module]
    fn register_test_providers_label(globals: &mut GlobalsBuilder) {
        fn configured_providers_label() -> starlark::Result<StarlarkConfiguredProvidersLabel> {
            Ok(StarlarkConfiguredProvidersLabel::new(
                ConfiguredProvidersLabel::new(
                    ConfiguredTargetLabel::testing_parse(
                        "foo//bar:baz",
                        ConfigurationData::testing_new(),
                    ),
                    ProvidersName::NonDefault(triomphe::Arc::new(NonDefaultProvidersName::Named(
                        ArcSlice::new([
                            ProviderName::new("qux".to_owned())?,
                            ProviderName::new("quux".to_owned())?,
                        ]),
                    ))),
                ),
            ))
        }

        fn bzlmod_module_label() -> starlark::Result<StarlarkConfiguredProvidersLabel> {
            Ok(StarlarkConfiguredProvidersLabel::new(
                ConfiguredProvidersLabel::new(
                    ConfiguredTargetLabel::testing_parse(
                        "llvm//runtimes/libunwind:unwind",
                        ConfigurationData::testing_new(),
                    ),
                    ProvidersName::Default,
                ),
            ))
        }

        fn bzlmod_runtime_alias_label() -> starlark::Result<StarlarkConfiguredProvidersLabel> {
            let apparent = "plan61_configured_label_runtime_alias_derived";
            let canonical = "plan61_owner++configured_label+derived";
            let wrong_global = "plan61_wrong_owner++configured_label+derived";
            slug_core::cells::register_test_dynamic_extension_cell_alias(
                apparent.to_owned(),
                wrong_global.to_owned(),
            );
            let snapshot = BzlmodRuntimeCellInstallSnapshot {
                root_module_name: None,
                extension_cells: Vec::new(),
                scoped_aliases: Vec::new(),
                dynamic_aliases: vec![BzlmodRuntimeDynamicAlias {
                    apparent_name: apparent.to_owned(),
                    canonical_name: canonical.to_owned(),
                }],
            };
            let resolver = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
                CellName::testing_new("root"),
                HashMap::new(),
                &snapshot,
            )
            .unwrap();
            Ok(
                StarlarkConfiguredProvidersLabel::new_with_cell_alias_resolver(
                    ConfiguredProvidersLabel::new(
                        ConfiguredTargetLabel::testing_parse(
                            &format!("{apparent}//pkg:target"),
                            ConfigurationData::testing_new(),
                        ),
                        ProvidersName::Default,
                    ),
                    Some(resolver),
                ),
            )
        }

        fn providers_label() -> starlark::Result<StarlarkProvidersLabel> {
            Ok(StarlarkProvidersLabel {
                label: ProvidersLabel::new(
                    TargetLabel::testing_parse("foo//bar:baz"),
                    ProvidersName::NonDefault(triomphe::Arc::new(NonDefaultProvidersName::Named(
                        ArcSlice::new([
                            ProviderName::new("qux".to_owned())?,
                            ProviderName::new("quux".to_owned())?,
                        ]),
                    ))),
                ),
            })
        }
    }

    #[test]
    fn test_configured_providers_label_to_json() {
        let mut a = Assert::new();
        a.globals_add(register_test_providers_label);
        a.eq(
            &"'\"foo//bar:baz[qux][quux] (<CFG>)\"'"
                .replace("<CFG>", &ConfigurationData::testing_new().to_string()),
            "json.encode(configured_providers_label())",
        );
    }

    #[test]
    fn test_providers_label_to_json() {
        let mut a = Assert::new();
        a.globals_add(register_test_providers_label);
        a.eq(
            "'\"foo//bar:baz[qux][quux]\"'",
            "json.encode(providers_label())",
        );
    }

    #[test]
    fn configured_label_without_alias_owner_uses_apparent_workspace() {
        let mut a = Assert::new();
        a.globals_add(register_test_providers_label);
        a.eq("\"llvm\"", "bzlmod_module_label().workspace_name");
        a.eq("\"llvm\"", "bzlmod_module_label().repo_name");
        a.eq("\"external/llvm\"", "bzlmod_module_label().workspace_root");
    }

    #[test]
    fn configured_label_workspace_names_prefer_runtime_aliases_before_globals()
    -> slug_error::Result<()> {
        let apparent = "plan61_configured_label_runtime_alias";
        let canonical = "plan61_owner++configured_label+generated";
        let wrong_global = "plan61_wrong_owner++configured_label+generated";
        let snapshot = BzlmodRuntimeCellInstallSnapshot {
            root_module_name: None,
            extension_cells: Vec::new(),
            scoped_aliases: Vec::new(),
            dynamic_aliases: vec![BzlmodRuntimeDynamicAlias {
                apparent_name: apparent.to_owned(),
                canonical_name: canonical.to_owned(),
            }],
        };
        slug_core::cells::register_test_dynamic_extension_cell_alias(
            apparent.to_owned(),
            wrong_global.to_owned(),
        );
        let resolver = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            CellName::testing_new("root"),
            HashMap::new(),
            &snapshot,
        )?;
        let label = ConfiguredProvidersLabel::new(
            ConfiguredTargetLabel::testing_parse(
                &format!("{apparent}//pkg:target"),
                ConfigurationData::testing_new(),
            ),
            ProvidersName::Default,
        );

        let starlark_label =
            StarlarkConfiguredProvidersLabel::new_with_cell_alias_resolver(label, Some(resolver));

        assert_eq!(starlark_label.bazel_workspace_name(), canonical);
        assert_eq!(
            slug_core::cells::resolve_test_dynamic_extension_cell_alias(apparent).as_deref(),
            Some(wrong_global)
        );
        Ok(())
    }

    #[test]
    fn configured_label_workspace_names_use_explicit_root_before_global_root()
    -> slug_error::Result<()> {
        let apparent = "root";
        let canonical = "plan61_owner++configured_label+root";
        let snapshot = BzlmodRuntimeCellInstallSnapshot {
            root_module_name: None,
            extension_cells: Vec::new(),
            scoped_aliases: Vec::new(),
            dynamic_aliases: vec![BzlmodRuntimeDynamicAlias {
                apparent_name: apparent.to_owned(),
                canonical_name: canonical.to_owned(),
            }],
        };
        let resolver = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            CellName::testing_new("workspace"),
            HashMap::new(),
            &snapshot,
        )?;
        let label = ConfiguredProvidersLabel::new(
            ConfiguredTargetLabel::testing_parse(
                &format!("{apparent}//pkg:target"),
                ConfigurationData::testing_new(),
            ),
            ProvidersName::Default,
        );

        let starlark_label =
            StarlarkConfiguredProvidersLabel::new_with_cell_alias_resolver_and_root(
                label.clone(),
                Some(resolver),
                Some(CellName::testing_new("workspace")),
            );
        let no_root_label =
            StarlarkConfiguredProvidersLabel::new_with_cell_alias_resolver(label, None);

        assert_eq!(starlark_label.bazel_workspace_name(), canonical);
        assert_eq!(no_root_label.bazel_workspace_name(), "");
        Ok(())
    }

    #[test]
    fn configured_label_workspace_names_runtime_miss_ignores_global_alias() -> slug_error::Result<()>
    {
        let apparent = "plan61_configured_label_runtime_miss";
        let wrong_global = "plan61_wrong_owner++configured_label+unowned";
        slug_core::cells::register_test_dynamic_extension_cell_alias(
            apparent.to_owned(),
            wrong_global.to_owned(),
        );
        let resolver = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            CellName::testing_new("root"),
            HashMap::new(),
            &BzlmodRuntimeCellInstallSnapshot::default(),
        )?;
        let label = ConfiguredProvidersLabel::new(
            ConfiguredTargetLabel::testing_parse(
                &format!("{apparent}//pkg:target"),
                ConfigurationData::testing_new(),
            ),
            ProvidersName::Default,
        );
        let no_owner_label = StarlarkConfiguredProvidersLabel::new(label.clone());
        let starlark_label =
            StarlarkConfiguredProvidersLabel::new_with_cell_alias_resolver(label, Some(resolver));

        assert_eq!(starlark_label.bazel_workspace_name(), apparent);
        assert_eq!(no_owner_label.bazel_workspace_name(), apparent);
        assert_eq!(
            slug_core::cells::resolve_test_dynamic_extension_cell_alias(apparent).as_deref(),
            Some(wrong_global)
        );
        Ok(())
    }

    #[test]
    fn configured_label_no_snapshot_resolver_miss_ignores_global_alias() -> slug_error::Result<()> {
        let apparent = "plan61_configured_label_no_snapshot_miss";
        let wrong_global = "plan61_wrong_owner++configured_label+no_snapshot";
        slug_core::cells::register_test_dynamic_extension_cell_alias(
            apparent.to_owned(),
            wrong_global.to_owned(),
        );
        let resolver = CellAliasResolver::new(CellName::testing_new("root"), HashMap::new())?;
        let label = ConfiguredProvidersLabel::new(
            ConfiguredTargetLabel::testing_parse(
                &format!("{apparent}//pkg:target"),
                ConfigurationData::testing_new(),
            ),
            ProvidersName::Default,
        );
        let starlark_label =
            StarlarkConfiguredProvidersLabel::new_with_cell_alias_resolver(label, Some(resolver));

        assert_eq!(starlark_label.bazel_workspace_name(), apparent);
        assert_eq!(
            slug_core::cells::resolve_test_dynamic_extension_cell_alias(apparent).as_deref(),
            Some(wrong_global)
        );
        Ok(())
    }

    #[test]
    fn configured_label_derived_labels_keep_runtime_alias_resolver() {
        let mut a = Assert::new();
        a.globals_add(register_test_providers_label);
        a.eq(
            "\"plan61_owner++configured_label+derived\"",
            "bzlmod_runtime_alias_label().relative(':other').workspace_name",
        );
        a.eq(
            "\"external/plan61_owner++configured_label+derived\"",
            "bzlmod_runtime_alias_label().same_package_label('other').workspace_root",
        );
    }
}
