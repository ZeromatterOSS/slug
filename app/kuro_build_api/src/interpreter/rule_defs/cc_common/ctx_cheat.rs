/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! CtxCheat* stubs — lightweight `ctx` / `ctx.actions` / `ctx.label` stand-ins
//! used when rules_cc code runs outside of a real rule analysis context.

use std::fmt;
use std::fmt::Display;
use std::hash::Hash;

use allocative::Allocative;
use starlark::collections::StarlarkHasher;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;

use crate::interpreter::rule_defs::fragments::ConfigurationFragments;

// ============================================================================
// CtxCheatStub - Stub for actions2ctx_cheat return value
// ============================================================================

/// A stub context returned by actions2ctx_cheat (used when no real actions available).
///
/// This provides the minimum attributes needed by rules_cc's compile function.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxCheatStub;

impl Display for CtxCheatStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<ctx_cheat_stub>")
    }
}

starlark_simple_value!(CtxCheatStub);

#[starlark_value(type = "ctx_cheat_stub")]
impl<'v> StarlarkValue<'v> for CtxCheatStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "label"
                | "bin_dir"
                | "genfiles_dir"
                | "configuration"
                | "actions"
                | "fragments"
                | "workspace_name"
                | "exec_groups"
                | "toolchains"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "label" => Some(heap.alloc(CtxCheatLabelStub)),
            "bin_dir" => {
                let m = crate::interpreter::rule_defs::build_config::get_compilation_mode();
                Some(heap.alloc(CtxCheatDirStub {
                    path: format!(
                        "bazel-out/{}-{}/bin",
                        crate::interpreter::rule_defs::context::host_target_cpu(),
                        m
                    ),
                }))
            }
            "genfiles_dir" => {
                let m = crate::interpreter::rule_defs::build_config::get_compilation_mode();
                Some(heap.alloc(CtxCheatDirStub {
                    path: format!(
                        "bazel-out/{}-{}/genfiles",
                        crate::interpreter::rule_defs::context::host_target_cpu(),
                        m
                    ),
                }))
            }
            "configuration" => Some(heap.alloc(CtxCheatConfigStub)),
            "actions" => Some(heap.alloc(CtxCheatActionsStub)),
            "fragments" => {
                let mode = crate::interpreter::rule_defs::build_config::get_compilation_mode();
                let cpp = crate::interpreter::rule_defs::fragments::CppFragment::new(
                    mode, false, false, false,
                );
                Some(heap.alloc(ConfigurationFragments::new(cpp)))
            }
            "workspace_name" => Some(heap.alloc_str("").to_value()),
            "exec_groups" => Some(heap.alloc(
                crate::interpreter::rule_defs::context::ResolvedExecGroups {
                    groups: std::collections::HashMap::new(),
                    valid_names: Vec::new(),
                },
            )),
            "toolchains" => Some(heap.alloc(
                crate::interpreter::rule_defs::context::ResolvedToolchains {
                    toolchains: std::collections::HashMap::new(),
                    exec_platform: String::new(),
                },
            )),
            _ => None,
        }
    }
}

/// A context wrapper returned by actions2ctx_cheat that preserves the real actions.
///
/// This wraps the real AnalysisActions so that create_cc_compile_action can
/// use them to register actual compile actions.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
pub struct CtxCheatWithActions<'v> {
    /// The real actions object (AnalysisActions)
    pub(crate) actions: Value<'v>,
    /// Target cell name (e.g., "protobuf")
    #[allocative(skip)]
    pub(crate) cell_name: String,
    /// Package path (e.g., "third_party/utf8_range")
    #[allocative(skip)]
    pub(crate) pkg_path: String,
    /// Target name (e.g., "utf8_validity")
    #[allocative(skip)]
    pub(crate) target_name: String,
    /// Configuration hash of the owning target (empty when unknown).
    #[allocative(skip)]
    pub(crate) cfg_hash: String,
}

impl<'v> Display for CtxCheatWithActions<'v> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<ctx_cheat_with_actions>")
    }
}

#[starlark_value(type = "ctx_cheat_stub")]
impl<'v> StarlarkValue<'v> for CtxCheatWithActions<'v> {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "label"
                | "bin_dir"
                | "genfiles_dir"
                | "configuration"
                | "actions"
                | "fragments"
                | "workspace_name"
                | "exec_groups"
                | "toolchains"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "label" => Some(heap.alloc(CtxCheatLabelDynamic {
                name: self.target_name.clone(),
                package: self.pkg_path.clone(),
                workspace_name: self.cell_name.clone(),
            })),
            "bin_dir" => {
                let path = if !self.cell_name.is_empty() && !self.cfg_hash.is_empty() {
                    format!("buck-out/v2/gen/{}/{}", self.cell_name, self.cfg_hash)
                } else {
                    let m = crate::interpreter::rule_defs::build_config::get_compilation_mode();
                    format!(
                        "bazel-out/{}-{}/bin",
                        crate::interpreter::rule_defs::context::host_target_cpu(),
                        m
                    )
                };
                Some(heap.alloc(CtxCheatDirStub { path }))
            }
            "genfiles_dir" => {
                let path = if !self.cell_name.is_empty() && !self.cfg_hash.is_empty() {
                    format!("buck-out/v2/gen/{}/{}", self.cell_name, self.cfg_hash)
                } else {
                    let m = crate::interpreter::rule_defs::build_config::get_compilation_mode();
                    format!(
                        "bazel-out/{}-{}/genfiles",
                        crate::interpreter::rule_defs::context::host_target_cpu(),
                        m
                    )
                };
                Some(heap.alloc(CtxCheatDirStub { path }))
            }
            "configuration" => Some(heap.alloc(CtxCheatConfigStub)),
            // Return the REAL actions object here
            "actions" => Some(self.actions),
            "fragments" => {
                let mode = crate::interpreter::rule_defs::build_config::get_compilation_mode();
                let cpp = crate::interpreter::rule_defs::fragments::CppFragment::new(
                    mode, false, false, false,
                );
                Some(heap.alloc(ConfigurationFragments::new(cpp)))
            }
            "workspace_name" => Some(heap.alloc_str("").to_value()),
            "exec_groups" => Some(heap.alloc(
                crate::interpreter::rule_defs::context::ResolvedExecGroups {
                    groups: std::collections::HashMap::new(),
                    valid_names: Vec::new(),
                },
            )),
            "toolchains" => Some(heap.alloc(
                crate::interpreter::rule_defs::context::ResolvedToolchains {
                    toolchains: std::collections::HashMap::new(),
                    exec_platform: String::new(),
                },
            )),
            _ => None,
        }
    }
}

impl<'v> starlark::values::AllocValue<'v> for CtxCheatWithActions<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex_no_freeze(self)
    }
}

/// A stub for ctx.actions.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxCheatActionsStub;

impl Display for CtxCheatActionsStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<actions>")
    }
}

starlark_simple_value!(CtxCheatActionsStub);

#[starlark_value(type = "actions")]
impl<'v> StarlarkValue<'v> for CtxCheatActionsStub {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(ctx_cheat_actions_stub_methods)
    }
}

#[starlark_module]
fn ctx_cheat_actions_stub_methods(builder: &mut MethodsBuilder) {
    /// Declares a file in the output tree.
    #[allow(unused_variables)]
    fn declare_file<'v>(
        this: &CtxCheatActionsStub,
        #[starlark(require = pos)] filename: &str,
        #[starlark(require = named, default = NoneType)] sibling: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Return a stub artifact
        Ok(heap.alloc(CtxCheatArtifactStub {
            path: filename.to_owned(),
        }))
    }

    /// Declares a directory in the output tree.
    #[allow(unused_variables)]
    fn declare_directory<'v>(
        this: &CtxCheatActionsStub,
        #[starlark(require = pos)] filename: &str,
        #[starlark(require = named, default = NoneType)] sibling: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(CtxCheatArtifactStub {
            path: filename.to_owned(),
        }))
    }

    /// Runs an action (stub implementation).
    #[allow(unused_variables)]
    fn run<'v>(
        this: &CtxCheatActionsStub,
        #[starlark(require = named, default = NoneType)] mnemonic: Value<'v>,
        #[starlark(require = named, default = NoneType)] executable: Value<'v>,
        #[starlark(require = named, default = NoneType)] arguments: Value<'v>,
        #[starlark(require = named, default = NoneType)] inputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] outputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] progress_message: Value<'v>,
        #[starlark(require = named, default = NoneType)] resource_set: Value<'v>,
        #[starlark(require = named, default = NoneType)] env: Value<'v>,
        #[starlark(require = named, default = false)] use_default_shell_env: bool,
        #[starlark(require = named, default = NoneType)] execution_requirements: Value<'v>,
        #[starlark(require = named, default = NoneType)] toolchain: Value<'v>,
        #[starlark(require = named, default = NoneType)] exec_group: Value<'v>,
        #[starlark(require = named, default = NoneType)] tools: Value<'v>,
        #[starlark(require = named, default = NoneType)] input_manifests: Value<'v>,
        #[starlark(require = named, default = NoneType)] unused_inputs_list: Value<'v>,
        #[starlark(require = named, default = NoneType)] shadowed_action: Value<'v>,
    ) -> starlark::Result<NoneType> {
        // Stub: do nothing - just accept the parameters
        Ok(NoneType)
    }
}

/// A stub for artifact root (Bazel compatibility).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxCheatArtifactRootStub {
    path: String,
}

impl Display for CtxCheatArtifactRootStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<root {}>", self.path)
    }
}

starlark_simple_value!(CtxCheatArtifactRootStub);

#[starlark_value(type = "root")]
impl<'v> StarlarkValue<'v> for CtxCheatArtifactRootStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        attribute == "path"
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "path" => Some(heap.alloc_str(&self.path).to_value()),
            _ => None,
        }
    }
}

/// A stub for artifact from ctx.actions.declare_file.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxCheatArtifactStub {
    pub(crate) path: String,
}

impl Display for CtxCheatArtifactStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<artifact {}>", self.path)
    }
}

starlark_simple_value!(CtxCheatArtifactStub);

#[starlark_value(type = "File")]
impl<'v> StarlarkValue<'v> for CtxCheatArtifactStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "path"
                | "short_path"
                | "basename"
                | "extension"
                | "is_source"
                | "root"
                | "is_directory"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "path" => Some(heap.alloc_str(&self.path).to_value()),
            "short_path" => Some(heap.alloc_str(&self.path).to_value()),
            "basename" => {
                let basename = self.path.rsplit('/').next().unwrap_or(&self.path);
                Some(heap.alloc_str(basename).to_value())
            }
            "extension" => {
                let ext = self.path.rsplit('.').next().unwrap_or("");
                Some(heap.alloc_str(ext).to_value())
            }
            "is_source" => Some(Value::new_bool(false)),
            "is_directory" => Some(Value::new_bool(false)),
            "root" => {
                let m = crate::interpreter::rule_defs::build_config::get_compilation_mode();
                Some(heap.alloc(CtxCheatArtifactRootStub {
                    path: format!(
                        "bazel-out/{}-{}/bin",
                        crate::interpreter::rule_defs::context::host_target_cpu(),
                        m
                    ),
                }))
            }
            _ => None,
        }
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        match CtxCheatArtifactStub::from_value(other) {
            Some(other) => Ok(self.path == other.path),
            None => Ok(false),
        }
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.path.hash(hasher);
        Ok(())
    }
}

/// A stub for ctx.configuration.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxCheatConfigStub;

impl Display for CtxCheatConfigStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<configuration>")
    }
}

starlark_simple_value!(CtxCheatConfigStub);

#[starlark_value(type = "configuration")]
impl<'v> StarlarkValue<'v> for CtxCheatConfigStub {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(ctx_cheat_config_stub_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "coverage_enabled")
    }

    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "coverage_enabled" => Some(Value::new_bool(false)),
            _ => None,
        }
    }
}

#[starlark_module]
fn ctx_cheat_config_stub_methods(builder: &mut MethodsBuilder) {
    /// Returns whether sibling repository layout is used.
    fn is_sibling_repository_layout(this: &CtxCheatConfigStub) -> starlark::Result<bool> {
        let _ = this;
        Ok(false)
    }
}

/// A stub for directory paths (bin_dir, genfiles_dir).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxCheatDirStub {
    path: String,
}

impl Display for CtxCheatDirStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<dir {}>", self.path)
    }
}

starlark_simple_value!(CtxCheatDirStub);

#[starlark_value(type = "root")]
impl<'v> StarlarkValue<'v> for CtxCheatDirStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "path")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "path" => Some(heap.alloc_str(&self.path).to_value()),
            _ => None,
        }
    }
}

/// A stub label for the ctx_cheat_stub.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxCheatLabelStub;

impl Display for CtxCheatLabelStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "//stub:stub")
    }
}

starlark_simple_value!(CtxCheatLabelStub);

#[starlark_value(type = "Label")]
impl<'v> StarlarkValue<'v> for CtxCheatLabelStub {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(ctx_cheat_label_stub_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "name" | "package" | "workspace_name")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "name" => Some(heap.alloc_str("stub").to_value()),
            "package" => Some(heap.alloc_str("stub").to_value()),
            "workspace_name" => Some(heap.alloc_str("").to_value()),
            _ => None,
        }
    }
}

#[starlark_module]
fn ctx_cheat_label_stub_methods(builder: &mut MethodsBuilder) {
    /// Returns a label with the same package but a different name.
    fn same_package_label<'v>(
        this: &CtxCheatLabelStub,
        #[starlark(require = pos)] _name: &str,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        // Return a new label stub with the given name
        Ok(heap.alloc(CtxCheatLabelStub))
    }
}

/// A dynamic label with real target info for the ctx_cheat.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxCheatLabelDynamic {
    name: String,
    package: String,
    workspace_name: String,
}

impl Display for CtxCheatLabelDynamic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.workspace_name.is_empty() {
            write!(f, "//{}:{}", self.package, self.name)
        } else {
            write!(
                f,
                "@{}//{}:{}",
                self.workspace_name, self.package, self.name
            )
        }
    }
}

starlark_simple_value!(CtxCheatLabelDynamic);

#[starlark_value(type = "Label")]
impl<'v> StarlarkValue<'v> for CtxCheatLabelDynamic {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(ctx_cheat_label_dynamic_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "name" | "package" | "workspace_name")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "name" => Some(heap.alloc_str(&self.name).to_value()),
            "package" => Some(heap.alloc_str(&self.package).to_value()),
            "workspace_name" => Some(heap.alloc_str(&self.workspace_name).to_value()),
            _ => None,
        }
    }
}

#[starlark_module]
fn ctx_cheat_label_dynamic_methods(builder: &mut MethodsBuilder) {
    /// Returns a label with the same package but a different name.
    fn same_package_label<'v>(
        this: &CtxCheatLabelDynamic,
        #[starlark(require = pos)] name: &str,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(CtxCheatLabelDynamic {
            name: name.to_owned(),
            package: this.package.clone(),
            workspace_name: this.workspace_name.clone(),
        }))
    }
}
