/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! CcCommonInternal + CcCommonModule — the compile/link action construction
//! entry points called by rules_cc Starlark code. These two big starlark
//! method blocks live here because they are deeply interdependent.

use std::fmt;
use std::fmt::Display;
use std::hash::Hash;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use allocative::Allocative;
use starlark::coerce::Coerce;
use starlark::collections::SmallMap;
use starlark::collections::StarlarkHasher;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::Freeze;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::ValueError;
use starlark::values::ValueLifetimeless;
use starlark::values::ValueLike;
use starlark::values::dict::AllocHashableDict;
use starlark::values::dict::Dict;
use starlark::values::dict::DictRef;
use starlark::values::list::AllocList;
use starlark::values::list::ListRef;
use starlark::values::none::NoneOr;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;
use starlark::values::tuple::TupleRef;

use crate::interpreter::rule_defs::artifact::starlark_artifact::StarlarkArtifact;
use crate::interpreter::rule_defs::artifact::starlark_artifact_like::StarlarkInputArtifactLike;
use crate::interpreter::rule_defs::artifact::starlark_declared_artifact::StarlarkDeclaredArtifact;
use crate::interpreter::rule_defs::cc_common::ctx_cheat::CtxCheatArtifactStub;
use crate::interpreter::rule_defs::cc_common::ctx_cheat::CtxCheatWithActions;
use crate::interpreter::rule_defs::cc_common::feature_config::CcEnvEntry;
use crate::interpreter::rule_defs::cc_common::feature_config::CcEnvSet;
use crate::interpreter::rule_defs::cc_common::feature_config::CcExpandIfEqual;
use crate::interpreter::rule_defs::cc_common::feature_config::CcFeatureEnvSets;
use crate::interpreter::rule_defs::cc_common::feature_config::CcFeatureFlagSets;
use crate::interpreter::rule_defs::cc_common::feature_config::CcFlagGroup;
use crate::interpreter::rule_defs::cc_common::feature_config::CcFlagSet;
use crate::interpreter::rule_defs::cc_common::feature_config::CcToolchainFeatures;
use crate::interpreter::rule_defs::cc_common::feature_config::CcWithFeatureSet;
use crate::interpreter::rule_defs::cc_common::feature_config::FeatureConfiguration;
use crate::interpreter::rule_defs::cc_common::host::include_flag_for_context_attr;
use crate::interpreter::rule_defs::cc_common::host::include_flag_for_dir_impl;
use crate::interpreter::rule_defs::cc_common::host::is_msvc_compiler;
use crate::interpreter::rule_defs::cc_common::host::is_windows_host;
use crate::interpreter::rule_defs::cc_common::host::normalize_action_name;
use crate::interpreter::rule_defs::cc_common::host::normalize_external_cells_path;
use crate::interpreter::rule_defs::cc_common::host::resolve_windows_compiler;
use crate::interpreter::rule_defs::cc_common::msvc_detect::get_msvc_tool_paths;
use crate::interpreter::rule_defs::cc_common::providers::CcCompilationContext;
use crate::interpreter::rule_defs::cc_common::providers::CcCompilationContextGen;
use crate::interpreter::rule_defs::cc_common::providers::CcDebugContext;
use crate::interpreter::rule_defs::cc_common::providers::CcInfoInstanceGen;
use crate::interpreter::rule_defs::cc_common::providers::CcLinkingOutputs;
use crate::interpreter::rule_defs::cc_common::providers::CcLinkingOutputsGen;
use crate::interpreter::rule_defs::cc_common::providers::CcToolchainConfigInfoInstanceGen;
use crate::interpreter::rule_defs::cc_common::providers::CcToolchainInfoProvider;
use crate::interpreter::rule_defs::cc_common::providers::CcToolchainVariables;
use crate::interpreter::rule_defs::cc_common::providers::CcToolchainVariablesGen;
use crate::interpreter::rule_defs::cc_common::providers::CompilationOutputs;
use crate::interpreter::rule_defs::cc_common::providers::CompilationOutputsGen;
use crate::interpreter::rule_defs::cc_common::providers::ExecutionInfoProvider;
use crate::interpreter::rule_defs::cc_common::providers::HeaderInfoStub;
use crate::interpreter::rule_defs::cc_common::providers::LibraryToLinkGen;
use crate::interpreter::rule_defs::cc_common::providers::LinkerInputStubGen;
use crate::interpreter::rule_defs::cc_common::providers::LinkingContextWithInputsGen;
use crate::interpreter::rule_defs::depset::depset_summary;
use crate::interpreter::rule_defs::depset::depset_to_artifact_inputs;
use crate::interpreter::rule_defs::depset::depset_to_list;
use crate::interpreter::rule_defs::depset::is_depset_value;

// ============================================================================
// CcCommonInternal - Internal API returned by internal_DO_NOT_USE()
// ============================================================================

static CC_INTERNAL_FREEZE_COUNT: AtomicUsize = AtomicUsize::new(0);

const CC_INTERNAL_FREEZE_CHECKPOINT_LARGE_LEN: usize = 256;

/// Helper: insert `(key, val)` into `map`, keyed by a freshly allocated
/// Starlark string, iff `val` isn't `None`. Used by `create_compile_variables`
/// and `create_link_variables` to build variable dicts without 10-line
/// `if !x.is_none() { map.insert_hashed(heap.alloc_str(...)...) }` blocks.
/// Derives `compilation_mode` from a FeatureConfiguration. rules_cc's
/// `configure_features.bzl` adds `cpp_configuration.compilation_mode()` (the
/// per-cfg value, from Plan 19.5) as a requested feature, so the active mode
/// is detectable by checking which of "opt" / "dbg" / "fastbuild" is
/// enabled. Falls back to the process-global `BUILD_CONFIG` entry for
/// contexts that never plumb through `configure_features` (tests, stubbed
/// toolchains).
fn compilation_mode_from_features(feature_configuration: Value<'_>) -> String {
    if let Some(fc) = feature_configuration.downcast_ref::<FeatureConfiguration>() {
        for mode in ["opt", "dbg", "fastbuild"] {
            if fc.is_feature_enabled(mode) {
                return mode.to_owned();
            }
        }
    }
    crate::interpreter::rule_defs::build_config::get_compilation_mode()
}

fn action_category_from_bazel_action_name(action_name: &str) -> String {
    normalize_action_name(action_name)
        .replace("c++", "cpp")
        .replace('-', "_")
}

fn is_header_parsing_action(action_name: &str) -> bool {
    let short_name = action_name
        .rsplit_once(':')
        .map(|(_, name)| name)
        .unwrap_or(action_name);
    let normalized = normalize_action_name(short_name).replace('_', "-");
    normalized == "c++-header-parsing"
}

fn host_llvm_toolchain_bin(tool: &str) -> Option<String> {
    let root = slug_core::cells::get_dynamic_project_root()?;
    let os = match std::env::consts::OS {
        "linux" => "linux",
        "macos" => "darwin",
        _ => return None,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        _ => return None,
    };
    let suffix = format!("-{os}-{arch}");
    let external = root.join("bazel-external");
    let mut candidates = Vec::new();
    for entry in std::fs::read_dir(&external).ok()?.flatten() {
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        let is_llvm_toolchain = name.starts_with("llvm-toolchain-minimal-")
            || name.starts_with("llvm+http_archive+llvm-toolchain-minimal-");
        if !is_llvm_toolchain || !name.ends_with(&suffix) {
            continue;
        }
        let path = entry.path().join("bin").join(tool);
        if path.is_file() {
            candidates.push(path.to_string_lossy().into_owned());
        }
    }
    candidates.sort();
    candidates.into_iter().next()
}

fn has_host_llvm_toolchain() -> bool {
    host_llvm_toolchain_bin("clang++").is_some()
}

fn is_musl_cc_toolchain_target(
    target_system_name: Option<&str>,
    target_libc: Option<&str>,
) -> bool {
    target_system_name.is_some_and(|s| s.contains("musl"))
        || target_libc.is_some_and(|s| s.contains("musl"))
}

fn is_compiler_rt_crtbegin_link_output(output_path: Option<&str>) -> bool {
    output_path.is_some_and(|path| {
        path.contains("compiler-rt/libclang_rt.crtbegin.so")
            || path.ends_with("libclang_rt.crtbegin.so")
    })
}

fn cc_toolchain_attr_any<'v>(
    cc_toolchain: Value<'v>,
    heap: Heap<'v>,
    names: &[&str],
) -> Option<Value<'v>> {
    for name in names {
        if let Ok(Some(value)) = cc_toolchain.get_attr(name, heap)
            && !value.is_none()
        {
            return Some(value);
        }
    }
    None
}

fn cc_toolchain_target_system_name<'v>(
    cc_toolchain: Value<'v>,
    heap: Heap<'v>,
) -> Option<Value<'v>> {
    cc_toolchain_attr_any(
        cc_toolchain,
        heap,
        &["target_gnu_system_name", "target_system_name"],
    )
}

fn cc_toolchain_target_libc<'v>(cc_toolchain: Value<'v>, heap: Heap<'v>) -> Option<Value<'v>> {
    cc_toolchain_attr_any(cc_toolchain, heap, &["libc", "target_libc"])
}

fn first_external_dir(
    external: &std::path::Path,
    matches: impl Fn(&str) -> bool,
) -> Option<std::path::PathBuf> {
    let mut candidates = Vec::new();
    for entry in std::fs::read_dir(external).ok()?.flatten() {
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        if matches(name) {
            let path = entry.path();
            if path.is_dir() {
                candidates.push(path);
            }
        }
    }
    candidates.sort();
    candidates.into_iter().next()
}

fn push_existing_isystem(args: &mut Vec<String>, path: std::path::PathBuf) {
    if path.is_dir() {
        args.push("-isystem".to_owned());
        args.push(path.to_string_lossy().into_owned());
    }
}

fn push_clang_resource_include(args: &mut Vec<String>) {
    let Some(clang) = host_llvm_toolchain_bin("clang") else {
        return;
    };
    let clang = std::path::PathBuf::from(clang);
    let Some(toolchain_root) = clang.parent().and_then(|bin| bin.parent()) else {
        return;
    };
    let clang_lib = toolchain_root.join("lib").join("clang");
    let mut candidates = Vec::new();
    for entry in std::fs::read_dir(&clang_lib)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
    {
        let include = entry.path().join("include");
        if include.is_dir() {
            candidates.push(include);
        }
    }
    candidates.sort();
    if let Some(include) = candidates.into_iter().last() {
        args.push("-Xclang".to_owned());
        args.push("-internal-isystem".to_owned());
        args.push("-Xclang".to_owned());
        args.push(include.to_string_lossy().into_owned());
    }
}

fn copy_dir_contents(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    if !src.is_dir() {
        return Ok(());
    }
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_contents(&src_path, &dst_path)?;
        } else if src_path.is_file() {
            if let Some(parent) = dst_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn write_musl_alltypes_header(
    musl: &std::path::Path,
    out: &std::path::Path,
) -> std::io::Result<()> {
    let sed = musl.join("tools").join("mkalltypes.sed");
    let arch = musl
        .join("arch")
        .join("x86_64")
        .join("bits")
        .join("alltypes.h.in");
    let generic = musl.join("include").join("alltypes.h.in");
    let output = std::process::Command::new("sed")
        .arg("-f")
        .arg(sed)
        .arg(arch)
        .arg(generic)
        .output()?;
    if !output.status.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "failed to generate musl alltypes.h",
        ));
    }
    std::fs::write(out, output.stdout)
}

fn write_musl_syscall_header(musl: &std::path::Path, out: &std::path::Path) -> std::io::Result<()> {
    let input = musl
        .join("arch")
        .join("x86_64")
        .join("bits")
        .join("syscall.h.in");
    let source = std::fs::read_to_string(input)?;
    let mut generated = source.clone();
    for line in source.lines() {
        if line.contains("__NR_") {
            generated.push_str(&line.replace("__NR_", "SYS_"));
            generated.push('\n');
        }
    }
    std::fs::write(out, generated)
}

fn ensure_musl_generated_include_dir(musl: &std::path::Path) -> Option<std::path::PathBuf> {
    let out = musl.join("generated").join("x86_64").join("includes");
    let bits = out.join("bits");
    let alltypes = bits.join("alltypes.h");
    let syscall = bits.join("syscall.h");
    if alltypes.is_file() && syscall.is_file() {
        return Some(out);
    }

    let result = (|| -> std::io::Result<()> {
        copy_dir_contents(&musl.join("include"), &out)?;
        copy_dir_contents(&musl.join("arch").join("generic"), &out)?;
        copy_dir_contents(&musl.join("arch").join("x86_64"), &out)?;
        std::fs::create_dir_all(&bits)?;
        write_musl_alltypes_header(musl, &alltypes)?;
        write_musl_syscall_header(musl, &syscall)?;
        Ok(())
    })();

    match result {
        Ok(()) => Some(out),
        Err(_) => None,
    }
}

fn llvm_musl_compile_default_args() -> Vec<String> {
    let mut args = [
        "-target",
        "x86_64-linux-musl",
        "-no-canonical-prefixes",
        "-Wno-builtin-macro-redefined",
        "-D__DATE__=\"redacted\"",
        "-D__TIMESTAMP__=\"redacted\"",
        "-D__TIME__=\"redacted\"",
        "-ffile-compilation-dir=.",
        "-Xclang",
        "-fno-cxx-modules",
        "-Wno-module-import-in-extern-c",
        "-Werror=incomplete-umbrella",
        "-nostdlibinc",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();

    let Some(root) = slug_core::cells::get_dynamic_project_root() else {
        return args;
    };
    let external = root.join("bazel-external");

    if let Some(kernel_headers) = first_external_dir(&external, |name| {
        name.starts_with("llvm+kernel_headers+linux_kernel_headers_x86")
            || name.starts_with("llvm++kernel_headers+linux_kernel_headers_x86")
    }) {
        push_existing_isystem(&mut args, kernel_headers.join("include"));
    }

    if let Some(musl) = first_external_dir(&external, |name| {
        name == "llvm+musl+musl_libc" || name == "llvm++musl+musl_libc"
    }) {
        if let Some(include_dir) = ensure_musl_generated_include_dir(&musl) {
            push_existing_isystem(&mut args, include_dir);
        } else {
            push_existing_isystem(&mut args, musl.join("include"));
            push_existing_isystem(&mut args, musl.join("arch").join("x86_64"));
            push_existing_isystem(&mut args, musl.join("src").join("include"));
        }
    }

    if let Some(compiler_rt) = first_external_dir(&external, |name| {
        name == "llvm+llvm_source+compiler-rt" || name == "llvm++llvm_source+compiler-rt"
    }) {
        push_existing_isystem(&mut args, compiler_rt.join("include"));
    }

    push_clang_resource_include(&mut args);

    args.extend(
        [
            "-fstack-protector",
            "-Wall",
            "-Wthread-safety",
            "-Wself-assign",
            "-Wunused-but-set-parameter",
            "-Wno-free-nonheap-object",
            "-fcolor-diagnostics",
            "-fno-omit-frame-pointer",
        ]
        .into_iter()
        .map(str::to_owned),
    );

    args
}

fn depset_values<'v>(value: Value<'v>, heap: Heap<'v>) -> starlark::Result<Vec<Value<'v>>> {
    if value.is_none() {
        Ok(Vec::new())
    } else {
        depset_to_list(value, heap)
    }
}

fn depset_or_iterable_values<'v>(
    value: Value<'v>,
    heap: Heap<'v>,
) -> starlark::Result<Vec<Value<'v>>> {
    if value.is_none() {
        return Ok(Vec::new());
    }
    if is_depset_value(value) {
        return depset_to_list(value, heap);
    }
    let mut values = Vec::new();
    if let Ok(iter) = value.iterate(heap) {
        for item in iter {
            values.push(item);
        }
    }
    Ok(values)
}

#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    Trace,
    Coerce,
    Freeze
)]
#[repr(C)]
struct CcFrozenListGen<V: ValueLifetimeless> {
    items: Vec<V>,
}

starlark::starlark_complex_value!(CcFrozenList);

impl<V: ValueLifetimeless + Display> Display for CcFrozenListGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[")?;
        for (index, item) in self.items.iter().enumerate() {
            if index != 0 {
                f.write_str(", ")?;
            }
            write!(f, "{item}")?;
        }
        f.write_str("]")
    }
}

pub(crate) fn cc_frozen_list_items<'v>(value: Value<'v>) -> Option<Vec<Value<'v>>> {
    if let Some(list) = ListRef::from_value(value) {
        return Some(list.iter().collect());
    }
    CcFrozenList::from_value(value)
        .map(|list| list.items.iter().map(|item| item.to_value()).collect())
}

#[starlark::values::starlark_value(type = "list")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for CcFrozenListGen<V>
where
    Self: ProvidesStaticType<'v> + Display,
{
    fn collect_repr(&self, collector: &mut String) {
        collector.push('[');
        for (index, item) in self.items.iter().enumerate() {
            if index != 0 {
                collector.push_str(", ");
            }
            item.to_value().collect_repr(collector);
        }
        collector.push(']');
    }

    fn to_bool(&self) -> bool {
        !self.items.is_empty()
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        let Some(other_items) = cc_frozen_list_items(other) else {
            return Ok(false);
        };
        if self.items.len() != other_items.len() {
            return Ok(false);
        }
        for (left, right) in self.items.iter().zip(other_items) {
            if !left.to_value().equals(right)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn at(&self, index: Value<'v>, _heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let Some(index) = index.unpack_i32() else {
            return ValueError::unsupported_with(self, "[]", index);
        };
        let len = self.items.len() as i32;
        let index = if index < 0 { len + index } else { index };
        if index < 0 || index >= len {
            return Err(starlark::Error::new_other(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "list index out of range",
            )));
        }
        Ok(self.items[index as usize].to_value())
    }

    fn length(&self) -> starlark::Result<i32> {
        Ok(self.items.len() as i32)
    }

    fn iterate_collect(&self, _heap: Heap<'v>) -> starlark::Result<Vec<Value<'v>>> {
        Ok(self.items.iter().map(|item| item.to_value()).collect())
    }

    fn add(&self, rhs: Value<'v>, heap: Heap<'v>) -> Option<starlark::Result<Value<'v>>> {
        let rhs_items = cc_frozen_list_items(rhs)?;
        let mut items = self
            .items
            .iter()
            .map(|item| item.to_value())
            .collect::<Vec<_>>();
        items.extend(rhs_items);
        Some(Ok(heap.alloc(AllocList(items))))
    }

    fn radd(&self, lhs: Value<'v>, heap: Heap<'v>) -> Option<starlark::Result<Value<'v>>> {
        let mut items = cc_frozen_list_items(lhs)?;
        items.extend(self.items.iter().map(|item| item.to_value()));
        Some(Ok(heap.alloc(AllocList(items))))
    }

    fn is_in(&self, other: Value<'v>) -> starlark::Result<bool> {
        for item in &self.items {
            if item.to_value().equals(other)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        "list".hash(hasher);
        self.items.len().hash(hasher);
        for item in &self.items {
            item.to_value().write_hash(hasher)?;
        }
        Ok(())
    }
}

fn cc_internal_freeze_value<'v>(value: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
    if let Some(list) = ListRef::from_value(value) {
        return cc_internal_freeze_values(list.iter(), heap);
    }
    if let Some(tuple) = TupleRef::from_value(value) {
        return cc_internal_freeze_values(tuple.iter(), heap);
    }
    if let Some(dict) = DictRef::from_value(value) {
        let mut entries = Vec::new();
        for (key, value) in dict.iter() {
            let key = cc_internal_freeze_value(key, heap)?;
            let value = cc_internal_freeze_value(value, heap)?;
            entries.push((key, value));
        }
        return Ok(heap.alloc(AllocHashableDict(entries)));
    }
    Ok(value)
}

fn cc_internal_freeze_values<'v>(
    values: impl IntoIterator<Item = Value<'v>>,
    heap: Heap<'v>,
) -> starlark::Result<Value<'v>> {
    let mut frozen = Vec::new();
    for value in values {
        frozen.push(cc_internal_freeze_value(value, heap)?);
    }
    Ok(heap.alloc(CcFrozenListGen { items: frozen }))
}

fn cc_internal_freeze_depset_or_iterable<'v>(
    value: Value<'v>,
    heap: Heap<'v>,
) -> starlark::Result<Value<'v>> {
    if value.is_none() {
        return cc_internal_freeze_values(Vec::new(), heap);
    }
    if is_depset_value(value) {
        return cc_internal_freeze_values(depset_to_list(value, heap)?, heap);
    }
    cc_internal_freeze_value(value, heap)
}

fn cc_freeze_user_link_flags<'v>(
    user_link_flags: Value<'v>,
    heap: Heap<'v>,
) -> starlark::Result<Value<'v>> {
    if user_link_flags.is_none() {
        return cc_internal_freeze_values(Vec::new(), heap);
    }
    if is_depset_value(user_link_flags) {
        return cc_internal_freeze_values(depset_to_list(user_link_flags, heap)?, heap);
    }
    let Some(flags) = ListRef::from_value(user_link_flags) else {
        return cc_internal_freeze_value(user_link_flags, heap);
    };
    let mut options = Vec::new();
    for flag in flags.iter() {
        if flag.unpack_str().is_some() {
            options.push(flag);
        } else if let Some(nested) = ListRef::from_value(flag) {
            options.extend(nested.iter());
        } else {
            return Err(starlark::Error::new_other(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Elements of list in user_link_flags must be either Strings or lists.",
            )));
        }
    }
    cc_internal_freeze_values(options, heap)
}

fn cc_value_path<'v>(value: Value<'v>, heap: Heap<'v>) -> Option<String> {
    if let Some(path) = value.unpack_str() {
        return Some(path.to_owned());
    }
    if let Some(artifact) = value.downcast_ref::<StarlarkArtifact>()
        && let Ok(bound) = artifact.get_bound_starlark_artifact()
    {
        return Some(
            bound
                .artifact()
                .get_path()
                .with_full_path(|p| p.as_str().to_owned()),
        );
    }
    if let Some(artifact) = value.downcast_ref::<StarlarkDeclaredArtifact<'v>>()
        && let Ok(bound) = artifact.get_bound_starlark_artifact()
    {
        return Some(
            bound
                .artifact()
                .get_path()
                .with_full_path(|p| p.as_str().to_owned()),
        );
    }
    value
        .get_attr("path", heap)
        .ok()
        .flatten()
        .and_then(|path| path.unpack_str().map(str::to_owned))
}

fn cc_location_matches_path(label: &str, path: &str) -> bool {
    let query_name = label
        .trim_start_matches(':')
        .rsplit(':')
        .next()
        .unwrap_or(label)
        .rsplit('/')
        .next()
        .unwrap_or(label);
    path == query_name || path.ends_with(&format!("/{query_name}"))
}

fn cc_expand_link_flag_locations<'v>(
    flag: Value<'v>,
    inputs: &[Value<'v>],
    heap: Heap<'v>,
) -> starlark::Result<Value<'v>> {
    let Some(input) = flag.unpack_str() else {
        return Ok(flag);
    };
    if !input.contains("$(location")
        && !input.contains("$(execpath")
        && !input.contains("$(rootpath")
        && !input.contains("$(rlocationpath")
    {
        return Ok(flag);
    }

    let input_paths = inputs
        .iter()
        .filter_map(|value| cc_value_path(*value, heap))
        .collect::<Vec<_>>();
    let mut result = String::with_capacity(input.len());
    let mut remaining = input;
    while let Some(start) = remaining.find("$(") {
        result.push_str(&remaining[..start]);
        remaining = &remaining[start..];

        let pattern: Option<(usize, bool)> = if remaining.starts_with("$(locations ") {
            Some(("$(locations ".len(), true))
        } else if remaining.starts_with("$(location ") {
            Some(("$(location ".len(), false))
        } else if remaining.starts_with("$(execpaths ") {
            Some(("$(execpaths ".len(), true))
        } else if remaining.starts_with("$(execpath ") {
            Some(("$(execpath ".len(), false))
        } else if remaining.starts_with("$(rootpaths ") {
            Some(("$(rootpaths ".len(), true))
        } else if remaining.starts_with("$(rootpath ") {
            Some(("$(rootpath ".len(), false))
        } else if remaining.starts_with("$(rlocationpaths ") {
            Some(("$(rlocationpaths ".len(), true))
        } else if remaining.starts_with("$(rlocationpath ") {
            Some(("$(rlocationpath ".len(), false))
        } else {
            None
        };

        if let Some((prefix_len, is_multi)) = pattern
            && let Some(end) = remaining.find(')')
        {
            let label = remaining[prefix_len..end].trim();
            let paths = input_paths
                .iter()
                .filter(|path| cc_location_matches_path(label, path))
                .map(String::as_str)
                .collect::<Vec<_>>();
            if !paths.is_empty() {
                if is_multi {
                    result.push_str(&paths.join(" "));
                } else {
                    result.push_str(paths[0]);
                }
            } else {
                result.push_str(&remaining[..end + 1]);
            }
            remaining = &remaining[end + 1..];
            continue;
        }

        result.push_str("$(");
        remaining = &remaining[2..];
    }
    result.push_str(remaining);
    Ok(heap.alloc_str(&result).to_value())
}

fn cc_common_checkpoint(
    name: &'static str,
    fields: impl IntoIterator<Item = (&'static str, usize)>,
) {
    slug_util::memory_checkpoint::checkpoint(name, fields);
}

fn depset_shape(value: Value<'_>) -> (usize, usize, usize, usize) {
    depset_summary(value)
        .map(|summary| {
            (
                summary.direct_len,
                summary.transitive_len,
                summary.depth as usize,
                summary.is_empty as usize,
            )
        })
        .unwrap_or((0, 0, 0, value.is_none() as usize))
}

/// Returns whether an action name describes a C/C++ compile action.
/// Matches Bazel's `c-compile`, `c++-compile`, `c++-module-compile`, etc.
/// while excluding preprocess-only actions.
fn is_compile_action_name(name: NoneOr<&str>) -> bool {
    match name.into_option() {
        Some(n) => n.contains("compile") && !n.contains("preprocess"),
        None => true, // Default path: treat as compile.
    }
}

fn insert_if_set<'v>(
    map: &mut SmallMap<Value<'v>, Value<'v>>,
    heap: starlark::values::Heap<'v>,
    key: &str,
    val: Value<'v>,
) {
    if !val.is_none() {
        map.insert_hashed(heap.alloc_str(key).to_value().get_hashed().unwrap(), val);
    }
}

fn cc_attr<'v>(value: Value<'v>, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
    value.get_attr(name, heap).ok().flatten()
}

fn cc_string_list<'v>(value: Value<'v>, heap: Heap<'v>) -> Vec<String> {
    if value.is_none() {
        return Vec::new();
    }
    value
        .iterate(heap)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|item| item.unpack_str().map(str::to_owned))
        .collect()
}

fn parse_cc_with_feature_set<'v>(value: Value<'v>, heap: Heap<'v>) -> Option<CcWithFeatureSet> {
    Some(CcWithFeatureSet {
        features: cc_attr(value, "features", heap)
            .map(|features| cc_string_list(features, heap))
            .unwrap_or_default(),
        not_features: cc_attr(value, "not_features", heap)
            .map(|features| cc_string_list(features, heap))
            .unwrap_or_default(),
    })
}

fn parse_cc_env_entry<'v>(value: Value<'v>, heap: Heap<'v>) -> Option<CcEnvEntry> {
    let key = cc_attr(value, "key", heap)?.unpack_str()?.to_owned();
    let env_value = cc_attr(value, "value", heap)?.unpack_str()?.to_owned();
    let expand_if_available = cc_attr(value, "expand_if_available", heap)
        .and_then(|value| value.unpack_str().map(str::to_owned));
    Some(CcEnvEntry {
        key,
        value: env_value,
        expand_if_available,
    })
}

fn parse_cc_env_set<'v>(value: Value<'v>, heap: Heap<'v>) -> Option<CcEnvSet> {
    let actions = cc_attr(value, "actions", heap)
        .map(|actions| {
            cc_string_list(actions, heap)
                .into_iter()
                .map(|action| normalize_action_name(&action))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if actions.is_empty() {
        return None;
    }
    let env_entries = cc_attr(value, "env_entries", heap)
        .and_then(|entries| entries.iterate(heap).ok())
        .into_iter()
        .flatten()
        .filter_map(|entry| parse_cc_env_entry(entry, heap))
        .collect::<Vec<_>>();
    let with_features = cc_attr(value, "with_features", heap)
        .and_then(|sets| sets.iterate(heap).ok())
        .into_iter()
        .flatten()
        .filter_map(|set| parse_cc_with_feature_set(set, heap))
        .collect::<Vec<_>>();
    Some(CcEnvSet {
        actions,
        env_entries,
        with_features,
    })
}

fn parse_cc_feature_env_sets<'v>(
    feature_name: &str,
    feature: Value<'v>,
    heap: Heap<'v>,
) -> Option<CcFeatureEnvSets> {
    let env_sets = cc_attr(feature, "env_sets", heap)
        .and_then(|env_sets| env_sets.iterate(heap).ok())
        .into_iter()
        .flatten()
        .filter_map(|env_set| parse_cc_env_set(env_set, heap))
        .collect::<Vec<_>>();
    if env_sets.is_empty() {
        return None;
    }
    Some(CcFeatureEnvSets {
        feature_name: feature_name.to_owned(),
        env_sets,
    })
}

fn parse_cc_expand_if_equal<'v>(value: Value<'v>, heap: Heap<'v>) -> Option<CcExpandIfEqual> {
    Some(CcExpandIfEqual {
        variable: cc_attr(value, "name", heap)?.unpack_str()?.to_owned(),
        value: cc_attr(value, "value", heap)?.unpack_str()?.to_owned(),
    })
}

fn parse_cc_flag_group<'v>(value: Value<'v>, heap: Heap<'v>) -> Option<CcFlagGroup> {
    let flags = cc_attr(value, "flags", heap)
        .map(|flags| cc_string_list(flags, heap))
        .unwrap_or_default();
    let flag_groups = cc_attr(value, "flag_groups", heap)
        .and_then(|groups| groups.iterate(heap).ok())
        .into_iter()
        .flatten()
        .filter_map(|group| parse_cc_flag_group(group, heap))
        .collect::<Vec<_>>();
    Some(CcFlagGroup {
        flags,
        flag_groups,
        iterate_over: cc_attr(value, "iterate_over", heap)
            .and_then(|value| value.unpack_str().map(str::to_owned)),
        expand_if_available: cc_attr(value, "expand_if_available", heap)
            .and_then(|value| value.unpack_str().map(str::to_owned)),
        expand_if_not_available: cc_attr(value, "expand_if_not_available", heap)
            .and_then(|value| value.unpack_str().map(str::to_owned)),
        expand_if_true: cc_attr(value, "expand_if_true", heap)
            .and_then(|value| value.unpack_str().map(str::to_owned)),
        expand_if_false: cc_attr(value, "expand_if_false", heap)
            .and_then(|value| value.unpack_str().map(str::to_owned)),
        expand_if_equal: cc_attr(value, "expand_if_equal", heap)
            .and_then(|value| parse_cc_expand_if_equal(value, heap)),
    })
}

fn parse_cc_flag_set<'v>(
    value: Value<'v>,
    heap: Heap<'v>,
    default_action: Option<&str>,
) -> Option<CcFlagSet> {
    let mut actions = cc_attr(value, "actions", heap)
        .map(|actions| {
            cc_string_list(actions, heap)
                .into_iter()
                .flat_map(|action| expand_rules_cc_action_name(&action))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if actions.is_empty() {
        if let Some(default_action) = default_action {
            actions.push(normalize_action_name(default_action));
        }
    }
    let flag_groups = cc_attr(value, "flag_groups", heap)
        .and_then(|groups| groups.iterate(heap).ok())
        .into_iter()
        .flatten()
        .filter_map(|group| parse_cc_flag_group(group, heap))
        .collect::<Vec<_>>();
    if flag_groups.is_empty() {
        return None;
    }
    let with_features = cc_attr(value, "with_features", heap)
        .and_then(|sets| sets.iterate(heap).ok())
        .into_iter()
        .flatten()
        .filter_map(|set| parse_cc_with_feature_set(set, heap))
        .collect::<Vec<_>>();
    Some(CcFlagSet {
        actions,
        flag_groups,
        with_features,
    })
}

fn parse_cc_feature_flag_sets<'v>(
    feature_name: &str,
    feature: Value<'v>,
    heap: Heap<'v>,
) -> Option<CcFeatureFlagSets> {
    let flag_sets = cc_attr(feature, "flag_sets", heap)
        .and_then(|flag_sets| flag_sets.iterate(heap).ok())
        .into_iter()
        .flatten()
        .filter_map(|flag_set| parse_cc_flag_set(flag_set, heap, None))
        .collect::<Vec<_>>();
    if flag_sets.is_empty() {
        return None;
    }
    Some(CcFeatureFlagSets {
        feature_name: feature_name.to_owned(),
        flag_sets,
    })
}

fn expand_rules_cc_action_name(action: &str) -> Vec<String> {
    let short_name = action
        .rsplit_once(':')
        .map(|(_, name)| name)
        .unwrap_or(action);
    let names: &[&str] = match short_name {
        "all_cc_compile_actions" => &[
            "c++-compile",
            "c-compile",
            "preprocess-assemble",
            "assemble",
            "objc-compile",
            "linkstamp-compile",
            "c++-header-parsing",
            "c++-module-compile",
            "c++-module-codegen",
            "lto-backend",
        ],
        "all_cpp_compile_actions" | "cpp_compile_actions" => &[
            "c++-compile",
            "linkstamp-compile",
            "c++-header-parsing",
            "c++-module-compile",
            "c++-module-codegen",
            "lto-backend",
        ],
        "c_compile_actions" => &["c-compile"],
        "assembly_actions" => &["assemble", "preprocess-assemble"],
        "link_actions" | "all_cc_link_actions" => &[
            "c++-link-executable",
            "c++-link-dynamic-library",
            "c++-link-nodeps-dynamic-library",
            "lto-index-for-executable",
            "lto-index-for-dynamic-library",
            "lto-index-for-nodeps-dynamic-library",
            "objc-executable",
        ],
        "link_executable_actions" | "cc_link_executable_actions" => &[
            "c++-link-executable",
            "lto-index-for-executable",
            "objc-executable",
        ],
        "dynamic_library_link_actions" => &[
            "c++-link-dynamic-library",
            "c++-link-nodeps-dynamic-library",
            "lto-index-for-dynamic-library",
            "lto-index-for-nodeps-dynamic-library",
        ],
        "nodeps_dynamic_library_link_actions" => &[
            "c++-link-nodeps-dynamic-library",
            "lto-index-for-nodeps-dynamic-library",
        ],
        "transitive_link_actions" => &[
            "c++-link-executable",
            "c++-link-dynamic-library",
            "lto-index-for-executable",
            "lto-index-for-dynamic-library",
            "objc-executable",
        ],
        "cpp_link_executable" => &["c++-link-executable"],
        "cpp_link_dynamic_library" => &["c++-link-dynamic-library"],
        "cpp_link_nodeps_dynamic_library" => &["c++-link-nodeps-dynamic-library"],
        "cpp_link_static_library" => &["c++-link-static-library"],
        "cpp_compile" => &["c++-compile"],
        "c_compile" => &["c-compile"],
        _ => return vec![normalize_action_name(action)],
    };
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn cc_action_names<'v>(value: Value<'v>, heap: Heap<'v>) -> Vec<String> {
    depset_or_iterable_values(value, heap)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|action| {
            action.unpack_str().map(str::to_owned).or_else(|| {
                cc_attr(action, "name", heap)?
                    .unpack_str()
                    .map(str::to_owned)
            })
        })
        .flat_map(|action| expand_rules_cc_action_name(&action))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_rules_cc_link_action_set_labels() {
        let actions = expand_rules_cc_action_name("@rules_cc//cc/toolchains/actions:link_actions");
        assert!(actions.iter().any(|a| a == "c++-link-executable"));
        assert!(actions.iter().any(|a| a == "c++-link-dynamic-library"));
        assert!(
            actions
                .iter()
                .any(|a| a == "c++-link-nodeps-dynamic-library")
        );
    }

    #[test]
    fn expands_rules_cc_single_action_labels() {
        assert_eq!(
            expand_rules_cc_action_name(
                "@rules_cc//cc/toolchains/actions:cpp_link_dynamic_library"
            ),
            vec!["c++-link-dynamic-library".to_owned()]
        );
    }

    #[test]
    fn identifies_header_parsing_action_labels() {
        assert!(is_header_parsing_action("c++-header-parsing"));
        assert!(is_header_parsing_action(
            "@rules_cc//cc/toolchains/actions:cpp_header_parsing"
        ));
    }

    #[test]
    fn identifies_compiler_rt_crtbegin_link_output() {
        assert!(is_compiler_rt_crtbegin_link_output(Some(
            "gen/llvm+llvm_source+compiler-rt/e966/external/llvm+llvm_source+compiler-rt/libclang_rt.crtbegin.so"
        )));
        assert!(!is_compiler_rt_crtbegin_link_output(Some(
            "gen/other/libclang_rt.builtins.so"
        )));
    }
}

fn parse_cc_modern_feature_constraint<'v>(
    value: Value<'v>,
    heap: Heap<'v>,
) -> Option<CcWithFeatureSet> {
    let feature_names = |features: Value<'v>| {
        depset_or_iterable_values(features, heap)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|feature| {
                cc_attr(feature, "name", heap)?
                    .unpack_str()
                    .map(str::to_owned)
            })
            .collect::<Vec<_>>()
    };
    Some(CcWithFeatureSet {
        features: cc_attr(value, "all_of", heap)
            .map(feature_names)
            .unwrap_or_default(),
        not_features: cc_attr(value, "none_of", heap)
            .map(feature_names)
            .unwrap_or_default(),
    })
}

fn parse_cc_modern_args_flag_set<'v>(
    value: Value<'v>,
    heap: Heap<'v>,
    default_action: Option<&str>,
) -> Option<CcFlagSet> {
    let nested = cc_attr(value, "nested", heap)?;
    if nested.is_none() {
        return None;
    }
    let legacy_flag_group = cc_attr(nested, "legacy_flag_group", heap)?;
    let mut actions = cc_attr(value, "actions", heap)
        .map(|actions| cc_action_names(actions, heap))
        .unwrap_or_default();
    if actions.is_empty()
        && let Some(default_action) = default_action
    {
        actions.push(normalize_action_name(default_action));
    }
    if actions.is_empty() {
        return None;
    }
    let with_features = cc_attr(value, "requires_any_of", heap)
        .and_then(|sets| sets.iterate(heap).ok())
        .into_iter()
        .flatten()
        .filter_map(|set| parse_cc_modern_feature_constraint(set, heap))
        .collect::<Vec<_>>();
    Some(CcFlagSet {
        actions,
        flag_groups: vec![parse_cc_flag_group(legacy_flag_group, heap)?],
        with_features,
    })
}

fn parse_cc_modern_feature_flag_sets<'v>(
    feature_name: &str,
    feature: Value<'v>,
    heap: Heap<'v>,
) -> Option<CcFeatureFlagSets> {
    let args_list = cc_attr(feature, "args", heap)?;
    let flag_sets = cc_attr(args_list, "args", heap)
        .and_then(|args| args.iterate(heap).ok())
        .into_iter()
        .flatten()
        .filter_map(|args| parse_cc_modern_args_flag_set(args, heap, None))
        .collect::<Vec<_>>();
    if flag_sets.is_empty() {
        return None;
    }
    Some(CcFeatureFlagSets {
        feature_name: feature_name.to_owned(),
        flag_sets,
    })
}

fn parse_cc_modern_toolchain_arg_flag_sets<'v>(
    toolchain_config_info: Value<'v>,
    heap: Heap<'v>,
) -> Vec<CcFlagSet> {
    let Some(args_list) = cc_attr(toolchain_config_info, "args", heap) else {
        return Vec::new();
    };
    cc_attr(args_list, "by_action", heap)
        .and_then(|by_action| by_action.iterate(heap).ok())
        .into_iter()
        .flatten()
        .flat_map(|by_action| {
            let action_name = cc_attr(by_action, "action", heap)
                .and_then(|action| cc_attr(action, "name", heap))
                .and_then(|name| name.unpack_str().map(str::to_owned));
            cc_attr(by_action, "args", heap)
                .and_then(|args| args.iterate(heap).ok())
                .into_iter()
                .flatten()
                .filter_map(move |args| {
                    parse_cc_modern_args_flag_set(args, heap, action_name.as_deref())
                })
        })
        .collect()
}

fn cc_variable_to_string<'v>(value: Value<'v>, heap: Heap<'v>) -> Option<String> {
    if value.is_none() {
        return None;
    }
    if let Some(s) = value.unpack_str() {
        return Some(s.to_owned());
    }
    if let Some(b) = value.unpack_bool() {
        return Some(if b { "1" } else { "0" }.to_owned());
    }
    cc_attr(value, "path", heap).and_then(|path| path.unpack_str().map(str::to_owned))
}

fn expand_cc_scalar_template<'v>(
    template: &str,
    heap: Heap<'v>,
    get_var: impl Fn(&str) -> Option<Value<'v>>,
) -> starlark::Result<String> {
    let mut expanded = String::new();
    let mut rest = template;
    while let Some(start) = rest.find("%{") {
        expanded.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find('}') else {
            expanded.push_str(&rest[start..]);
            return Ok(expanded);
        };
        let name = &after_start[..end];
        let Some(value) = get_var(name).and_then(|value| cc_variable_to_string(value, heap)) else {
            return Err(starlark::Error::new_other(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Cannot expand C++ toolchain variable '{}'", name),
            )));
        };
        expanded.push_str(&value);
        rest = &after_start[end + 1..];
    }
    expanded.push_str(rest);
    Ok(expanded)
}

fn cc_variable_available<'v>(value: Option<Value<'v>>, heap: Heap<'v>) -> bool {
    let Some(value) = value else {
        return false;
    };
    if value.is_none() {
        return false;
    }
    if is_depset_value(value) {
        return depset_to_list(value, heap)
            .map(|values| !values.is_empty())
            .unwrap_or(false);
    }
    if let Ok(mut iter) = value.iterate(heap) {
        return iter.next().is_some();
    }
    true
}

fn cc_variable_truthy<'v>(value: Option<Value<'v>>, heap: Heap<'v>) -> bool {
    let Some(value) = value else {
        return false;
    };
    if value.is_none() {
        return false;
    }
    if let Some(b) = value.unpack_bool() {
        return b;
    }
    if let Some(s) = value.unpack_str() {
        return !s.is_empty();
    }
    if is_depset_value(value) {
        return depset_to_list(value, heap)
            .map(|values| !values.is_empty())
            .unwrap_or(false);
    }
    if let Ok(mut iter) = value.iterate(heap) {
        return iter.next().is_some();
    }
    true
}

fn cc_flag_group_conditions_match<'v>(
    group: &CcFlagGroup,
    heap: Heap<'v>,
    get_var: &impl Fn(&str) -> Option<Value<'v>>,
) -> bool {
    if let Some(name) = &group.expand_if_available {
        if !cc_variable_available(get_var(name), heap) {
            return false;
        }
    }
    if let Some(name) = &group.expand_if_not_available {
        if cc_variable_available(get_var(name), heap) {
            return false;
        }
    }
    if let Some(name) = &group.expand_if_true {
        if !cc_variable_truthy(get_var(name), heap) {
            return false;
        }
    }
    if let Some(name) = &group.expand_if_false {
        if cc_variable_truthy(get_var(name), heap) {
            return false;
        }
    }
    if let Some(equal) = &group.expand_if_equal {
        if get_var(&equal.variable)
            .and_then(|value| cc_variable_to_string(value, heap))
            .as_deref()
            != Some(equal.value.as_str())
        {
            return false;
        }
    }
    true
}

fn expand_cc_flag_group<'v>(
    group: &CcFlagGroup,
    args: &mut Vec<Value<'v>>,
    heap: Heap<'v>,
    get_var: &impl Fn(&str) -> Option<Value<'v>>,
    iteration: Option<(&str, Value<'v>)>,
) -> starlark::Result<()> {
    let get_scoped_var = |key: &str| -> Option<Value<'v>> {
        if let Some((iteration_key, iteration_value)) = iteration {
            if key == iteration_key {
                return Some(iteration_value);
            }
        }
        get_var(key)
    };

    if !cc_flag_group_conditions_match(group, heap, &get_scoped_var) {
        return Ok(());
    }

    if let Some(iterate_over) = &group.iterate_over
        && iteration.map(|(key, _)| key != iterate_over.as_str()) != Some(false)
    {
        if let Some(value) = get_var(iterate_over) {
            for item in depset_or_iterable_values(value, heap)? {
                expand_cc_flag_group(group, args, heap, get_var, Some((iterate_over, item)))?;
            }
        }
        return Ok(());
    }

    for nested in &group.flag_groups {
        expand_cc_flag_group(nested, args, heap, get_var, iteration)?;
    }
    for flag in &group.flags {
        let expanded = expand_cc_scalar_template(flag, heap, get_scoped_var)?;
        args.push(heap.alloc_str(&expanded).to_value());
    }
    Ok(())
}

fn expand_cc_flag_sets<'v>(
    feature_configuration: &FeatureConfiguration,
    action_name: &str,
    variables: Value<'v>,
    heap: Heap<'v>,
) -> starlark::Result<Vec<Value<'v>>> {
    let get_var = |key: &str| -> Option<Value<'v>> {
        if let Ok(Some(v)) = variables.get_attr(key, heap) {
            return Some(v);
        }
        if let Some(dict_ref) = DictRef::from_value(variables) {
            return dict_ref.get_str(key);
        }
        None
    };

    let mut args = Vec::new();
    for flag_set in feature_configuration
        .action_config_flag_sets
        .iter()
        .chain(feature_configuration.feature_flag_sets.iter())
    {
        if !flag_set.applies_to_action(action_name)
            || !flag_set.with_features_match(feature_configuration)
        {
            continue;
        }
        for group in &flag_set.flag_groups {
            expand_cc_flag_group(group, &mut args, heap, &get_var, None)?;
        }
    }
    Ok(args)
}

pub(crate) fn cc_toolchain_features_from_config_info<'v>(
    toolchain_config_info: Value<'v>,
    tools_directory: &str,
    heap: Heap<'v>,
) -> CcToolchainFeatures {
    let mut feature_names = Vec::new();
    let mut default_enabled_features = Vec::new();
    let mut feature_env_sets = Vec::new();
    let mut feature_flag_sets = Vec::new();
    let mut action_config_flag_sets = Vec::new();
    let mut action_config_names = Vec::new();

    // Extract feature names from toolchain_config_info. Slug's native
    // constructor uses `features`; rules_cc's Starlark wrapper stores the
    // Bazel-shaped provider field as `_features_DO_NOT_USE`.
    let legacy_features_val = toolchain_config_info
        .get_attr("_features_DO_NOT_USE", heap)
        .ok()
        .flatten();
    let features_val = legacy_features_val.or_else(|| {
        toolchain_config_info
            .get_attr("features", heap)
            .ok()
            .flatten()
    });
    if let Some(features_val) = features_val {
        if !features_val.is_none()
            && let Ok(iter) = features_val.iterate(heap)
        {
            for feature in iter {
                if let Ok(Some(name_val)) = feature.get_attr("name", heap)
                    && let Some(name) = name_val.unpack_str()
                {
                    feature_names.push(name.to_owned());
                    if let Some(env_sets) = parse_cc_feature_env_sets(name, feature, heap) {
                        feature_env_sets.push(env_sets);
                    }
                    if let Some(flag_sets) = parse_cc_feature_flag_sets(name, feature, heap)
                        .or_else(|| parse_cc_modern_feature_flag_sets(name, feature, heap))
                    {
                        feature_flag_sets.push(flag_sets);
                    }
                    if let Ok(Some(enabled_val)) = feature.get_attr("enabled", heap)
                        && enabled_val.unpack_bool() == Some(true)
                    {
                        default_enabled_features.push(name.to_owned());
                    }
                }
            }
        }
    }

    if let Ok(Some(enabled_features_val)) = toolchain_config_info.get_attr("enabled_features", heap)
    {
        if !enabled_features_val.is_none()
            && let Ok(iter) = enabled_features_val.iterate(heap)
        {
            for feature in iter {
                if let Some(name) =
                    cc_attr(feature, "name", heap).and_then(|name| name.unpack_str())
                    && !default_enabled_features
                        .iter()
                        .any(|feature| feature == name)
                {
                    default_enabled_features.push(name.to_owned());
                }
            }
        }
    }

    let configs_val = toolchain_config_info
        .get_attr("_action_configs_DO_NOT_USE", heap)
        .ok()
        .flatten()
        .or_else(|| {
            toolchain_config_info
                .get_attr("action_configs", heap)
                .ok()
                .flatten()
        });
    if let Some(configs_val) = configs_val {
        if !configs_val.is_none()
            && let Ok(iter) = configs_val.iterate(heap)
        {
            for config in iter {
                if let Ok(Some(name_val)) = config.get_attr("action_name", heap)
                    && let Some(name) = name_val.unpack_str()
                {
                    action_config_names.push(name.to_owned());
                    if let Some(flag_sets) = cc_attr(config, "flag_sets", heap)
                        .and_then(|flag_sets| flag_sets.iterate(heap).ok())
                    {
                        for flag_set in flag_sets {
                            if let Some(flag_set) = parse_cc_flag_set(flag_set, heap, Some(name)) {
                                action_config_flag_sets.push(flag_set);
                            }
                        }
                    }
                }
            }
        }
    }
    let modern_action_config_flag_sets =
        parse_cc_modern_toolchain_arg_flag_sets(toolchain_config_info, heap);
    if !modern_action_config_flag_sets.is_empty() {
        action_config_flag_sets = modern_action_config_flag_sets;
    }

    CcToolchainFeatures {
        feature_names,
        default_enabled_features,
        feature_env_sets,
        action_config_flag_sets,
        feature_flag_sets,
        action_config_names,
        tools_directory: tools_directory.to_owned(),
    }
}

/// Helper: push a path string or its corresponding artifact to the args list.
fn push_path_or_artifact<'v>(
    path_str: &str,
    artifact_map: &std::collections::HashMap<String, Value<'v>>,
    args: &mut Vec<Value<'v>>,
    heap: Heap<'v>,
) {
    if let Some(&artifact) = artifact_map.get(path_str) {
        args.push(artifact);
    } else {
        args.push(heap.alloc_str(path_str).to_value());
    }
}

/// Internal cc_common API struct.
///
/// Returned by `cc_common.internal_DO_NOT_USE()`. Contains internal functions
/// that rules_cc uses for low-level C++ compilation actions.
///
/// Reference: cc/private/cc_internal.bzl in rules_cc
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcCommonInternal;

impl Display for CcCommonInternal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cc_common.internal")
    }
}

starlark_simple_value!(CcCommonInternal);

#[starlark_value(type = "cc_common_internal")]
impl<'v> StarlarkValue<'v> for CcCommonInternal {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(cc_common_internal_methods)
    }
}

/// Internal methods for cc_common.internal_DO_NOT_USE() return value.
///
/// These are used by rules_cc's internal Starlark code.
#[starlark_module]
fn cc_common_internal_methods(builder: &mut MethodsBuilder) {
    /// Creates a C++ compile action.
    ///
    /// This is a native function that registers a compile action with Slug's
    /// action execution system. It bridges rules_cc's Starlark code to the
    /// native action registration infrastructure.
    #[allow(unused_variables)]
    fn create_cc_compile_action<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named, default = NoneType)] action_construction_context: Value<'v>,
        #[starlark(require = named, default = NoneType)] cc_compilation_context: Value<'v>,
        #[starlark(require = named, default = NoneType)] cc_toolchain: Value<'v>,
        #[starlark(require = named, default = NoneType)] configuration: Value<'v>,
        #[starlark(require = named, default = NoneType)] copts_filter: Value<'v>,
        #[starlark(require = named, default = NoneType)] feature_configuration: Value<'v>,
        #[starlark(require = named, default = NoneType)] additional_compilation_inputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] additional_include_scanning_roots: Value<
            'v,
        >,
        #[starlark(require = named, default = NoneType)] source: Value<'v>,
        #[starlark(require = named, default = NoneType)] output_file: Value<'v>,
        #[starlark(require = named, default = NoneType)] diagnostics_file: Value<'v>,
        #[starlark(require = named, default = NoneType)] dotd_file: Value<'v>,
        #[starlark(require = named, default = NoneType)] gcno_file: Value<'v>,
        #[starlark(require = named, default = NoneType)] dwo_file: Value<'v>,
        #[starlark(require = named, default = false)] use_pic: bool,
        #[starlark(require = named, default = NoneType)] lto_indexing_file: Value<'v>,
        #[starlark(require = named)] action_name: NoneOr<&str>,
        #[starlark(require = named, default = NoneType)] compile_build_variables: Value<'v>,
        #[starlark(require = named, default = false)] needs_include_validation: bool,
        #[starlark(require = named, default = NoneType)] toolchain_type: Value<'v>,
        #[starlark(kwargs)] kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let heap = eval.heap();

        // Validate required parameters
        if source.is_none() || output_file.is_none() {
            // Cannot create compile action without source and output
            return Ok(NoneType);
        }

        // Get the action name for mnemonic/category.
        let action_name_raw = action_name.into_option().unwrap_or("c-compile");
        let is_header_parsing_action = is_header_parsing_action(action_name_raw);

        // Get the actions from action_construction_context
        // The context is a CtxCheatWithActions that has the real actions
        let actions_attr_result = action_construction_context.get_attr("actions", heap);
        let actions_value = if let Ok(Some(actions)) = actions_attr_result {
            actions
        } else {
            // Fallback: action_construction_context might itself be actions
            action_construction_context
        };

        if is_header_parsing_action {
            // Header parsing is a validation-only action. Slug does not consume
            // the generated header-token artifact, and executing these actions
            // for runtime headers such as glibc's private bits/*.h turns
            // validation into a build failure. Still bind the declared outputs
            // so downstream providers that carry the token artifacts remain
            // well-formed.
            if let Ok(Some(write_method)) = actions_value.get_attr("write", heap) {
                let content = heap.alloc_str("").to_value();
                for artifact in [
                    output_file,
                    dotd_file,
                    diagnostics_file,
                    gcno_file,
                    dwo_file,
                    lto_indexing_file,
                ] {
                    if artifact.is_none() {
                        continue;
                    }
                    if let Ok(Some(as_output_method)) = artifact.get_attr("as_output", heap)
                        && let Ok(out) = eval.eval_function(as_output_method, &[], &[])
                    {
                        eval.eval_function(write_method, &[out, content], &[])?;
                    }
                }
            }
            return Ok(NoneType);
        }

        // Try to get the run method from actions
        let run_attr_result = actions_value.get_attr("run", heap);
        let run_method = match run_attr_result {
            Ok(Some(method)) => method,
            _ => {
                // No run method available - this is a stub context
                return Ok(NoneType);
            }
        };

        // Get source path for progress message
        let source_path = source
            .get_attr("path", heap)
            .ok()
            .flatten()
            .and_then(|v| v.unpack_str())
            .unwrap_or("unknown")
            .to_owned();

        // Convert Bazel action names (with hyphens) to Slug categories (snake_case)
        let action_name_str = action_category_from_bazel_action_name(action_name_raw);

        // Determine if this is a C++ compile action (vs plain C)
        let is_cpp = action_name_raw.contains("c++") || action_name_raw.contains("cpp");

        // Get compiler path from toolchain if available, otherwise use platform default
        let default_compiler = match std::env::consts::OS {
            "windows" => {
                // On Windows, resolve cl.exe to its full MSVC path
                if let Some(tools) = get_msvc_tool_paths() {
                    tools.cl.as_str()
                } else {
                    "cl.exe"
                }
            }
            "macos" => "/usr/bin/clang++",
            _ => {
                if is_cpp {
                    "/usr/bin/g++"
                } else {
                    "/usr/bin/gcc"
                }
            }
        };
        let compiler_path = if !cc_toolchain.is_none() {
            // Try to get compiler path from toolchain
            let raw = cc_toolchain
                .get_attr("compiler_executable", heap)
                .ok()
                .flatten()
                .and_then(|v| v.unpack_str().map(|s| s.to_owned()))
                .unwrap_or_else(|| default_compiler.to_owned());
            // Resolve bare "cl.exe" to full path on Windows
            if is_windows_host() {
                resolve_windows_compiler(&raw)
            } else {
                raw
            }
        } else {
            default_compiler.to_owned()
        };

        // Need to call .as_output() on the output artifact to mark it as an output
        // This is required by Slug's run() to bind the artifact to an action
        let output_artifact = match output_file.get_attr("as_output", heap) {
            Ok(Some(as_output_method)) => eval
                .eval_function(as_output_method, &[], &[])
                .unwrap_or(output_file),
            _ => output_file,
        };

        // Build the command line arguments list
        let msvc = is_msvc_compiler(&compiler_path);
        let mut args_vec: Vec<Value<'v>> = Vec::new();

        // Get output path as string for MSVC /Fo flag
        let output_path_str = output_file
            .get_attr("path", heap)
            .ok()
            .flatten()
            .and_then(|v| v.unpack_str().map(|s| s.to_owned()))
            .unwrap_or_default();

        if msvc {
            // MSVC flags: cl.exe /nologo /EHsc /c source /Fo<output>
            args_vec.push(heap.alloc_str(&compiler_path).to_value());
            args_vec.push(heap.alloc_str("/nologo").to_value());
            args_vec.push(heap.alloc_str("/EHsc").to_value());
            args_vec.push(heap.alloc_str("/c").to_value());
            args_vec.push(source);
            args_vec.push(
                heap.alloc_str(&format!("/Fo{}", output_path_str))
                    .to_value(),
            );

            // Add MSVC system include paths (STL headers, Windows SDK)
            if let Some(tools) = get_msvc_tool_paths() {
                for inc in [
                    &tools.msvc_include,
                    &tools.ucrt_include,
                    &tools.um_include,
                    &tools.shared_include,
                ] {
                    if !inc.is_empty() {
                        args_vec.push(heap.alloc_str(&format!("/I{}", inc)).to_value());
                    }
                }
            }
        } else {
            // GCC/Clang: compiler -c source -o output -fPIC
            args_vec.push(heap.alloc_str(&compiler_path).to_value());
            args_vec.push(heap.alloc_str("-c").to_value());
            args_vec.push(source);
            args_vec.push(heap.alloc_str("-o").to_value());
            args_vec.push(heap.alloc_str(&output_path_str).to_value());
            // -fPIC for position-independent code (not applicable to MSVC)
            args_vec.push(heap.alloc_str("-fPIC").to_value());

            // Plan 19.6: always-on compile flags + compilation-mode flag set.
            // Matches rules_cc's linux_cc_toolchain_config `compile_flags` and
            // mode-specific `*_compile_flags` features. Sourced from the
            // feature_configuration so exec-cfg tool builds pick up the opt
            // default declared by `platform(exec_properties={...})` while
            // target-cfg builds see the user's `--compilation_mode`.
            for flag in [
                "-U_FORTIFY_SOURCE",
                "-fstack-protector",
                "-Wall",
                "-fno-omit-frame-pointer",
            ] {
                args_vec.push(heap.alloc_str(flag).to_value());
            }
            let mode = compilation_mode_from_features(feature_configuration);
            match mode.as_str() {
                "opt" => {
                    for flag in [
                        "-g0",
                        "-O2",
                        "-D_FORTIFY_SOURCE=1",
                        "-DNDEBUG",
                        "-ffunction-sections",
                        "-fdata-sections",
                    ] {
                        args_vec.push(heap.alloc_str(flag).to_value());
                    }
                }
                "dbg" => {
                    args_vec.push(heap.alloc_str("-g").to_value());
                    args_vec.push(heap.alloc_str("-O0").to_value());
                }
                _ => {
                    args_vec.push(heap.alloc_str("-g0").to_value());
                }
            }
        }

        // Add workspace root as include path (Bazel always includes the workspace root
        // so that `#include "pkg/header.h"` works for any package in the workspace).
        let mut seen_include_dirs = std::collections::HashSet::new();
        {
            let flag = if msvc { "/I." } else { "-I." };
            args_vec.push(heap.alloc_str(flag).to_value());
            seen_include_dirs.insert(".".to_string());
        }

        // Add include directories from compilation context (deduplicated).
        //
        // `external_includes` is the field rules_cc's
        // `init_cc_compilation_context` populates when the target lives
        // in an external repo (cc_helper.bzl: `external_include_dirs.append(...)`
        // when `repo_name != ""`). The `-I` paths a target's hdrs glob
        // expects (e.g. `-Iexternal/llvm-project/llvm/include` for
        // `@llvm-project//llvm:config`) land there, not in `includes`.
        // Without iterating it, every external-repo cc_library compile
        // gets the headers in its input tree but never the `-I` flag
        // that maps the include name to the file, and the compile fails
        // with `fatal error: <hdr>: No such file or directory` even
        // though the file is on disk in the sandbox.
        if !cc_compilation_context.is_none() {
            for attr_name in &[
                "includes",
                "system_includes",
                "quote_includes",
                "external_includes",
            ] {
                if let Ok(Some(includes_val)) = cc_compilation_context.get_attr(attr_name, heap) {
                    if !includes_val.is_none() {
                        for elem in depset_values(includes_val, heap)? {
                            let dir = elem.to_str();
                            if dir.is_empty() || !seen_include_dirs.insert(dir.to_string()) {
                                continue;
                            }
                            let flag = include_flag_for_context_attr(attr_name, &dir, msvc);
                            args_vec.push(heap.alloc_str(&flag).to_value());
                        }
                    }
                }
            }
        }

        // Add include paths for external repos and source directories.
        if let Some(src_path_str) = source
            .get_attr("path", heap)
            .ok()
            .flatten()
            .and_then(|v| v.unpack_str())
        {
            // Normalize buck-out/v2/external_cells/bzlmod/<name>/<version>/... paths to
            // external/<name>/... for include path computation. This ensures that the same
            // include path logic applies whether the source is referenced via the symlink
            // (external/<name>/...) or the raw buck-out path.
            let normalized_src_path;
            let effective_src_path: &str =
                if let Some(norm) = normalize_external_cells_path(src_path_str) {
                    normalized_src_path = norm;
                    &normalized_src_path
                } else {
                    src_path_str
                };

            // For external repo sources with /src/ dir, add as include path.
            // Per Plan 29: this is a per-target source-derived dir; we add it
            // directly to this action's args_vec only — no global registry.
            if let Some(ext_idx) = effective_src_path.find("/src/") {
                let inc_dir = &effective_src_path[..ext_idx + 5];
                if seen_include_dirs.insert(inc_dir.to_string()) {
                    let flag = include_flag_for_dir_impl(inc_dir, msvc);
                    args_vec.push(heap.alloc_str(&flag).to_value());
                }
            }
            // Also add "external/<repo>/" for direct includes and "external/" for
            // repo-name-prefixed includes (e.g., `#include "rules_cc/cc/..."` in
            // rules_cc source files). Per Plan 29: per-target only.
            if effective_src_path.starts_with("external/") {
                if let Some(second_slash) = effective_src_path[9..].find('/') {
                    let repo_dir = &effective_src_path[..9 + second_slash];
                    if seen_include_dirs.insert(repo_dir.to_string()) {
                        let flag = include_flag_for_dir_impl(repo_dir, msvc);
                        args_vec.push(heap.alloc_str(&flag).to_value());
                    }
                }
                if seen_include_dirs.insert("external/".to_owned()) {
                    let ext_flag = if msvc {
                        "/Iexternal/"
                    } else {
                        "-isystemexternal/"
                    };
                    args_vec.push(heap.alloc_str(ext_flag).to_value());
                }
            }
            // Note: prior code registered the source file's parent directory in
            // a process-global include-dir registry so *other* targets' compile
            // actions would also pick it up. That cross-target leak is the
            // exact bug Plan 29 addresses — propagation between targets must
            // go through `CcCompilationContext.includes` providers, not a
            // shared mutex. The dir for THIS target's compile is already added
            // above (as repo_dir / inc_dir), which covers what the target
            // legitimately needs.
        }

        // Add preprocessor defines from cc_compilation_context
        // MSVC uses /D, GCC/Clang uses -D
        let define_prefix = if msvc { "/D" } else { "-D" };
        if !cc_compilation_context.is_none() {
            for attr_name in &["defines", "local_defines"] {
                if let Ok(Some(defines_val)) = cc_compilation_context.get_attr(attr_name, heap) {
                    if !defines_val.is_none() {
                        for elem in depset_values(defines_val, heap)? {
                            let def = elem.to_str();
                            if !def.is_empty() {
                                args_vec.push(
                                    heap.alloc_str(&format!("{}{}", define_prefix, def))
                                        .to_value(),
                                );
                            }
                        }
                    }
                }
            }
        }

        // (Plan 29.4 follow-up) Pull `user_compile_flags` out of the
        // `compile_build_variables` struct rules_cc 0.2.17 hands us.
        // rules_cc's Starlarkified compile path (`cc/private/compile/
        // compile.bzl`) packs the cc_library `copts` (+ conlyopts /
        // cxxopts, after `cc_helper.get_copts` make-variable expansion)
        // into a `compile_build_variables` struct keyed by
        // `user_compile_flags` and calls `cc_common.internal_DO_NOT_USE().
        // create_cc_compile_action(..., compile_build_variables = ...)`.
        // Before this, slug's create_cc_compile_action accepted the
        // parameter but ignored it — so for any cc_library going through
        // the Starlarkified path, every copt was silently dropped from
        // the compile command (`-DHAVE_VCS_VERSION_INC`, `-I$(WORKSPACE_
        // ROOT)/clang/lib/Basic`, etc., on `@llvm-project//clang:basic`).
        // Mirrors the get_var() helper in `get_memory_inefficient_command
        // _line` that already reads this variable for the toolchain-
        // feature-driven command-line synth.
        if !compile_build_variables.is_none() {
            let read_var = |key: &str| -> Option<Value<'v>> {
                if let Ok(Some(v)) = compile_build_variables.get_attr(key, heap) {
                    if !v.is_none() {
                        return Some(v);
                    }
                }
                if let Some(dict_ref) = DictRef::from_value(compile_build_variables) {
                    if let Some(v) = dict_ref.get_str(key) {
                        if !v.is_none() {
                            return Some(v);
                        }
                    }
                }
                None
            };
            let collect_flags = |val: Value<'v>| -> Vec<String> {
                let mut out = Vec::new();
                if is_depset_value(val) {
                    if let Ok(listed) = depset_to_list(val, heap) {
                        for f in listed {
                            if let Some(s) = f.unpack_str() {
                                if !s.is_empty() {
                                    out.push(s.to_owned());
                                }
                            }
                        }
                        return out;
                    }
                    return out;
                }
                if let Ok(iter) = val.iterate(heap) {
                    for f in iter {
                        if let Some(s) = f.unpack_str() {
                            if !s.is_empty() {
                                out.push(s.to_owned());
                            }
                        }
                    }
                }
                out
            };
            if let Some(ucf) = read_var("user_compile_flags") {
                for s in collect_flags(ucf) {
                    args_vec.push(heap.alloc_str(&s).to_value());
                }
            }
        }

        // Add dependency file generation flags if dotd_file is specified
        if !dotd_file.is_none() {
            if msvc {
                // MSVC: /showIncludes outputs deps to stdout (no .d file created).
                // Use actions.write() to create an empty .d file as a separate action,
                // since rules_cc declared the artifact and it must be bound.
                args_vec.push(heap.alloc_str("/showIncludes").to_value());
                if let Ok(Some(write_method)) = actions_value.get_attr("write", heap) {
                    let dotd_output = if let Ok(Some(m)) = dotd_file.get_attr("as_output", heap) {
                        eval.eval_function(m, &[], &[]).ok()
                    } else {
                        None
                    };
                    if let Some(dotd_out) = dotd_output {
                        // actions.write(output, content) - write empty string to .d file
                        let content = heap.alloc_str("").to_value();
                        let _ = eval.eval_function(write_method, &[dotd_out, content], &[]);
                    }
                }
            } else {
                // GCC/Clang: -MMD -MF <depfile>
                args_vec.push(heap.alloc_str("-MMD").to_value());
                args_vec.push(heap.alloc_str("-MF").to_value());
                if let Some(dotd_path) = dotd_file
                    .get_attr("path", heap)
                    .ok()
                    .flatten()
                    .and_then(|v| v.unpack_str().map(str::to_owned))
                {
                    args_vec.push(heap.alloc_str(&dotd_path).to_value());
                }
            }
        }

        let arguments = heap.alloc(args_vec);

        // Build the outputs list with all output artifacts
        let mut outputs_vec: Vec<Value<'v>> = vec![output_artifact];

        // Helper to add auxiliary output artifact to the outputs list
        macro_rules! add_output {
            ($artifact:expr) => {
                if !$artifact.is_none() {
                    if let Ok(Some(method)) = $artifact.get_attr("as_output", heap) {
                        if let Ok(out) = eval.eval_function(method, &[], &[]) {
                            outputs_vec.push(out);
                        }
                    }
                }
            };
        }

        // Add auxiliary outputs if provided (dotd, diagnostics, gcno, dwo, lto)
        // On MSVC, dotd_file is handled by a separate write action
        if !msvc {
            add_output!(dotd_file);
        }
        add_output!(diagnostics_file);
        add_output!(gcno_file);
        add_output!(dwo_file);
        add_output!(lto_indexing_file);

        let outputs_list = heap.alloc(outputs_vec);

        // Thread transitive headers (including rules_cc virtual-include
        // symlinks) into the compile action as inputs so scheduling orders
        // the symlink/template actions before this compile. Without this,
        // `<bin_dir>/.../_virtual_includes/<name>/<hdr>` is referenced via
        // `-I` but never materialized.
        let mut compile_inputs: Vec<Value<'v>> = Vec::new();
        if !cc_compilation_context.is_none() {
            if let Ok(Some(headers)) = cc_compilation_context.get_attr("headers", heap) {
                if !headers.is_none() {
                    compile_inputs.extend(depset_to_artifact_inputs(headers, heap)?);
                }
            }
        }
        let compile_inputs_value: Value<'v> = if compile_inputs.is_empty() {
            Value::new_none()
        } else {
            heap.alloc(compile_inputs)
        };

        // Build the progress message
        let progress_msg = heap
            .alloc_str(&format!("Compiling {}", source_path))
            .to_value();

        // Build named arguments for run()
        // run(arguments, outputs=outputs, mnemonic=mnemonic, progress_message=msg, identifier=id)
        //
        // Use source path + PIC-ness as identifier to disambiguate multiple
        // compile actions for the same source. rules_cc's `cc_common.compile`
        // may register both a PIC and a non-PIC compile of the same source
        // when `use_pic_for_dynamic_libs` differs from `use_pic_for_binaries`
        // (Plan 20.2) — notably in opt mode. Without this suffix both
        // actions register under the same (category, identifier), tripping
        // slug's action-registry dedup.
        let identifier_str = if use_pic {
            format!("{source_path}.pic")
        } else {
            source_path.clone()
        };
        let identifier = heap.alloc_str(&identifier_str).to_value();
        let mut named_args: Vec<(&str, Value<'v>)> = vec![
            ("outputs", outputs_list),
            ("mnemonic", heap.alloc_str(&action_name_str).to_value()),
            ("progress_message", progress_msg),
            ("identifier", identifier),
        ];
        if !compile_inputs_value.is_none() {
            named_args.push(("inputs", compile_inputs_value));
        }

        // Invoke actions.run() using Starlark's function evaluation
        // This properly registers the action through Slug's infrastructure
        let _run_result = eval.eval_function(run_method, &[arguments], &named_args)?;

        Ok(NoneType)
    }

    /// Gets the artifact name for a given category.
    ///
    /// Categories include: "object_file", "pic_object_file", "executable", etc.
    #[allow(unused_variables)]
    fn get_artifact_name_for_category<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named)] category: &str,
        #[starlark(require = named, default = "")] output_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<String> {
        // TODO(cc_common): Implement proper artifact naming based on toolchain
        // For now, return basic naming conventions
        let name = if output_name.is_empty() {
            "output"
        } else {
            output_name
        };
        // Category names come in both uppercase (from rules_cc artifact_category_names struct)
        // and lowercase (from direct string usage). Normalize to uppercase for matching.
        // Platform-specific extensions: Windows uses .obj/.lib/.dll/.exe, Unix uses .o/.a/.so
        let windows = is_windows_host();
        let result = match category.to_uppercase().as_str() {
            // Object files
            "OBJECT_FILE" => {
                if windows {
                    format!("{}.obj", name)
                } else {
                    format!("{}.o", name)
                }
            }
            "PIC_OBJECT_FILE" => {
                if windows {
                    format!("{}.obj", name)
                } else {
                    format!("{}.pic.o", name)
                }
            }
            "PIC_FILE" => {
                if windows {
                    format!("{}.obj", name)
                } else {
                    format!("{}.pic", name)
                }
            }

            // Libraries
            "STATIC_LIBRARY" => {
                if windows {
                    format!("{}.lib", name)
                } else {
                    format!("lib{}.a", name)
                }
            }
            "ALWAYSLINK_STATIC_LIBRARY" => {
                if windows {
                    format!("{}.lo.lib", name)
                } else {
                    format!("lib{}.lo", name)
                }
            }
            "DYNAMIC_LIBRARY" => {
                if windows {
                    format!("{}.dll", name)
                } else {
                    format!("lib{}.so", name)
                }
            }
            "INTERFACE_LIBRARY" => {
                if windows {
                    format!("{}.if.lib", name)
                } else {
                    format!("lib{}.so", name)
                }
            }

            // Executables
            "EXECUTABLE" => {
                if windows {
                    format!("{}.exe", name)
                } else {
                    name.to_owned()
                }
            }

            // Dependency tracking
            "INCLUDED_FILE_LIST" => format!("{}.d", name),

            // Diagnostics
            "SERIALIZED_DIAGNOSTICS_FILE" => format!("{}.dia", name),

            // Headers
            "GENERATED_HEADER" => format!("{}.h", name),
            "PROCESSED_HEADER" => format!("{}.h", name),

            // C++20 modules
            "CPP_MODULE" => format!("{}.pcm", name),
            "CPP_MODULES_DDI" => format!("{}.ddi", name),
            "CPP_MODULES_INFO" => format!("{}.modinfo", name),
            "CPP_MODULES_MODMAP" => format!("{}.modmap", name),
            "CPP_MODULES_MODMAP_INPUT" => format!("{}.input_modmap", name),

            // Preprocessing
            "PREPROCESSED_C_SOURCE" => format!("{}.i", name),
            "PREPROCESSED_CPP_SOURCE" => format!("{}.ii", name),

            // Coverage (gcov)
            "COVERAGE_DATA_FILE" => format!("{}.gcno", name),
            "COVERAGE_NOTES_FILE" => format!("{}.gcda", name),

            // Other
            "CLIF_OUTPUT_PROTO" => format!("{}.opb", name),

            // Unknown category - use category as extension
            _ => format!("{}.{}", name, category),
        };
        Ok(result)
    }

    /// Combines toolchain variables from multiple sources.
    ///
    /// Takes 2 or 3 positional arguments - base variables plus 1-2 override variables.
    /// Variables are merged, with later arguments taking precedence.
    #[allow(unused_variables)]
    fn combine_cc_toolchain_variables<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = pos)] base: Value<'v>,
        #[starlark(require = pos)] first_override: Value<'v>,
        #[starlark(default = NoneType)] second_override: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        let mut merged: SmallMap<Value<'v>, Value<'v>> = SmallMap::new();

        // Merge all variable dicts: base + first_override + second_override (later values override)
        for vars_val in [base, first_override, second_override] {
            if vars_val.is_none() {
                continue;
            }
            // Try to downcast to CcToolchainVariables and iterate its inner dict
            if let Some(cv) = vars_val.downcast_ref::<CcToolchainVariablesGen<Value<'v>>>() {
                let inner = cv.vars;
                if !inner.is_none() {
                    if let Some(dict_ref) = DictRef::from_value(inner) {
                        for (k, v) in dict_ref.iter() {
                            if let Ok(hashed) = k.get_hashed() {
                                merged.insert_hashed(hashed, v);
                            }
                        }
                    }
                }
            }
            // If it's not a CcToolchainVariables (e.g., empty depset from _build_variables), skip
        }

        let merged_dict = heap.alloc(Dict::new(merged));
        Ok(heap.alloc(CcToolchainVariablesGen { vars: merged_dict }))
    }

    /// Gets the rule context from an actions object.
    ///
    /// This is a workaround used by rules_cc to access ctx from actions.
    /// We preserve the real actions object so create_cc_compile_action can use it.
    #[allow(unused_variables)]
    fn actions2ctx_cheat<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = pos)] actions: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Try to extract label info from the actions object
        let (cell_name, pkg_path, target_name, cfg_hash) =
            (|| -> Option<(String, String, String, String)> {
                let analysis_actions = actions
                    .downcast_ref::<crate::interpreter::rule_defs::context::AnalysisActions>(
                )?;
                let state = analysis_actions.state.try_borrow().ok()?;
                let registry = state.as_ref()?;
                let owner = registry.actions.owner();
                match owner {
                    slug_core::deferred::key::DeferredHolderKey::Base(
                        slug_core::deferred::base_deferred_key::BaseDeferredKey::TargetLabel(label),
                    ) => {
                        let cell = label.pkg().cell_name().as_str().to_owned();
                        let pkg = label.pkg().cell_relative_path().to_string();
                        let name = label.name().as_str().to_owned();
                        let cfg = label.cfg().output_hash().as_str().to_owned();
                        Some((cell, pkg, name, cfg))
                    }
                    _ => None,
                }
            })()
            .unwrap_or_else(|| {
                (
                    "".to_owned(),
                    "stub".to_owned(),
                    "stub".to_owned(),
                    "".to_owned(),
                )
            });

        // Return a wrapper that preserves the real actions object and label info
        // This allows create_cc_compile_action to register real actions
        Ok(eval.heap().alloc(CtxCheatWithActions {
            actions,
            cell_name,
            pkg_path,
            target_name,
            cfg_hash,
        }))
    }

    /// Creates CcToolchainVariables from a dictionary.
    #[allow(unused_variables)]
    fn cc_toolchain_variables<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named, default = NoneType)] vars: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Wrap the variables dict in CcToolchainVariables
        Ok(eval.heap().alloc(CcToolchainVariablesGen { vars }))
    }

    /// Freezes nested Starlark containers for rules_cc provider fields.
    fn freeze<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        value: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        if slug_util::memory_checkpoint::enabled() {
            let (direct, transitive, depth, is_empty) = depset_shape(value);
            let iterable_len = if value.is_none() || is_depset_value(value) {
                0
            } else {
                value
                    .iterate(eval.heap())
                    .map(|iter| iter.count())
                    .unwrap_or(0)
            };
            let freeze_count = CC_INTERNAL_FREEZE_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
            if direct + transitive >= CC_INTERNAL_FREEZE_CHECKPOINT_LARGE_LEN
                || depth >= 16
                || iterable_len >= CC_INTERNAL_FREEZE_CHECKPOINT_LARGE_LEN
                || freeze_count.is_power_of_two()
            {
                cc_common_checkpoint(
                    "cc_internal_freeze",
                    [
                        ("freeze_count", freeze_count),
                        ("is_depset", is_depset_value(value) as usize),
                        ("depset_direct", direct),
                        ("depset_transitive", transitive),
                        ("depset_depth", depth),
                        ("depset_empty", is_empty),
                        ("iterable_len", iterable_len),
                        ("is_none", value.is_none() as usize),
                    ],
                );
            }
        }
        cc_internal_freeze_value(value, eval.heap())
    }

    /// Returns the execution requirements for a given action.
    ///
    /// Returns a list of execution requirements (like "requires-worker-protocol:json")
    /// that should be added to actions using the specified tool.
    #[allow(unused_variables)]
    fn get_tool_requirement_for_action<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Return an empty list - no special execution requirements
        Ok(eval.heap().alloc(Vec::<String>::new()))
    }

    /// Creates a tree artifact compile action template.
    fn create_cc_compile_action_template<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        // TODO(cc_common): Implement tree artifact compile template
        Ok(NoneType)
    }

    /// Wraps link actions for platform compatibility.
    ///
    /// Arguments:
    /// - actions: The ctx.actions object
    /// - build_config: Build configuration (usually ctx.configuration), optional
    /// - use_shareable_artifact_factory: Whether to use shareable artifact factory, optional
    #[allow(unused_variables)]
    fn wrap_link_actions<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = pos)] actions: Value<'v>,
        #[starlark(default = NoneType)] build_config: Value<'v>,
        #[starlark(default = false)] use_shareable_artifact_factory: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TODO(cc_common): Implement link action wrapping
        // Return a wrapper that proxies the actions object
        Ok(actions)
    }

    /// Gets the SONAME for a dynamic library.
    #[allow(unused_variables)]
    fn dynamic_library_soname<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = pos)] actions: Value<'v>,
        #[starlark(require = pos)] short_path: &str,
        #[starlark(require = pos)] preserve_name: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<String> {
        // Extract library name from the path for SONAME
        let basename = short_path.rsplit('/').next().unwrap_or(short_path);
        Ok(basename.to_owned())
    }

    /// Creates a symlink for a dynamic library.
    #[allow(unused_variables)]
    fn dynamic_library_symlink<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = pos)] actions: Value<'v>,
        #[starlark(require = pos)] artifact: Value<'v>,
        #[starlark(require = pos)] solib_dir: Value<'v>,
        #[starlark(require = pos)] preserve_name: bool,
        #[starlark(require = pos)] use_short_path: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Return the artifact unchanged - symlink creation is a stub
        Ok(artifact)
    }

    /// Interns a sequence for efficiency (returns it unchanged).
    #[allow(unused_variables)]
    fn intern_seq<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = pos)] value: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Return the sequence unchanged - interning is just an optimization
        Ok(value)
    }

    /// Gets link arguments for a given feature configuration.
    ///
    /// This function extracts variables from build_variables and constructs
    /// the linker command line arguments. For rules_cc compatibility, this
    /// returns an Args-like list that can be passed to actions.run(arguments=...).
    ///
    /// The build_variables contain `libraries_to_link` which is a list of
    /// provider instances created by rules_cc:
    /// - _NamedLibraryInfo: type in {object_file, static_library, dynamic_library, interface_library}
    /// - _ObjectFileGroupInfo: type = object_file_group, has .object_files list
    /// - _VersionedLibraryInfo: type = versioned_dynamic_library, has .name and .path
    ///
    /// For dynamic_library type, .name is a short library name (e.g., "hello_lib")
    /// that should be emitted as -l<name>. For other types, .name is a full path.
    #[allow(unused_variables)]
    fn get_link_args<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: Value<'v>,
        #[starlark(require = named)] build_variables: Value<'v>,
        #[starlark(require = named, default = NoneType)] parameter_file_type: Value<'v>,
        // Slug extension: Optional input artifacts for proper path resolution.
        #[starlark(require = named, default = NoneType)] input_artifacts: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();

        // Get action name as string, normalized to hyphen form
        let raw_action_name = action_name.unpack_str().unwrap_or("c++-link-executable");
        let normalized_name = normalize_action_name(raw_action_name);
        let action_name_str = normalized_name.as_str();

        let mut args: Vec<Value<'v>> = Vec::new();

        // Helper to get a variable value from either CcToolchainVariables or a raw dict
        let get_var = |key: &str| -> Option<Value<'v>> {
            if let Some(v) = build_variables.get_attr(key, heap).ok().flatten() {
                return Some(v);
            }
            if let Some(dict_ref) = DictRef::from_value(build_variables) {
                if let Some(v) = dict_ref.get_str(key) {
                    return Some(v);
                }
            }
            None
        };

        // Build a map from artifact paths to artifact values (for resolving string paths)
        let mut artifact_map: std::collections::HashMap<String, Value<'v>> =
            std::collections::HashMap::new();
        if !input_artifacts.is_none() {
            let artifacts = if is_depset_value(input_artifacts) {
                depset_to_list(input_artifacts, heap).unwrap_or_default()
            } else if let Ok(iter) = input_artifacts.iterate(heap) {
                iter.collect()
            } else {
                Vec::new()
            };
            for artifact in artifacts {
                if let Ok(Some(short_path)) = artifact.get_attr("short_path", heap) {
                    if let Some(path_str) = short_path.unpack_str() {
                        artifact_map.insert(path_str.to_owned(), artifact);
                    }
                }
                if let Ok(Some(path_attr)) = artifact.get_attr("path", heap) {
                    if let Some(path_str) = path_attr.unpack_str() {
                        artifact_map.insert(path_str.to_owned(), artifact);
                    }
                }
            }
        }

        // --- Output path ---
        let msvc = is_windows_host();
        if let Some(output) = get_var("output_execpath") {
            if action_name_str.contains("static-library") {
                if msvc {
                    // MSVC lib.exe: /nologo /OUT:<path>
                    args.push(heap.alloc_str("/nologo").to_value());
                } else {
                    args.push(heap.alloc_str("rcs").to_value());
                }
            } else if action_name_str.contains("dynamic-library") {
                if msvc {
                    args.push(heap.alloc_str("/nologo").to_value());
                    args.push(heap.alloc_str("/DLL").to_value());
                } else {
                    args.push(heap.alloc_str("-shared").to_value());
                    args.push(heap.alloc_str("-o").to_value());
                }
            } else {
                if msvc {
                    args.push(heap.alloc_str("/nologo").to_value());
                } else {
                    args.push(heap.alloc_str("-o").to_value());
                }
            }

            // For MSVC, format output as /OUT:<path>
            let output_path_str = if let Some(s) = output.unpack_str() {
                Some(s.to_owned())
            } else if let Ok(Some(path)) = output.get_attr("path", heap) {
                path.unpack_str().map(|s| s.to_owned())
            } else {
                None
            };

            if msvc {
                if let Some(ref path) = output_path_str {
                    args.push(heap.alloc_str(&format!("/OUT:{}", path)).to_value());
                }
                // Also need to bind the artifact
                if let Ok(Some(as_output_method)) = output.get_attr("as_output", heap) {
                    let _ = eval.eval_function(as_output_method, &[], &[]);
                }
            } else if output.unpack_str().is_some() {
                args.push(output);
            } else {
                let path_result = output.get_attr("path", heap);
                if let Ok(Some(as_output_method)) = output.get_attr("as_output", heap) {
                    match eval.eval_function(as_output_method, &[], &[]) {
                        Ok(output_artifact) => {
                            args.push(output_artifact);
                        }
                        Err(_) => {
                            if let Ok(Some(path)) = path_result {
                                args.push(path);
                            } else {
                                args.push(heap.alloc_str(&output.to_str()).to_value());
                            }
                        }
                    }
                } else if let Ok(Some(path)) = path_result {
                    args.push(path);
                } else {
                    args.push(heap.alloc_str(&output.to_str()).to_value());
                }
            }
            if !msvc
                && action_name_str.contains("dynamic-library")
                && is_compiler_rt_crtbegin_link_output(output_path_str.as_deref())
            {
                args.push(heap.alloc_str("-nostdlib").to_value());
            }
        }

        // Helper: iterate a value that may be a list or depset
        let iterate_value =
            |val: Value<'v>, eval_ref: &mut Evaluator<'v, '_, '_>| -> Vec<Value<'v>> {
                let h = eval_ref.heap();
                if is_depset_value(val) {
                    return depset_to_list(val, h).unwrap_or_default();
                }
                if let Ok(iter) = val.iterate(h) {
                    iter.collect()
                } else {
                    Vec::new()
                }
            };

        // --- Library search directories ---
        let mut lib_search_dirs: Vec<String> = Vec::new();
        if let Some(dirs) = get_var("library_search_directories") {
            for dir in iterate_value(dirs, eval) {
                if let Some(dir_str) = dir.unpack_str() {
                    if !dir_str.is_empty() {
                        if msvc {
                            args.push(heap.alloc_str(&format!("/LIBPATH:{}", dir_str)).to_value());
                        } else {
                            args.push(heap.alloc_str(&format!("-L{}", dir_str)).to_value());
                        }
                        lib_search_dirs.push(dir_str.to_owned());
                    }
                }
            }
        }

        // Add MSVC system library paths
        if msvc {
            if let Some(tools) = get_msvc_tool_paths() {
                if !tools.msvc_lib.is_empty() {
                    args.push(
                        heap.alloc_str(&format!("/LIBPATH:{}", tools.msvc_lib))
                            .to_value(),
                    );
                }
                if !tools.ucrt_lib.is_empty() {
                    args.push(
                        heap.alloc_str(&format!("/LIBPATH:{}", tools.ucrt_lib))
                            .to_value(),
                    );
                }
                if !tools.um_lib.is_empty() {
                    args.push(
                        heap.alloc_str(&format!("/LIBPATH:{}", tools.um_lib))
                            .to_value(),
                    );
                }
            }
        }

        // --- Libraries to link ---
        // On Linux, wrap in --start-group/--end-group for circular dep resolution.
        // MSVC doesn't need this (it always resolves circular deps).
        let is_executable_link = action_name_str.contains("executable");
        if is_executable_link && !msvc {
            args.push(heap.alloc_str("-Wl,--start-group").to_value());
        }
        // Process based on .type field from rules_cc provider instances
        if let Some(libs) = get_var("libraries_to_link") {
            if let Ok(iter) = libs.iterate(heap) {
                for lib in iter {
                    // Get the library type to determine how to format the argument
                    let lib_type = lib
                        .get_attr("type", heap)
                        .ok()
                        .flatten()
                        .and_then(|v| v.unpack_str().map(|s| s.to_owned()));

                    let is_whole_archive = lib
                        .get_attr("is_whole_archive", heap)
                        .ok()
                        .flatten()
                        .map(|v| v.unpack_bool() == Some(true))
                        .unwrap_or(false);

                    if is_whole_archive {
                        args.push(heap.alloc_str("-Wl,--whole-archive").to_value());
                    }

                    match lib_type.as_deref() {
                        Some("dynamic_library") => {
                            // Dynamic library: emit -l<name> flag
                            // .name is a short name like "hello_lib" (from "libhello_lib.so")
                            if let Some(name) = lib.get_attr("name", heap).ok().flatten() {
                                if let Some(name_str) = name.unpack_str() {
                                    args.push(
                                        heap.alloc_str(&format!("-l{}", name_str)).to_value(),
                                    );
                                }
                            }
                        }
                        Some("versioned_dynamic_library") => {
                            // Versioned dynamic library: use -l:<name> for exact match
                            if let Some(name) = lib.get_attr("name", heap).ok().flatten() {
                                if let Some(name_str) = name.unpack_str() {
                                    args.push(
                                        heap.alloc_str(&format!("-l:{}", name_str)).to_value(),
                                    );
                                }
                            }
                        }
                        Some("object_file_group") => {
                            // Object file group: iterate .object_files and add each
                            if let Some(object_files) =
                                lib.get_attr("object_files", heap).ok().flatten()
                            {
                                if let Ok(obj_iter) = object_files.iterate(heap) {
                                    for obj in obj_iter {
                                        if obj.get_type() == "File" {
                                            args.push(obj);
                                        } else if let Some(path_str) = obj.unpack_str() {
                                            push_path_or_artifact(
                                                path_str,
                                                &artifact_map,
                                                &mut args,
                                                heap,
                                            );
                                        } else {
                                            args.push(obj);
                                        }
                                    }
                                }
                            }
                        }
                        Some("object_file")
                        | Some("static_library")
                        | Some("interface_library") => {
                            // These types use .name as a full path
                            if let Some(name) = lib.get_attr("name", heap).ok().flatten() {
                                if let Some(name_str) = name.unpack_str() {
                                    push_path_or_artifact(name_str, &artifact_map, &mut args, heap);
                                } else {
                                    args.push(name);
                                }
                            }
                        }
                        _ => {
                            // Unknown type or no type field - use legacy fallback
                            if let Some(path_str) = lib.unpack_str() {
                                push_path_or_artifact(path_str, &artifact_map, &mut args, heap);
                            } else if let Some(artifact) =
                                lib.get_attr("artifact", heap).ok().flatten()
                            {
                                if artifact.is_none() {
                                    if let Some(name) = lib.get_attr("name", heap).ok().flatten() {
                                        if let Some(name_str) = name.unpack_str() {
                                            push_path_or_artifact(
                                                name_str,
                                                &artifact_map,
                                                &mut args,
                                                heap,
                                            );
                                        } else {
                                            args.push(name);
                                        }
                                    }
                                } else {
                                    args.push(artifact);
                                }
                            } else if let Some(name) = lib.get_attr("name", heap).ok().flatten() {
                                if let Some(name_str) = name.unpack_str() {
                                    push_path_or_artifact(name_str, &artifact_map, &mut args, heap);
                                } else {
                                    args.push(name);
                                }
                            } else if lib.get_type() == "File" {
                                args.push(lib);
                            } else {
                                let path_str = lib.to_str();
                                push_path_or_artifact(&path_str, &artifact_map, &mut args, heap);
                            }
                        }
                    }

                    if is_whole_archive {
                        args.push(heap.alloc_str("-Wl,--no-whole-archive").to_value());
                    }
                }
            }
        }

        if is_executable_link && !msvc {
            args.push(heap.alloc_str("-Wl,--end-group").to_value());
        }

        // --- User link flags ---
        // Deduplicate flags while preserving order, since transitive depsets
        // can produce massive duplication (e.g., -lm -lpthread repeated 2000+ times).
        if let Some(flags) = get_var("user_link_flags") {
            let mut seen_flags = std::collections::HashSet::new();
            if let Ok(iter) = flags.iterate(heap) {
                for flag in iter {
                    if let Some(s) = flag.unpack_str() {
                        if seen_flags.insert(s.to_owned()) {
                            args.push(flag);
                        }
                    }
                }
            }
        }

        // --- Runtime library search directories (-rpath flags) ---
        // Use $ORIGIN-relative paths so the runtime linker can find shared libraries
        // regardless of the working directory when the binary is executed.
        let output_dir: Option<String> = get_var("output_execpath").and_then(|v| {
            let path_str = if let Some(s) = v.unpack_str() {
                s.to_owned()
            } else if let Ok(Some(path_attr)) = v.get_attr("path", heap) {
                path_attr
                    .unpack_str()
                    .map(|s| s.to_owned())
                    .unwrap_or_else(|| v.to_str())
            } else {
                v.to_str()
            };
            std::path::Path::new(&path_str)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
        });

        let make_origin_rpath = |dir_str: &str| -> String {
            // runtime_library_search_directories paths are relative to the binary's
            // output directory (i.e., relative to $ORIGIN). Use them directly.
            // e.g. dir_str="../__hello_lib__" → "-Wl,-rpath,$ORIGIN/../__hello_lib__"
            format!("-Wl,-rpath,$ORIGIN/{}", dir_str)
        };

        let mut has_rpath = false;
        let mut seen_rpaths: std::collections::HashSet<String> = std::collections::HashSet::new();
        if let Some(dirs) = get_var("runtime_library_search_directories") {
            for dir in iterate_value(dirs, eval) {
                if let Some(dir_str) = dir.unpack_str() {
                    if !dir_str.is_empty() {
                        let rpath = make_origin_rpath(dir_str);
                        if seen_rpaths.insert(rpath.clone()) {
                            args.push(heap.alloc_str(&rpath).to_value());
                        }
                        has_rpath = true;
                    }
                }
            }
        }
        // Fallback: use library_search_directories for rpath if no explicit rpath dirs
        if !has_rpath && !lib_search_dirs.is_empty() {
            for dir_str in &lib_search_dirs {
                let rpath = make_origin_rpath(dir_str);
                if seen_rpaths.insert(rpath.clone()) {
                    args.push(heap.alloc_str(&rpath).to_value());
                }
            }
        }

        Ok(heap.alloc(args))
    }

    /// Declares a compile output file.
    ///
    /// This function uses the real AnalysisActions from the ctx parameter
    /// to create a properly registered output artifact.
    fn declare_compile_output_file<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[starlark(require = named)] ctx: Value<'v>,
        #[starlark(require = named)] label: Value<'v>,
        #[starlark(require = named, default = "")] output_name: &str,
        #[starlark(require = named, default = NoneType)] configuration: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        let _ = (label, configuration); // Unused for now

        // Get the real actions from ctx.actions
        let actions_value = match ctx.get_attr("actions", heap) {
            Ok(Some(actions)) => actions,
            _ => {
                // Fallback to stub if no real actions available
                return Ok(heap.alloc(CtxCheatArtifactStub {
                    path: output_name.into(),
                    input_target: None,
                }));
            }
        };

        // Try to get the declare_file method
        let declare_file_method = match actions_value.get_attr("declare_file", heap) {
            Ok(Some(method)) => method,
            _ => {
                // Fallback to stub if declare_file not available
                return Ok(heap.alloc(CtxCheatArtifactStub {
                    path: output_name.into(),
                    input_target: None,
                }));
            }
        };

        // Call declare_file(output_name) using Starlark's function evaluation
        let filename = heap.alloc_str(output_name).to_value();
        match eval.eval_function(declare_file_method, &[filename], &[]) {
            Ok(artifact) => Ok(artifact),
            Err(_) => {
                // Fallback to stub on error
                Ok(heap.alloc(CtxCheatArtifactStub {
                    path: output_name.into(),
                    input_target: None,
                }))
            }
        }
    }

    /// Declares an auxiliary output file (dwo, gcno, etc.).
    #[allow(unused_variables)]
    fn declare_other_output_file<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named)] actions: Value<'v>,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named)] source_file: Value<'v>,
        #[starlark(require = named, default = "")] extension: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        // TODO(cc_common): Implement other output declaration
        Ok(NoneType)
    }

    /// Checks if an artifact is a tree artifact.
    ///
    /// A tree artifact is a directory artifact whose contents are determined at
    /// execution time. In Bazel, tree artifacts have `is_directory=True`.
    fn is_tree_artifact<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        artifact: Value<'v>,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<bool> {
        let heap = eval.heap();
        // Check if artifact has is_directory attribute (Bazel's TreeArtifact indicator)
        if let Ok(Some(is_dir)) = artifact.get_attr("is_directory", heap) {
            if let Some(b) = is_dir.unpack_bool() {
                return Ok(b);
            }
        }
        // Check if the path ends with a directory marker (no extension and no dot in basename)
        if let Ok(Some(path_attr)) = artifact.get_attr("path", heap) {
            if let Some(path_str) = path_attr.unpack_str() {
                // Tree artifacts typically don't have file extensions
                if let Some(basename) = path_str.rsplit('/').next() {
                    if !basename.contains('.') && !basename.is_empty() {
                        // Heuristic: could be a tree artifact, but without extension
                        // we can't be sure. Default to false for safety.
                        return Ok(false);
                    }
                }
            }
        }
        Ok(false)
    }

    /// Computes the output name prefix directory.
    ///
    /// This returns the directory prefix for object files, typically `_objs/{purpose}`.
    /// In Bazel, this creates object files in a target-specific subdirectory.
    #[allow(unused_variables)]
    fn compute_output_name_prefix_dir<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named, default = NoneType)] configuration: Value<'v>,
        #[starlark(require = named, default = NoneType)] purpose: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<String> {
        // The purpose is typically the target name or a unique identifier.
        // Object files should go in `_objs/{purpose}/` directory.
        if purpose.is_none() {
            // No purpose specified, use a default
            return Ok("_objs".to_owned());
        }

        // Try to get a string value from purpose
        if let Some(purpose_str) = purpose.unpack_str() {
            // If purpose is empty string, return just "_objs" without trailing slash
            // to avoid double slashes like "_objs//main.o"
            if purpose_str.is_empty() {
                return Ok("_objs".to_owned());
            }
            return Ok(format!("_objs/{}", purpose_str));
        }

        // If purpose has a 'name' attribute (like a Label), use that
        if let Ok(Some(name)) = purpose.get_attr("name", eval.heap()) {
            if let Some(name_str) = name.unpack_str() {
                if name_str.is_empty() {
                    return Ok("_objs".to_owned());
                }
                return Ok(format!("_objs/{}", name_str));
            }
        }

        // Fallback: just use _objs
        Ok("_objs".to_owned())
    }

    /// Interns a string sequence variable value for efficiency.
    fn intern_string_sequence_variable_value<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        value: Value<'v>,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // For now, just return the value as-is
        // TODO(cc_common): Implement proper interning
        Ok(value)
    }

    /// Gets per-file compile options.
    ///
    /// In Bazel, per-file copts are specified via `--per_file_copt` flags with
    /// the format `regex_filter@flag1,flag2`. The regex is matched against the
    /// source file path, and matching flags are returned.
    ///
    /// For now, returns the global --copt/--cxxopt flags since per-file patterns
    /// are rarely used. The function signature is correct for rules_cc compatibility.
    fn per_file_copts<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[starlark(require = pos)] _cpp_configuration: Value<'v>,
        #[starlark(require = pos)] _source_file: Value<'v>,
        #[starlark(require = pos)] _label: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Return empty list — per-file copts are not commonly used.
        // Global --copt/--cxxopt flags are already applied in cc_common.compile().
        Ok(eval.heap().alloc(AllocList::EMPTY))
    }

    /// Checks access to private API (allowlist enforcement).
    #[allow(unused_variables)]
    fn check_private_api<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named)] allowlist: Value<'v>,
        #[starlark(require = named, default = 1)] depth: i32,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<bool> {
        // Always allow for now
        Ok(true)
    }

    /// Creates a HeaderInfo struct.
    #[allow(unused_variables)]
    fn create_header_info<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named, default = NoneType)] headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] modular_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] textual_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] header_module: Value<'v>,
        #[starlark(require = named, default = NoneType)] pic_header_module: Value<'v>,
        #[starlark(require = named, default = NoneType)] modular_public_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] modular_private_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] separate_module_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] separate_module: Value<'v>,
        #[starlark(require = named, default = NoneType)] separate_pic_module: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Return a HeaderInfo stub with the necessary attributes
        Ok(eval.heap().alloc(HeaderInfoStub))
    }

    /// Creates a HeaderInfo struct with dependency tracking.
    #[allow(unused_variables)]
    fn create_header_info_with_deps<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named, default = NoneType)] headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] modular_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] textual_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] deps: Value<'v>,
        #[starlark(require = named, default = NoneType)] header_info: Value<'v>,
        #[starlark(require = named, default = NoneType)] merged_deps: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TODO(cc_common): Implement proper HeaderInfo with deps
        Ok(eval.heap().alloc(HeaderInfoStub))
    }

    /// Creates a toolchain features object from CcToolchainConfigInfo.
    ///
    /// Called by rules_cc's cc_common.bzl which delegates to this native method.
    /// The returned object has `configure_features()` and
    /// `default_features_and_action_configs()` methods used by configure_features.bzl.
    #[allow(unused_variables)]
    fn cc_toolchain_features<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named)] toolchain_config_info: Value<'v>,
        #[starlark(require = named)] tools_directory: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<CcToolchainFeatures> {
        let _ = this;
        Ok(cc_toolchain_features_from_config_info(
            toolchain_config_info,
            tools_directory,
            eval.heap(),
        ))
    }

    /// Creates a solib symlink for a shared library artifact.
    ///
    /// In Bazel, this creates a symlink in the solib directory for dynamic linking.
    /// For now, returns the artifact unchanged since we don't need the symlink
    /// for local execution.
    #[allow(unused_variables)]
    fn solib_symlink_action<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named)] ctx: Value<'v>,
        #[starlark(require = named)] artifact: Value<'v>,
        #[starlark(require = named)] solib_directory: &str,
        #[starlark(require = named)] runtime_solib_dir_base: &str,
    ) -> starlark::Result<Value<'v>> {
        // Return the artifact unchanged - symlinks aren't needed for local execution
        Ok(artifact)
    }

    /// Returns the exec platform OS name.
    /// Used by cc_toolchain_config_info.bzl to tag the toolchain config.
    #[allow(unused_variables)]
    fn exec_os<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = pos)] ctx: Value<'v>,
    ) -> starlark::Result<String> {
        if cfg!(target_os = "linux") {
            Ok("linux".to_owned())
        } else if cfg!(target_os = "macos") {
            Ok("darwin".to_owned())
        } else if cfg!(target_os = "windows") {
            Ok("windows".to_owned())
        } else {
            Ok("unknown".to_owned())
        }
    }

    /// Returns the target platform OS name.
    #[allow(unused_variables)]
    fn target_os<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = pos)] ctx: Value<'v>,
    ) -> starlark::Result<String> {
        if cfg!(target_os = "linux") {
            Ok("linux".to_owned())
        } else if cfg!(target_os = "macos") {
            Ok("darwin".to_owned())
        } else if cfg!(target_os = "windows") {
            Ok("windows".to_owned())
        } else {
            Ok("unknown".to_owned())
        }
    }
}

// ============================================================================
// CcCommonModule - The main cc_common module
// ============================================================================

/// The cc_common module provides C/C++ compilation support.
///
/// This is Bazel's native module for C++ build configuration. For Bazel 9.0+,
/// most of the actual compilation logic is in pure Starlark (rules_cc), but
/// the native cc_common module provides low-level primitives.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcCommonModule;

impl Display for CcCommonModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cc_common")
    }
}

starlark_simple_value!(CcCommonModule);

#[starlark_value(type = "cc_common")]
impl<'v> StarlarkValue<'v> for CcCommonModule {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(cc_common_module_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        // Report which attributes exist for hasattr() checks
        matches!(
            attribute,
            "internal_DO_NOT_USE"
                | "create_cc_toolchain_config_info"
                | "get_tool_for_action"
                | "get_execution_requirements"
                | "action_is_enabled"
                | "get_memory_inefficient_command_line"
                | "get_environment_variables"
                | "empty_variables"
                | "do_not_use_tools_cpp_compiler_present"
                | "is_cc_toolchain_resolution_enabled_do_not_use"
                | "CcToolchainInfo"
                | "merge_compilation_contexts"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "do_not_use_tools_cpp_compiler_present" => Some(Value::new_bool(true)),
            "CcToolchainInfo" => Some(heap.alloc(CcToolchainInfoProvider)),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec![
            "internal_DO_NOT_USE".to_owned(),
            "create_cc_toolchain_config_info".to_owned(),
            "get_tool_for_action".to_owned(),
            "get_execution_requirements".to_owned(),
            "action_is_enabled".to_owned(),
            "get_memory_inefficient_command_line".to_owned(),
            "get_environment_variables".to_owned(),
            "empty_variables".to_owned(),
            "do_not_use_tools_cpp_compiler_present".to_owned(),
            "is_cc_toolchain_resolution_enabled_do_not_use".to_owned(),
            "CcToolchainInfo".to_owned(),
            "merge_compilation_contexts".to_owned(),
        ]
    }
}

/// Methods on the cc_common module.
#[starlark_module]
fn cc_common_module_methods(builder: &mut MethodsBuilder) {
    /// Returns the internal cc_common API struct.
    ///
    /// Used by rules_cc via: cc_internal = cc_common.internal_DO_NOT_USE()
    #[starlark(attribute)]
    fn internal_DO_NOT_USE(this: &CcCommonModule) -> starlark::Result<CcCommonInternal> {
        let _ = this;
        Ok(CcCommonInternal)
    }

    /// Provider callable used by rules_cc's cc_binary for the launcher marker info.
    ///
    /// rules_cc reads `_CcLauncherInfo = cc_common.launcher_provider` at module
    /// load time. We return `ExecutionInfoProvider` as a placeholder — slug
    /// does not consume the returned provider value for any behaviour yet.
    #[starlark(attribute)]
    fn launcher_provider(this: &CcCommonModule) -> starlark::Result<ExecutionInfoProvider> {
        let _ = this;
        Ok(ExecutionInfoProvider)
    }

    /// Returns whether C++ toolchain resolution is enabled.
    ///
    /// In Bazel 9.0+, this always returns True (toolchain resolution is the default).
    /// Used by rules_cc's find_cc_toolchain() to determine whether to use
    /// ctx.toolchains[CC_TOOLCHAIN_TYPE] (modern) or ctx.attr._cc_toolchain (legacy).
    #[allow(unused_variables)]
    fn is_cc_toolchain_resolution_enabled_do_not_use<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] ctx: Value<'v>,
    ) -> starlark::Result<bool> {
        // Slug always uses toolchain resolution (Bazel 9.0+ behavior)
        Ok(true)
    }

    /// Returns an empty CC variables object.
    ///
    /// Used as a default argument for cc_common.get_memory_inefficient_command_line()
    /// and other functions that accept a Variables parameter.
    #[allow(unused_variables)]
    fn empty_variables<'v>(
        #[starlark(this)] this: &CcCommonModule,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Return an empty dict as the variables object
        Ok(heap.alloc(starlark::values::dict::Dict::default()))
    }

    /// Configures C++ features based on toolchain and requested features.
    ///
    /// Returns a FeatureConfiguration that controls which compiler flags are enabled.
    #[allow(unused_variables)]
    fn configure_features<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] ctx: Value<'v>,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named, default = NoneType)] requested_features: Value<'v>,
        #[starlark(require = named, default = NoneType)] unsupported_features: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<FeatureConfiguration> {
        let _ = (this, ctx, cc_toolchain);
        let heap = eval.heap();

        // Collect requested features from the list
        let mut req: Vec<String> = Vec::new();
        if !requested_features.is_none() {
            if let Ok(iter) = requested_features.iterate(heap) {
                for item in iter {
                    if let Some(s) = item.unpack_str() {
                        req.push(s.to_owned());
                    }
                }
            }
        }

        // Also include global --features from command line
        for feat in crate::interpreter::rule_defs::build_config::get_features() {
            if !req.contains(&feat) {
                req.push(feat);
            }
        }

        // Collect unsupported features from the list
        let mut unsup: Vec<String> = Vec::new();
        if !unsupported_features.is_none() {
            if let Ok(iter) = unsupported_features.iterate(heap) {
                for item in iter {
                    if let Some(s) = item.unpack_str() {
                        unsup.push(s.to_owned());
                    }
                }
            }
        }

        if let Ok(Some(toolchain_features)) = cc_toolchain.get_attr("_toolchain_features", heap) {
            if let Some(features) = toolchain_features.downcast_ref::<CcToolchainFeatures>() {
                return Ok(features.configure(req, unsup));
            }
        }

        Ok(FeatureConfiguration::new(req, unsup))
    }

    /// Compiles C/C++ source files.
    ///
    /// This is the main compilation function that creates compile actions for each
    /// source file and returns compilation context and outputs.
    ///
    /// Returns a tuple of (CcCompilationContext, CompilationOutputs).
    #[allow(unused_variables)]
    fn compile<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] actions: Value<'v>,
        #[starlark(require = named)] name: &str,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named, default = NoneType)] srcs: Value<'v>,
        #[starlark(require = named, default = NoneType)] public_hdrs: Value<'v>,
        #[starlark(require = named, default = NoneType)] private_hdrs: Value<'v>,
        #[starlark(require = named, default = NoneType)] textual_hdrs: Value<'v>,
        #[starlark(require = named, default = NoneType)] additional_inputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] loose_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] quote_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] system_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] framework_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] defines: Value<'v>,
        #[starlark(require = named, default = NoneType)] local_defines: Value<'v>,
        #[starlark(require = named, default = NoneType)] include_prefix: Value<'v>,
        #[starlark(require = named, default = NoneType)] strip_include_prefix: Value<'v>,
        #[starlark(require = named, default = NoneType)] user_compile_flags: Value<'v>,
        #[starlark(require = named, default = NoneType)] conly_flags: Value<'v>,
        #[starlark(require = named, default = NoneType)] cxx_flags: Value<'v>,
        #[starlark(require = named, default = NoneType)] compilation_contexts: Value<'v>,
        #[starlark(require = named, default = NoneType)] implementation_compilation_contexts: Value<
            'v,
        >,
        #[starlark(require = named, default = false)] disallow_pic_outputs: bool,
        #[starlark(require = named, default = false)] disallow_nopic_outputs: bool,
        #[starlark(require = named, default = NoneType)] additional_include_scanning_roots: Value<
            'v,
        >,
        #[starlark(require = named, default = false)] do_not_generate_module_map: bool,
        #[starlark(require = named, default = false)] code_coverage_enabled: bool,
        #[starlark(require = named, default = NoneType)] hdrs_checking_mode: Value<'v>,
        #[starlark(require = named, default = NoneType)] variables_extension: Value<'v>,
        #[starlark(require = named, default = NoneType)] language: Value<'v>,
        #[starlark(require = named, default = NoneType)] purpose: Value<'v>,
        #[starlark(require = named, default = NoneType)] copts_filter: Value<'v>,
        #[starlark(require = named, default = NoneType)] separate_module_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] module_interfaces: Value<'v>,
        #[starlark(require = named, default = NoneType)] non_compilation_additional_inputs: Value<
            'v,
        >,
        #[starlark(kwargs)] kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();

        // Collect source files to compile
        let mut object_files: Vec<Value<'v>> = Vec::new();
        let mut pic_object_files: Vec<Value<'v>> = Vec::new();

        // Collect extra compiler flags from parameters
        let mut extra_flags: Vec<String> = Vec::new();

        // Add include directories
        for (flag, val) in &[
            ("-I", includes),
            ("-iquote", quote_includes),
            ("-isystem", system_includes),
            ("-F", framework_includes),
        ] {
            if !val.is_none() {
                for elem in depset_values(*val, heap)? {
                    let dir = elem.to_str();
                    if !dir.is_empty() {
                        extra_flags.push(flag.to_string());
                        extra_flags.push(dir.to_string());
                    }
                }
            }
        }

        // Add defines
        for def_val in &[defines, local_defines] {
            if !def_val.is_none() {
                for elem in depset_values(*def_val, heap)? {
                    let d = elem.to_str();
                    if !d.is_empty() {
                        extra_flags.push(format!("-D{}", d));
                    }
                }
            }
        }

        // Add user compile flags
        if !user_compile_flags.is_none() {
            if let Ok(iter) = user_compile_flags.iterate(heap) {
                for flag in iter {
                    if let Some(s) = flag.unpack_str() {
                        extra_flags.push(s.to_owned());
                    }
                }
            }
        }

        // Add global --copt flags from command line
        for opt in crate::interpreter::rule_defs::build_config::get_copts() {
            extra_flags.push(opt);
        }

        // Get the declare_file method from actions
        let declare_file_method = actions.get_attr("declare_file", heap).ok().flatten();
        let run_method = actions.get_attr("run", heap).ok().flatten();

        // Propagate include directories from `compilation_contexts` (the
        // CcCompilationContext from each dep's CcInfo). The cc_library
        // pattern is to set `includes = ["include"]` on a hub target
        // (e.g. `:config` in @llvm-project//llvm:Demangle's transitive
        // closure) and have dependents pick up the resulting `-I` via
        // CcCompilationContext propagation. Without adding these to the
        // compile command line, the action's source `#include
        // "llvm/Demangle/Demangle.h"` resolves to nothing on RE workers
        // even when the header is in the action's input tree, because
        // the `-I` path that maps the include name to the file is
        // missing.
        //
        // Per Plan 29: include dirs from `compilation_contexts` (the
        // CcCompilationContext from each dep's CcInfo) flow into this
        // target's compile commands as `extra_flags` only — no global
        // registry. Bazel's CcCompilationContext is `@Immutable`, every
        // include-dir field is a `Depset<PathFragment>`, and propagation
        // between targets is exclusively by depset transitivity. We
        // match that here.
        if !compilation_contexts.is_none() {
            if let Ok(iter) = compilation_contexts.iterate(heap) {
                for ctx in iter {
                    for (attr_name, flag) in &[
                        ("includes", "-I"),
                        ("system_includes", "-isystem"),
                        ("quote_includes", "-iquote"),
                    ] {
                        if let Ok(Some(includes_val)) = ctx.get_attr(attr_name, heap) {
                            if !includes_val.is_none() {
                                for elem in depset_values(includes_val, heap)? {
                                    let dir = elem.to_str();
                                    if !dir.is_empty() {
                                        extra_flags.push(flag.to_string());
                                        extra_flags.push(dir.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Handle strip_include_prefix.
        //
        // Bazel: `strip_include_prefix = X` on a cc_library means headers can
        // be `#include`d with the `X/` component stripped. If X is relative
        // (no leading `/`), it's interpreted relative to the package.
        //
        // Concrete example: `@llvm-project//third-party/siphash:siphash` has
        //   hdrs = ["include/siphash/SipHash.h"]
        //   strip_include_prefix = "include"
        // A dependent with `#include "siphash/SipHash.h"` needs an include
        // path that points at `.../third-party/siphash/include/` so the
        // `include/` prefix is stripped.
        //
        // Register the resulting include dir globally (for compiles in the
        // same analysis session) and also expose it via the returned
        // `CcCompilationContext.includes` so dependents picking up this
        // target through `compilation_contexts` see it too. Package
        // directory is derived from a hdr's source artifact (iterates
        // public_hdrs / private_hdrs / textual_hdrs); falls back to srcs.
        let mut strip_include_dir: Option<String> = None;
        if let Some(strip_prefix) = strip_include_prefix.unpack_str() {
            if !strip_prefix.is_empty() {
                let trimmed_prefix = strip_prefix.trim_start_matches('/');

                let mut sample_hdr_path: Option<String> = None;
                for candidate in &[public_hdrs, private_hdrs, textual_hdrs, srcs] {
                    if candidate.is_none() {
                        continue;
                    }
                    if let Ok(iter) = candidate.iterate(heap) {
                        for hdr_tuple in iter {
                            let hdr = hdr_tuple
                                .at(heap.alloc(0i32).to_value(), heap)
                                .unwrap_or(hdr_tuple);
                            if let Some(hdr_path_raw) = hdr
                                .get_attr("path", heap)
                                .ok()
                                .flatten()
                                .and_then(|v| v.unpack_str())
                            {
                                let normalized;
                                let hdr_path: &str =
                                    if let Some(n) = normalize_external_cells_path(hdr_path_raw) {
                                        normalized = n;
                                        &normalized
                                    } else {
                                        hdr_path_raw
                                    };
                                sample_hdr_path = Some(hdr_path.to_owned());
                                break;
                            }
                        }
                    }
                    if sample_hdr_path.is_some() {
                        break;
                    }
                }

                if let Some(hdr_path) = sample_hdr_path {
                    // Find the trailing `<strip_prefix>/` segment in the hdr
                    // path and take everything up to and including it as the
                    // include root. Works for both external (`external/<repo>/<pkg>/<prefix>/...`)
                    // and root-cell (`<pkg>/<prefix>/...`) layouts.
                    let needle = format!("/{}/", trimmed_prefix);
                    let include_dir_owned: Option<String> =
                        if let Some(idx) = hdr_path.rfind(&needle) {
                            Some(hdr_path[..idx + needle.len() - 1].to_owned())
                        } else if hdr_path.starts_with(&format!("{}/", trimmed_prefix)) {
                            Some(trimmed_prefix.to_owned())
                        } else {
                            None
                        };
                    if let Some(dir) = include_dir_owned {
                        // Per Plan 29: strip_include_prefix dir flows to
                        // dependents via this target's CcCompilationContext
                        // .includes (merged in below at the depset
                        // construction site). No global registry needed.
                        strip_include_dir = Some(dir);
                    }
                }
            }
        }

        // Collect transitive header artifacts so compile actions declare them
        // as inputs. Without this, generated headers referenced via `-I` flags
        // (e.g. `:abi_breaking_h_gen` in @llvm-project//llvm) are not scheduled
        // before the consuming compile action runs — the compile fails with
        // `fatal error: llvm/Config/abi-breaking.h: No such file or directory`.
        //
        // Sources:
        //   - `public_hdrs` / `private_hdrs` / `textual_hdrs` from this call
        //     (may include declared-output artifacts from `attr.output` like
        //     `:abi_breaking_h_gen`)
        //   - `headers` depset from each dep's `CcCompilationContext`
        // `cc_helper._get_public_hdrs(ctx)` returns `[(artifact, label),
        // ...]` tuples (see rules_cc cc_helper.bzl `_map_to_list`). The
        // `actions.run(inputs=...)` handler in
        // `slug_action_impl::context::run` only downcasts list items to
        // `StarlarkArtifact` / `StarlarkDeclaredArtifact`; tuple entries
        // silently drop out of the action's declared input set. Local
        // execution still found the headers via the project filesystem,
        // but RE strictly enforces declared inputs and the compile
        // failed with `fatal error: <hdr>: No such file or directory`.
        //
        // Two shapes need handling:
        //   1. plain `list` of `(artifact, label)` tuples — what
        //      `_get_public_hdrs` returns. Iterate those directly.
        //   2. `depset` of bare artifacts — what
        //      `CcCompilationContext.headers` is built from.
        //
        // After collecting, unwrap any tuple to its first element so the
        // artifact lands in `compile_inputs` (no-op for bare artifacts).
        let unwrap_artifact_tuple =
            |v: Value<'v>| -> Value<'v> { v.at(heap.alloc(0i32).to_value(), heap).unwrap_or(v) };
        let collect_hdr_value =
            |hdr_val: Value<'v>, out: &mut Vec<Value<'v>>| -> starlark::Result<()> {
                for h in depset_or_iterable_values(hdr_val, heap)? {
                    out.push(unwrap_artifact_tuple(h));
                }
                Ok(())
            };
        let mut compile_inputs: Vec<Value<'v>> = Vec::new();
        for hdr_val in &[public_hdrs, private_hdrs, textual_hdrs] {
            collect_hdr_value(*hdr_val, &mut compile_inputs)?;
        }
        if !compilation_contexts.is_none() {
            if let Ok(iter) = compilation_contexts.iterate(heap) {
                for ctx in iter {
                    if let Ok(Some(headers)) = ctx.get_attr("headers", heap) {
                        compile_inputs.extend(depset_to_artifact_inputs(headers, heap)?);
                    }
                }
            }
        }
        let compile_inputs_value: Value<'v> = if compile_inputs.is_empty() {
            Value::new_none()
        } else {
            heap.alloc(compile_inputs.clone())
        };

        // Process source files if provided
        // srcs is a list of (Artifact, Label) tuples from cc_helper.get_srcs()
        if !srcs.is_none() {
            // Try to iterate over srcs
            if let Ok(iter) = srcs.iterate(heap) {
                let items: Vec<_> = iter.collect();
                for src_tuple in items {
                    // Extract the artifact from the (Artifact, Label) tuple
                    // Try tuple index first, then fall back to treating it as artifact directly
                    let src = src_tuple
                        .at(heap.alloc(0i32).to_value(), heap)
                        .unwrap_or(src_tuple);

                    // Get source file path
                    let src_path = src
                        .get_attr("path", heap)
                        .ok()
                        .flatten()
                        .and_then(|v| v.unpack_str())
                        .unwrap_or("unknown.c");

                    // Per Plan 29: source-path-derived dirs (the per-target
                    // `external/<repo>` and `<repo>/src/` heuristics) are
                    // already added to the action's args_vec by
                    // `create_cc_compile_action`'s per-source loop — no
                    // need to register them globally for cross-target
                    // visibility, which was the bug Plan 29 retired.

                    // Determine output filename (replace extension with .o)
                    let basename = src_path.rsplit('/').next().unwrap_or(src_path);
                    let output_name = if let Some(dot_pos) = basename.rfind('.') {
                        format!("_objs/{}/{}.o", name, &basename[..dot_pos])
                    } else {
                        format!("_objs/{}/{}.o", name, basename)
                    };
                    let pic_output_name = if let Some(dot_pos) = basename.rfind('.') {
                        format!("_objs/{}/{}.pic.o", name, &basename[..dot_pos])
                    } else {
                        format!("_objs/{}/{}.pic.o", name, basename)
                    };

                    // Declare output files
                    if let Some(declare_file) = declare_file_method {
                        // Regular object file
                        let output_file = eval.eval_function(
                            declare_file,
                            &[heap.alloc_str(&output_name).to_value()],
                            &[],
                        );
                        let output_file = output_file.ok();

                        // PIC object file
                        let pic_output_file = eval
                            .eval_function(
                                declare_file,
                                &[heap.alloc_str(&pic_output_name).to_value()],
                                &[],
                            )
                            .ok();

                        // Register compile action if run method available
                        if let (Some(run), Some(out), Some(pic_out)) =
                            (run_method, output_file, pic_output_file)
                        {
                            // Get output as output artifact
                            let output_artifact = out
                                .get_attr("as_output", heap)
                                .ok()
                                .flatten()
                                .and_then(|method| eval.eval_function(method, &[], &[]).ok())
                                .unwrap_or(out);
                            let pic_output_artifact = pic_out
                                .get_attr("as_output", heap)
                                .ok()
                                .flatten()
                                .and_then(|method| eval.eval_function(method, &[], &[]).ok())
                                .unwrap_or(pic_out);

                            // Build compile command: <compiler> [flags] -c src -o output
                            let host_compiler = match std::env::consts::OS {
                                "windows" => "cl.exe",
                                "macos" => "/usr/bin/clang",
                                _ => "/usr/bin/gcc",
                            };

                            // Determine C vs C++ specific flags
                            let is_cxx = src_path.ends_with(".cc")
                                || src_path.ends_with(".cpp")
                                || src_path.ends_with(".cxx");

                            let mut args_vec: Vec<Value<'v>> = Vec::new();
                            args_vec.push(heap.alloc_str(host_compiler).to_value());

                            // Add extra flags (includes, defines, user flags)
                            for flag in &extra_flags {
                                args_vec.push(heap.alloc_str(flag).to_value());
                            }

                            // Add C-only or C++-only flags (from function params + CLI)
                            if is_cxx {
                                if !cxx_flags.is_none() {
                                    if let Ok(iter) = cxx_flags.iterate(heap) {
                                        for flag in iter {
                                            args_vec.push(flag);
                                        }
                                    }
                                }
                                for opt in
                                    crate::interpreter::rule_defs::build_config::get_cxxopts()
                                {
                                    args_vec.push(heap.alloc_str(&opt).to_value());
                                }
                            } else {
                                if !conly_flags.is_none() {
                                    if let Ok(iter) = conly_flags.iterate(heap) {
                                        for flag in iter {
                                            args_vec.push(flag);
                                        }
                                    }
                                }
                                for opt in
                                    crate::interpreter::rule_defs::build_config::get_conlyopts()
                                {
                                    args_vec.push(heap.alloc_str(&opt).to_value());
                                }
                            }

                            args_vec.push(heap.alloc_str("-c").to_value());
                            args_vec.push(src);
                            args_vec.push(heap.alloc_str("-o").to_value());
                            args_vec.push(output_artifact);
                            let args = heap.alloc(args_vec);
                            let outputs_list = heap.alloc(vec![output_artifact]);
                            let progress = heap
                                .alloc_str(&format!("Compiling {}", basename))
                                .to_value();

                            // Call actions.run() for regular compile
                            // Use unique identifier to avoid "multiple actions with same category" error
                            let identifier = heap.alloc_str(&format!("{}.o", basename)).to_value();
                            let mut run_kwargs: Vec<(&str, Value<'v>)> = vec![
                                ("outputs", outputs_list),
                                ("category", heap.alloc_str("cpp_compile").to_value()),
                                ("identifier", identifier),
                                ("progress_message", progress),
                            ];
                            if !compile_inputs_value.is_none() {
                                run_kwargs.push(("inputs", compile_inputs_value));
                            }
                            let run_result = eval.eval_function(run, &[args], &run_kwargs);
                            // Register PIC compile action with unique identifier
                            let mut pic_args_vec: Vec<Value<'v>> = Vec::new();
                            pic_args_vec.push(heap.alloc_str(host_compiler).to_value());
                            for flag in &extra_flags {
                                pic_args_vec.push(heap.alloc_str(flag).to_value());
                            }
                            if is_cxx {
                                if !cxx_flags.is_none() {
                                    if let Ok(iter) = cxx_flags.iterate(heap) {
                                        for flag in iter {
                                            pic_args_vec.push(flag);
                                        }
                                    }
                                }
                                for opt in
                                    crate::interpreter::rule_defs::build_config::get_cxxopts()
                                {
                                    pic_args_vec.push(heap.alloc_str(&opt).to_value());
                                }
                            } else {
                                if !conly_flags.is_none() {
                                    if let Ok(iter) = conly_flags.iterate(heap) {
                                        for flag in iter {
                                            pic_args_vec.push(flag);
                                        }
                                    }
                                }
                                for opt in
                                    crate::interpreter::rule_defs::build_config::get_conlyopts()
                                {
                                    pic_args_vec.push(heap.alloc_str(&opt).to_value());
                                }
                            }
                            pic_args_vec.push(heap.alloc_str("-c").to_value());
                            pic_args_vec.push(heap.alloc_str("-fPIC").to_value());
                            pic_args_vec.push(src);
                            pic_args_vec.push(heap.alloc_str("-o").to_value());
                            pic_args_vec.push(pic_output_artifact);
                            let pic_args = heap.alloc(pic_args_vec);
                            let pic_outputs_list = heap.alloc(vec![pic_output_artifact]);
                            let pic_progress = heap
                                .alloc_str(&format!("Compiling {} (PIC)", basename))
                                .to_value();
                            let pic_identifier =
                                heap.alloc_str(&format!("{}.pic.o", basename)).to_value();

                            let mut pic_run_kwargs: Vec<(&str, Value<'v>)> = vec![
                                ("outputs", pic_outputs_list),
                                ("category", heap.alloc_str("cpp_compile").to_value()),
                                ("identifier", pic_identifier),
                                ("progress_message", pic_progress),
                            ];
                            if !compile_inputs_value.is_none() {
                                pic_run_kwargs.push(("inputs", compile_inputs_value));
                            }
                            let _ = eval.eval_function(run, &[pic_args], &pic_run_kwargs);

                            object_files.push(out);
                            pic_object_files.push(pic_out);
                        }
                    }
                }
            }
        }

        // Create compilation context with the provided headers/includes/defines
        // Merge public_hdrs, private_hdrs, and textual_hdrs into headers depset
        let merged_headers =
            if public_hdrs.is_none() && private_hdrs.is_none() && textual_hdrs.is_none() {
                Value::new_none()
            } else {
                let mut direct: Vec<Value<'v>> = Vec::new();
                for hdr_val in &[public_hdrs, private_hdrs, textual_hdrs] {
                    if !hdr_val.is_none() {
                        if let Ok(iter) = hdr_val.iterate(heap) {
                            for h in iter {
                                direct.push(h);
                            }
                        }
                    }
                }
                if direct.is_empty() {
                    Value::new_none()
                } else {
                    crate::interpreter::rule_defs::depset::make_depset_from_lists(
                        heap,
                        direct,
                        Vec::new(),
                        "default",
                    )
                    .unwrap_or(Value::new_none())
                }
            };

        // If strip_include_prefix produced an include root, append it to
        // the includes depset so that dependents consuming this target's
        // CcInfo get the include path via standard CcCompilationContext
        // propagation. This is the only correct cross-target propagation
        // mechanism — see Plan 29 for the rationale.
        let includes = if let Some(ref dir) = strip_include_dir {
            let mut direct: Vec<Value<'v>> = vec![heap.alloc_str(dir).to_value()];
            if !includes.is_none() {
                for v in depset_values(includes, heap)? {
                    direct.push(v);
                }
            }
            crate::interpreter::rule_defs::depset::make_depset_from_lists(
                heap,
                direct,
                Vec::new(),
                "default",
            )
            .unwrap_or(includes)
        } else {
            includes
        };

        let compilation_context = heap.alloc(CcCompilationContextGen {
            headers: merged_headers,
            includes,
            quote_includes,
            system_includes,
            external_includes: Value::new_none(),
            framework_includes,
            defines,
            local_defines,
        });

        // Create immutable compilation-output sequences. These support len()
        // and iteration while remaining valid inside provider/depset values.
        let objects_list = cc_internal_freeze_values(object_files.clone(), heap)?;
        let pic_objects_list = cc_internal_freeze_values(pic_object_files.clone(), heap)?;
        let compilation_outputs = heap.alloc(CompilationOutputsGen {
            objects: objects_list,
            pic_objects: pic_objects_list,
        });

        // Return tuple of (compilation_context, compilation_outputs)
        Ok(heap.alloc((compilation_context, compilation_outputs)))
    }

    /// Links C++ code into a binary or shared library.
    ///
    /// This is the core linking function that rules_cc calls to create
    /// executables and shared libraries from compilation outputs.
    #[allow(unused_variables)]
    fn link<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] actions: Value<'v>,
        #[starlark(require = named)] name: &str,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named, default = "c++")] language: &str,
        #[starlark(require = named, default = "executable")] output_type: &str,
        #[starlark(require = named, default = true)] link_deps_statically: bool,
        #[starlark(require = named, default = NoneType)] compilation_outputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] linking_contexts: Value<'v>,
        #[starlark(require = named, default = NoneType)] user_link_flags: Value<'v>,
        #[starlark(require = named, default = 0)] stamp: i32,
        #[starlark(require = named, default = NoneType)] additional_inputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] additional_outputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] variables_extension: Value<'v>,
        #[starlark(require = named, default = NoneType)] grep_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] main_output: Value<'v>,
        #[starlark(require = named, default = NoneType)] use_test_only_flags: Value<'v>,
        #[starlark(require = named, default = NoneType)] pdb_file: Value<'v>,
        #[starlark(require = named, default = NoneType)] win_def_file: Value<'v>,
        #[starlark(kwargs)] kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();

        // Get the declare_file and run methods from actions
        let declare_file_method = actions.get_attr("declare_file", heap).ok().flatten();
        let run_method = actions.get_attr("run", heap).ok().flatten();

        // Determine action name based on output type
        let action_name = match output_type {
            "dynamic_library" => "c++-link-dynamic-library",
            "static_library" => "c++-link-static-library",
            _ => "c++-link-executable",
        };

        // Determine output extension based on output type and platform
        let is_dynamic = output_type == "dynamic_library";
        let is_static = output_type == "static_library";
        let output_ext = if is_static {
            if is_windows_host() { ".lib" } else { ".a" }
        } else if is_dynamic {
            if is_windows_host() {
                ".dll"
            } else if std::env::consts::OS == "macos" {
                ".dylib"
            } else {
                ".so"
            }
        } else {
            if is_windows_host() { ".exe" } else { "" }
        };

        let output_name = format!("{}{}", name, output_ext);

        // Declare output file
        let output_file = if let Some(declare_file) = declare_file_method {
            eval.eval_function(
                declare_file,
                &[heap.alloc_str(&output_name).to_value()],
                &[],
            )
            .ok()
        } else {
            None
        };

        if let (Some(run), Some(out)) = (run_method, output_file) {
            let output_artifact = out
                .get_attr("as_output", heap)
                .ok()
                .flatten()
                .and_then(|method| eval.eval_function(method, &[], &[]).ok())
                .unwrap_or(out);

            // Get output path as string for MSVC /OUT: flag
            let output_path_str = out
                .get_attr("path", heap)
                .ok()
                .flatten()
                .and_then(|v| v.unpack_str().map(|s| s.to_owned()))
                .unwrap_or_else(|| output_name.clone());

            let direct_additional_inputs = if additional_inputs.is_none() {
                Vec::new()
            } else {
                depset_or_iterable_values(additional_inputs, heap)?
            };
            let expanded_user_link_flags = if user_link_flags.is_none() {
                user_link_flags
            } else {
                let mut flags = Vec::new();
                for flag in depset_or_iterable_values(user_link_flags, heap)? {
                    flags.push(cc_expand_link_flag_locations(
                        flag,
                        &direct_additional_inputs,
                        heap,
                    )?);
                }
                heap.alloc(AllocList(flags))
            };

            let mut link_vars: SmallMap<Value<'v>, Value<'v>> = SmallMap::new();
            link_vars.insert_hashed(
                heap.alloc_str("output_execpath")
                    .to_value()
                    .get_hashed()
                    .unwrap(),
                heap.alloc_str(&output_path_str).to_value(),
            );
            insert_if_set(
                &mut link_vars,
                heap,
                "user_link_flags",
                expanded_user_link_flags,
            );
            if is_dynamic {
                link_vars.insert_hashed(
                    heap.alloc_str("is_linking_dynamic_library")
                        .to_value()
                        .get_hashed()
                        .unwrap(),
                    Value::new_bool(true),
                );
            }
            if let Some(target_system_name) = cc_toolchain_target_system_name(cc_toolchain, heap) {
                insert_if_set(
                    &mut link_vars,
                    heap,
                    "target_system_name",
                    target_system_name,
                );
            }
            if let Some(target_libc) = cc_toolchain_target_libc(cc_toolchain, heap) {
                insert_if_set(&mut link_vars, heap, "target_libc", target_libc);
            }
            if !variables_extension.is_none()
                && let Some(dict_ref) = DictRef::from_value(variables_extension)
            {
                for (key, value) in dict_ref.iter() {
                    if let Ok(hashed) = key.get_hashed() {
                        link_vars.insert_hashed(hashed, value);
                    }
                }
            }
            let link_variables = heap.alloc(Dict::new(link_vars));
            let feature_args = feature_configuration
                .downcast_ref::<FeatureConfiguration>()
                .map(|fc| expand_cc_flag_sets(fc, action_name, link_variables, heap))
                .transpose()?
                .unwrap_or_default();
            let has_feature_args = !feature_args.is_empty();

            // Get linker tool path
            let linker_tool = match std::env::consts::OS {
                "windows" => {
                    let msvc = get_msvc_tool_paths();
                    if is_static {
                        msvc.as_ref()
                            .map(|t| t.lib.clone())
                            .unwrap_or_else(|| "lib.exe".to_owned())
                    } else {
                        msvc.as_ref()
                            .map(|t| t.link.clone())
                            .unwrap_or_else(|| "link.exe".to_owned())
                    }
                }
                "macos" => {
                    if is_static {
                        "/usr/bin/ar".to_owned()
                    } else {
                        "/usr/bin/clang++".to_owned()
                    }
                }
                _ => {
                    if is_static {
                        "/usr/bin/ar".to_owned()
                    } else {
                        "/usr/bin/g++".to_owned()
                    }
                }
            };

            // Build link command arguments
            let mut args: Vec<Value<'v>> = Vec::new();
            args.push(heap.alloc_str(&linker_tool).to_value());

            if is_static {
                if !has_feature_args {
                    // Static library fallback: ar rcs output.a obj1.o obj2.o ...
                    if !is_windows_host() {
                        args.push(heap.alloc_str("rcs").to_value());
                    }
                    args.push(output_artifact);
                    if is_windows_host() {
                        // MSVC lib.exe: /OUT:output.lib obj1.obj obj2.obj
                        // Replace the last push with /OUT: flag
                        args.pop();
                        let out_flag = format!("/OUT:{}", output_path_str);
                        args.push(heap.alloc_str(&out_flag).to_value());
                    }
                }
            } else {
                // Executable or shared library
                if is_windows_host() {
                    args.push(
                        heap.alloc_str(&format!("/OUT:{}", output_path_str))
                            .to_value(),
                    );
                    if is_dynamic {
                        args.push(heap.alloc_str("/DLL").to_value());
                    }
                } else {
                    args.push(heap.alloc_str("-o").to_value());
                    args.push(output_artifact);
                    if is_dynamic {
                        args.push(heap.alloc_str("-shared").to_value());
                        if is_compiler_rt_crtbegin_link_output(Some(&output_path_str)) {
                            args.push(heap.alloc_str("-nostdlib").to_value());
                        }
                    }
                }
            }

            args.extend(feature_args);
            let mut run_inputs = direct_additional_inputs.clone();

            // Collect object files from compilation_outputs
            if !compilation_outputs.is_none() {
                // Try objects attribute first (regular objects)
                if let Ok(Some(objects)) = compilation_outputs.get_attr("objects", heap) {
                    if !objects.is_none() {
                        if let Ok(iter) = objects.iterate(heap) {
                            for obj in iter {
                                args.push(obj);
                            }
                        }
                    }
                }
                // Also try pic_objects if no regular objects
                if let Ok(Some(pic_objects)) = compilation_outputs.get_attr("pic_objects", heap) {
                    if !pic_objects.is_none() {
                        if let Ok(iter) = pic_objects.iterate(heap) {
                            for obj in iter {
                                args.push(obj);
                            }
                        }
                    }
                }
            }

            // Collect linker inputs from linking_contexts
            if !linking_contexts.is_none() {
                if let Ok(iter) = linking_contexts.iterate(heap) {
                    for ctx_val in iter {
                        // Each linking_context has linker_inputs (a depset)
                        if let Ok(Some(linker_inputs)) = ctx_val.get_attr("linker_inputs", heap) {
                            if !linker_inputs.is_none() {
                                for input in depset_values(linker_inputs, heap)? {
                                    // Each linker input may have libraries
                                    if let Ok(Some(libraries)) = input.get_attr("libraries", heap) {
                                        if !libraries.is_none() {
                                            // Libraries can be a depset or list
                                            for lib in depset_or_iterable_values(libraries, heap)? {
                                                // Library_to_link has static_library, dynamic_library, objects, etc.
                                                // Respect link_deps_statically to choose between static/dynamic.
                                                let mut linked = false;
                                                if link_deps_statically {
                                                    // Prefer static_library when linking statically
                                                    if let Ok(Some(static_lib)) =
                                                        lib.get_attr("static_library", heap)
                                                    {
                                                        if !static_lib.is_none() {
                                                            args.push(static_lib);
                                                            linked = true;
                                                        }
                                                    }
                                                    // Fallback to pic_static_library
                                                    if !linked {
                                                        if let Ok(Some(pic_static_lib)) =
                                                            lib.get_attr("pic_static_library", heap)
                                                        {
                                                            if !pic_static_lib.is_none() {
                                                                args.push(pic_static_lib);
                                                                linked = true;
                                                            }
                                                        }
                                                    }
                                                } else {
                                                    // Prefer dynamic_library when linking dynamically
                                                    if let Ok(Some(dynamic_lib)) =
                                                        lib.get_attr("dynamic_library", heap)
                                                    {
                                                        if !dynamic_lib.is_none() {
                                                            args.push(dynamic_lib);
                                                            linked = true;
                                                        }
                                                    }
                                                    // Fallback: interface_library (import lib on Windows)
                                                    if !linked {
                                                        if let Ok(Some(iface_lib)) =
                                                            lib.get_attr("interface_library", heap)
                                                        {
                                                            if !iface_lib.is_none() {
                                                                args.push(iface_lib);
                                                                linked = true;
                                                            }
                                                        }
                                                    }
                                                    // Fallback to static_library if no dynamic available
                                                    if !linked {
                                                        if let Ok(Some(static_lib)) =
                                                            lib.get_attr("static_library", heap)
                                                        {
                                                            if !static_lib.is_none() {
                                                                args.push(static_lib);
                                                                linked = true;
                                                            }
                                                        }
                                                    }
                                                }
                                                // Also include objects from library_to_link
                                                // (only if no library was found above)
                                                if !linked {
                                                    if let Ok(Some(objects)) =
                                                        lib.get_attr("objects", heap)
                                                    {
                                                        if !objects.is_none() {
                                                            if let Ok(obj_iter) =
                                                                objects.iterate(heap)
                                                            {
                                                                for obj in obj_iter {
                                                                    args.push(obj);
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    // Extract user_link_flags from linker inputs (e.g., -lpthread)
                                    let input_additional_inputs =
                                        if let Ok(Some(dep_additional_inputs)) =
                                            input.get_attr("additional_inputs", heap)
                                        {
                                            if dep_additional_inputs.is_none() {
                                                Vec::new()
                                            } else {
                                                depset_or_iterable_values(
                                                    dep_additional_inputs,
                                                    heap,
                                                )?
                                            }
                                        } else {
                                            Vec::new()
                                        };
                                    run_inputs.extend(input_additional_inputs.iter().copied());
                                    if let Ok(Some(dep_link_flags)) =
                                        input.get_attr("user_link_flags", heap)
                                    {
                                        if !dep_link_flags.is_none() {
                                            for flag in
                                                depset_or_iterable_values(dep_link_flags, heap)?
                                            {
                                                args.push(cc_expand_link_flag_locations(
                                                    flag,
                                                    &input_additional_inputs,
                                                    heap,
                                                )?);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Add user link flags
            if !has_feature_args && !expanded_user_link_flags.is_none() {
                if let Ok(iter) = expanded_user_link_flags.iterate(heap) {
                    for flag in iter {
                        args.push(flag);
                    }
                }
            }

            // Add global --linkopt flags from command line
            if !is_static {
                for opt in crate::interpreter::rule_defs::build_config::get_linkopts() {
                    args.push(heap.alloc_str(&opt).to_value());
                }
            }

            // Add RPATH for dynamic library dependencies on non-Windows platforms.
            // This allows the runtime linker to find shared libraries relative to
            // the executable ($ORIGIN on Linux, @loader_path on macOS).
            if !is_static && !link_deps_statically && !is_windows_host() {
                let rpath_origin = if std::env::consts::OS == "macos" {
                    "@loader_path"
                } else {
                    "$ORIGIN"
                };
                // Add rpath pointing to the output directory itself and common lib locations
                args.push(
                    heap.alloc_str(&format!("-Wl,-rpath,{}", rpath_origin))
                        .to_value(),
                );
                args.push(
                    heap.alloc_str(&format!("-Wl,-rpath,{}/lib", rpath_origin))
                        .to_value(),
                );
            }

            let args_val = heap.alloc(args);
            let outputs_list = heap.alloc(vec![output_artifact]);
            let inputs_list = heap.alloc(run_inputs);
            let progress = heap
                .alloc_str(&format!("Linking {}", output_name))
                .to_value();
            let category = if is_static {
                "cpp_link_static_library"
            } else if is_dynamic {
                "cpp_link_dynamic_library"
            } else {
                "cpp_link_executable"
            };

            let _ = eval.eval_function(
                run,
                &[args_val],
                &[
                    ("outputs", outputs_list),
                    ("inputs", inputs_list),
                    ("category", heap.alloc_str(category).to_value()),
                    ("identifier", heap.alloc_str(&output_name).to_value()),
                    ("progress_message", progress),
                ],
            );

            // Create library_to_link if output is a library
            let library_to_link = if is_static || is_dynamic {
                heap.alloc(LibraryToLinkGen {
                    static_library: if is_static { out } else { Value::new_none() },
                    pic_static_library: Value::new_none(),
                    dynamic_library: if is_dynamic { out } else { Value::new_none() },
                    interface_library: Value::new_none(),
                    objects: Value::new_none(),
                    pic_objects: Value::new_none(),
                    alwayslink: false,
                })
            } else {
                Value::new_none()
            };

            // Return CcLinkingOutputs
            let executable = if !is_static && !is_dynamic {
                out
            } else {
                Value::new_none()
            };
            let linking_outputs = heap.alloc(CcLinkingOutputsGen {
                library_to_link,
                executable,
            });

            Ok(linking_outputs)
        } else {
            // Fallback: return empty linking outputs
            Ok(heap.alloc(CcLinkingOutputsGen {
                library_to_link: Value::new_none(),
                executable: Value::new_none(),
            }))
        }
    }

    /// Gets the tool path for a given action.
    #[allow(unused_variables)]
    fn get_tool_for_action<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<String> {
        // Normalize action name: rules_cc uses both hyphen (c++-link-executable) and
        // underscore (cpp_link_executable) variants. Normalize to hyphen form for matching.
        let normalized = normalize_action_name(action_name);
        let name = normalized.as_str();
        let tool = match std::env::consts::OS {
            "windows" => {
                let msvc = get_msvc_tool_paths();
                if name.contains("compile") {
                    msvc.as_ref()
                        .map(|t| t.cl.clone())
                        .unwrap_or_else(|| "cl.exe".to_owned())
                } else if name.contains("link") && name.contains("static-library") {
                    msvc.as_ref()
                        .map(|t| t.lib.clone())
                        .unwrap_or_else(|| "lib.exe".to_owned())
                } else if name.contains("link") {
                    // executable or dynamic library linking → use link.exe
                    msvc.as_ref()
                        .map(|t| t.link.clone())
                        .unwrap_or_else(|| "link.exe".to_owned())
                } else if name == "strip" || name == "objcopy" {
                    String::new()
                } else {
                    msvc.as_ref()
                        .map(|t| t.cl.clone())
                        .unwrap_or_else(|| "cl.exe".to_owned())
                }
            }
            "macos" => {
                if name.contains("compile") {
                    if name.starts_with("c-") || name.starts_with("c_") {
                        "/usr/bin/clang".to_owned()
                    } else {
                        "/usr/bin/clang++".to_owned()
                    }
                } else if name.contains("link") && name.contains("static-library") {
                    "/usr/bin/ar".to_owned()
                } else if name.contains("link") {
                    "/usr/bin/clang++".to_owned()
                } else if name == "strip" {
                    "/usr/bin/strip".to_owned()
                } else if name == "objcopy" {
                    "/usr/bin/objcopy".to_owned()
                } else {
                    "/usr/bin/clang".to_owned()
                }
            }
            _ => {
                if name.contains("compile") {
                    if name.starts_with("c-") || name.starts_with("c_") {
                        host_llvm_toolchain_bin("clang")
                            .unwrap_or_else(|| "/usr/bin/gcc".to_owned())
                    } else {
                        host_llvm_toolchain_bin("clang++")
                            .unwrap_or_else(|| "/usr/bin/g++".to_owned())
                    }
                } else if name.contains("link") && name.contains("static-library") {
                    "/usr/bin/ar".to_owned()
                } else if name.contains("link") {
                    host_llvm_toolchain_bin("clang++").unwrap_or_else(|| "/usr/bin/g++".to_owned())
                } else if name == "strip" {
                    "/usr/bin/strip".to_owned()
                } else if name == "objcopy" {
                    "/usr/bin/objcopy".to_owned()
                } else {
                    host_llvm_toolchain_bin("clang").unwrap_or_else(|| "/usr/bin/gcc".to_owned())
                }
            }
        };
        Ok(tool)
    }

    /// Gets execution requirements for a given action.
    #[allow(unused_variables)]
    fn get_execution_requirements<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TODO(cc_common): Implement proper execution requirements
        let map: SmallMap<Value<'v>, Value<'v>> = SmallMap::new();
        Ok(eval.heap().alloc(Dict::new(map)))
    }

    /// Checks if an action is enabled in the feature configuration.
    ///
    /// In Bazel, action enablement is controlled by features that gate specific
    /// compiler/linker actions. We check if the action_name corresponds to a
    /// known feature and consult the FeatureConfiguration if so.
    fn action_is_enabled<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<bool> {
        // Try to consult the FeatureConfiguration for action-specific features.
        // In Bazel, actions like "c++-compile", "c++-link-executable" etc. are
        // enabled based on the feature configuration. We map action names to
        // features where there's a direct correspondence.
        if let Some(fc) = feature_configuration.downcast_ref::<FeatureConfiguration>() {
            let normalized_action = normalize_action_name(action_name);
            let action_name = normalized_action.as_str();
            // Some actions correspond directly to features
            let feature_name = match action_name {
                "c++-compile" | "c-compile" | "cc-flags-make-variable" => None, // Always enabled
                _ if is_header_parsing_action(action_name) => Some("parse_headers"),
                "c++-link-executable"
                | "c++-link-dynamic-library"
                | "c++-link-nodeps-dynamic-library"
                | "c++-link-static-library" => None, // Always enabled
                // For other action names, check if there's a matching feature
                other => Some(other),
            };
            if let Some(feature) = feature_name {
                return Ok(fc.is_feature_enabled(feature));
            }
        }
        // Default: actions are enabled
        Ok(true)
    }

    /// Gets the command line for an action (memory inefficient version).
    #[allow(unused_variables)]
    fn get_memory_inefficient_command_line<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        #[starlark(require = named)] variables: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        // Normalize action name (rules_cc uses both underscore and hyphen variants)
        let normalized = normalize_action_name(action_name);
        let action_name = normalized.as_str();
        let mut args: Vec<Value<'v>> = Vec::new();

        // Helper to get a variable value from CcToolchainVariables or dict
        let get_var = |key: &str| -> Option<Value<'v>> {
            if let Ok(Some(v)) = variables.get_attr(key, heap) {
                return Some(v);
            }
            if let Some(dict_ref) = DictRef::from_value(variables) {
                return dict_ref.get_str(key);
            }
            None
        };

        // Helper to iterate a value that may be a depset or list
        let iterate_value =
            |val: Value<'v>, eval_ref: &mut Evaluator<'v, '_, '_>| -> Vec<Value<'v>> {
                let h = eval_ref.heap();
                if is_depset_value(val) {
                    return depset_to_list(val, h).unwrap_or_default();
                }
                if let Ok(iter) = val.iterate(h) {
                    iter.collect()
                } else {
                    Vec::new()
                }
            };

        let is_compile = action_name.contains("compile") && !action_name.contains("preprocess");
        let is_static_lib = action_name.contains("static-library");
        let is_dynamic_lib = action_name.contains("dynamic-library");
        let is_link = action_name.contains("link") && !action_name.contains("compile");
        let msvc = is_windows_host();

        // Helper to get string from a value (string or File with .path)
        let get_str_val = |v: Value<'v>| -> Option<String> {
            if let Some(s) = v.unpack_str() {
                return Some(s.to_owned());
            }
            if let Ok(Some(path_val)) = v.get_attr("path", heap) {
                if let Some(path_str) = path_val.unpack_str() {
                    return Some(path_str.to_owned());
                }
            }
            None
        };
        let target_system_name = get_var("target_system_name").and_then(|v| get_str_val(v));
        let target_libc = get_var("target_libc").and_then(|v| get_str_val(v));

        // --- Link/Archive actions ---
        if is_link {
            let output_path = get_var("output_execpath").and_then(|v| get_str_val(v));
            let feature_args = feature_configuration
                .downcast_ref::<FeatureConfiguration>()
                .map(|fc| expand_cc_flag_sets(fc, action_name, variables, heap))
                .transpose()?
                .unwrap_or_default();
            let has_feature_args = !feature_args.is_empty();
            let use_llvm_linux_link_defaults = !msvc
                && !has_feature_args
                && ((has_host_llvm_toolchain()
                    && is_musl_cc_toolchain_target(
                        target_system_name.as_deref(),
                        target_libc.as_deref(),
                    ))
                    || is_compiler_rt_crtbegin_link_output(output_path.as_deref()));

            if is_static_lib {
                if !has_feature_args {
                    if msvc {
                        // MSVC: lib.exe /nologo /OUT:<output>
                        args.push(heap.alloc_str("/nologo").to_value());
                        if let Some(ref path) = output_path {
                            args.push(heap.alloc_str(&format!("/OUT:{}", path)).to_value());
                        }
                    } else {
                        // ar archiver: rcs <output>
                        args.push(heap.alloc_str("rcs").to_value());
                        if let Some(ref path) = output_path {
                            args.push(heap.alloc_str(path).to_value());
                        }
                    }
                }
            } else if is_dynamic_lib {
                if msvc {
                    // MSVC: link.exe /nologo /DLL /OUT:<output>
                    args.push(heap.alloc_str("/nologo").to_value());
                    args.push(heap.alloc_str("/DLL").to_value());
                    if let Some(ref path) = output_path {
                        args.push(heap.alloc_str(&format!("/OUT:{}", path)).to_value());
                    }
                } else {
                    args.push(heap.alloc_str("-shared").to_value());
                    args.push(heap.alloc_str("-fPIC").to_value());
                    if use_llvm_linux_link_defaults {
                        args.push(heap.alloc_str("-nostdlib").to_value());
                    }
                    if let Some(ref path) = output_path {
                        args.push(heap.alloc_str("-o").to_value());
                        args.push(heap.alloc_str(path).to_value());
                    }
                }
            } else {
                // Executable link
                if msvc {
                    args.push(heap.alloc_str("/nologo").to_value());
                    if let Some(ref path) = output_path {
                        args.push(heap.alloc_str(&format!("/OUT:{}", path)).to_value());
                    }
                } else {
                    if let Some(ref path) = output_path {
                        args.push(heap.alloc_str("-o").to_value());
                        args.push(heap.alloc_str(path).to_value());
                    }
                    if use_llvm_linux_link_defaults {
                        for arg in [
                            "-target",
                            "x86_64-linux-musl",
                            "-static",
                            "-fuse-ld=lld",
                            "-rtlib=compiler-rt",
                            "-nostdlib++",
                            "--unwindlib=none",
                            "-Wl,-no-as-needed",
                            "-Wl,-z,relro,-z,now",
                            "-Wl,--push-state",
                            "-Wl,--as-needed",
                            "-lpthread",
                            "-ldl",
                            "-Wl,--pop-state",
                        ] {
                            args.push(heap.alloc_str(arg).to_value());
                        }
                    }
                }
            }

            args.extend(feature_args);

            // User link flags
            if !has_feature_args && let Some(user_flags) = get_var("user_link_flags") {
                if !user_flags.is_none() {
                    for flag in iterate_value(user_flags, eval) {
                        if let Some(s) = flag.unpack_str() {
                            if !s.is_empty() {
                                args.push(heap.alloc_str(s).to_value());
                            }
                        }
                    }
                }
            }

            // Compilation-mode-based linker flags. Mode is read from the
            // feature configuration (Plan 19.6) so exec-cfg links pick up
            // the exec platform's opt default while target-cfg links see
            // the user-requested mode from `--compilation_mode`.
            if !is_static_lib {
                let mode = compilation_mode_from_features(feature_configuration);
                match mode.as_str() {
                    "opt" => {
                        if !msvc {
                            let strip = crate::interpreter::rule_defs::build_config::get_strip();
                            if strip == "always" || (strip == "sometimes") {
                                args.push(heap.alloc_str("-Wl,-S").to_value());
                            }
                        }
                    }
                    "dbg" => {
                        if msvc {
                            args.push(heap.alloc_str("/DEBUG").to_value());
                        }
                    }
                    _ => {}
                }
            }

            // Add --linkopt flags from command line
            for opt in crate::interpreter::rule_defs::build_config::get_linkopts() {
                args.push(heap.alloc_str(&opt).to_value());
            }

            return Ok(heap.alloc(args));
        }

        // --- Compile actions below ---
        let feature_args = feature_configuration
            .downcast_ref::<FeatureConfiguration>()
            .map(|fc| expand_cc_flag_sets(fc, action_name, variables, heap))
            .transpose()?
            .unwrap_or_default();
        let has_feature_args = !feature_args.is_empty();

        // `-c` / `/c` (compile-only) belongs to the Bazel action_config's
        // command_line template, not to feature-derived flag_sets. It is only
        // emitted when a `source_file` variable is bound — i.e. the caller is
        // actually invoking a compile action with a known input. Callers like
        // rules_rust's `cargo_build_script` build compile_variables with
        // `cc_common.create_compile_variables(feature_configuration,
        // cc_toolchain)` (no source_file) to derive *default CFLAGS*, and
        // those CFLAGS must not include `-c` — otherwise downstream cc-rs
        // probes that compile-and-link in one shot (e.g. aws-lc-sys's
        // `memcmp_check`) end up producing relocatable object files instead
        // of executables.
        let has_source = get_var("source_file").is_some();
        if msvc {
            // MSVC compile flags
            args.push(heap.alloc_str("/nologo").to_value());
            args.push(heap.alloc_str("/EHsc").to_value());
            if is_compile && has_source {
                args.push(heap.alloc_str("/c").to_value());
            }
        } else {
            // GCC/Clang: -fPIC if pic variable is set
            if get_var("pic").is_some() {
                args.push(heap.alloc_str("-fPIC").to_value());
            }
            if is_compile && has_source {
                args.push(heap.alloc_str("-c").to_value());
            }
        }

        args.extend(feature_args);

        if is_compile
            && !msvc
            && !has_feature_args
            && has_host_llvm_toolchain()
            && is_musl_cc_toolchain_target(target_system_name.as_deref(), target_libc.as_deref())
        {
            for flag in llvm_musl_compile_default_args() {
                args.push(heap.alloc_str(&flag).to_value());
            }
        }
        // Compilation-mode-based flags. Mode comes from the feature
        // configuration (Plan 19.6) so an exec-cfg compile sees the exec
        // platform's opt default from `platform(exec_properties=...)`, and
        // a target-cfg compile sees whatever the user requested via
        // `--compilation_mode`. Also emit rules_cc's always-on compile flags
        // that the cc_toolchain_config feature set assumes as baseline.
        if is_compile && !has_feature_args {
            if !msvc {
                // Always-on flags from rules_cc's `compile_flags` feature.
                // rules_cc's default linux_cc_toolchain_config sets these
                // unconditionally via `compile_flags = [...]` on the
                // toolchain; emit them here so exec-cfg tool builds
                // (llvm-tblgen etc.) pick up the full baseline slug
                // previously skipped.
                for flag in [
                    "-U_FORTIFY_SOURCE",
                    "-fstack-protector",
                    "-Wall",
                    "-fno-omit-frame-pointer",
                ] {
                    args.push(heap.alloc_str(flag).to_value());
                }
            }

            let mode = compilation_mode_from_features(feature_configuration);
            match mode.as_str() {
                "opt" => {
                    if msvc {
                        args.push(heap.alloc_str("/O2").to_value());
                        args.push(heap.alloc_str("/DNDEBUG").to_value());
                    } else {
                        // Matches rules_cc's `opt_compile_flags` feature:
                        // `-g0 -O2 -D_FORTIFY_SOURCE=1 -DNDEBUG
                        //  -ffunction-sections -fdata-sections`.
                        for flag in [
                            "-g0",
                            "-O2",
                            "-D_FORTIFY_SOURCE=1",
                            "-DNDEBUG",
                            "-ffunction-sections",
                            "-fdata-sections",
                        ] {
                            args.push(heap.alloc_str(flag).to_value());
                        }
                    }
                }
                "dbg" => {
                    if msvc {
                        args.push(heap.alloc_str("/Od").to_value());
                        args.push(heap.alloc_str("/Zi").to_value());
                    } else {
                        args.push(heap.alloc_str("-g").to_value());
                        args.push(heap.alloc_str("-O0").to_value());
                    }
                }
                _ => {
                    // fastbuild: rules_cc adds `-g0` for skipping debug info.
                    if !msvc {
                        args.push(heap.alloc_str("-g0").to_value());
                    }
                }
            }
        }

        // Add --copt flags from command line (apply to all C/C++ compilations)
        if is_compile && !has_feature_args {
            for opt in crate::interpreter::rule_defs::build_config::get_copts() {
                args.push(heap.alloc_str(&opt).to_value());
            }
            // Add language-specific flags: --cxxopt for C++, --conlyopt for C
            // Determine language from action_name (c++-compile vs c-compile)
            if action_name.contains("c++") {
                for opt in crate::interpreter::rule_defs::build_config::get_cxxopts() {
                    args.push(heap.alloc_str(&opt).to_value());
                }
            } else {
                for opt in crate::interpreter::rule_defs::build_config::get_conlyopts() {
                    args.push(heap.alloc_str(&opt).to_value());
                }
            }
        }

        // Source file
        if let Some(source) = get_var("source_file") {
            if !source.is_none() {
                if let Some(s) = source.unpack_str() {
                    args.push(heap.alloc_str(s).to_value());
                } else if let Ok(Some(path_val)) = source.get_attr("path", heap) {
                    if let Some(path_str) = path_val.unpack_str() {
                        args.push(heap.alloc_str(path_str).to_value());
                    } else {
                        args.push(path_val);
                    }
                } else {
                    args.push(source);
                }
            }
        }

        // Output file
        if let Some(output) = get_var("output_file") {
            if !output.is_none() {
                let out_flag = if msvc { "/Fo" } else { "-o" };
                args.push(heap.alloc_str(out_flag).to_value());
                if let Some(s) = output.unpack_str() {
                    args.push(heap.alloc_str(s).to_value());
                } else if let Ok(Some(path_val)) = output.get_attr("path", heap) {
                    if let Some(path_str) = path_val.unpack_str() {
                        args.push(heap.alloc_str(path_str).to_value());
                    } else {
                        args.push(path_val);
                    }
                } else {
                    args.push(output);
                }
            }
        }

        // User compile flags
        if !has_feature_args && let Some(user_flags) = get_var("user_compile_flags") {
            if !user_flags.is_none() {
                for flag in iterate_value(user_flags, eval) {
                    if let Some(s) = flag.unpack_str() {
                        if !s.is_empty() {
                            args.push(heap.alloc_str(s).to_value());
                        }
                    }
                }
            }
        }

        // Include paths
        let inc_prefix = if msvc { "/I" } else { "-I" };
        if !has_feature_args && let Some(includes) = get_var("include_paths") {
            if !includes.is_none() {
                for inc in iterate_value(includes, eval) {
                    if let Some(s) = inc.unpack_str() {
                        if !s.is_empty() {
                            args.push(heap.alloc_str(&format!("{}{}", inc_prefix, s)).to_value());
                        }
                    }
                }
            }
        }

        // Quote include paths
        if !has_feature_args && let Some(quote_includes) = get_var("quote_include_paths") {
            if !quote_includes.is_none() {
                for inc in iterate_value(quote_includes, eval) {
                    if let Some(s) = inc.unpack_str() {
                        if !s.is_empty() {
                            if msvc {
                                args.push(heap.alloc_str(&format!("/I{}", s)).to_value());
                            } else {
                                args.push(heap.alloc_str("-iquote").to_value());
                                args.push(heap.alloc_str(s).to_value());
                            }
                        }
                    }
                }
            }
        }

        // System include paths
        if !has_feature_args && let Some(system_includes) = get_var("system_include_paths") {
            if !system_includes.is_none() {
                for inc in iterate_value(system_includes, eval) {
                    if let Some(s) = inc.unpack_str() {
                        if !s.is_empty() {
                            if msvc {
                                args.push(heap.alloc_str(&format!("/I{}", s)).to_value());
                            } else {
                                args.push(heap.alloc_str(&format!("-isystem{}", s)).to_value());
                            }
                        }
                    }
                }
            }
        }

        // External include paths
        if let Some(ext_includes) = get_var("external_include_paths") {
            if !ext_includes.is_none() {
                for inc in iterate_value(ext_includes, eval) {
                    if let Some(s) = inc.unpack_str() {
                        if !s.is_empty() {
                            if msvc {
                                args.push(heap.alloc_str(&format!("/I{}", s)).to_value());
                            } else {
                                args.push(heap.alloc_str(&format!("-isystem{}", s)).to_value());
                            }
                        }
                    }
                }
            }
        }

        // Preprocessor defines
        let def_prefix = if msvc { "/D" } else { "-D" };
        if let Some(defines) = get_var("preprocessor_defines") {
            if !defines.is_none() {
                for def in iterate_value(defines, eval) {
                    if let Some(s) = def.unpack_str() {
                        args.push(heap.alloc_str(&format!("{}{}", def_prefix, s)).to_value());
                    }
                }
            }
        }

        Ok(heap.alloc(args))
    }

    /// Gets environment variables for an action.
    #[allow(unused_variables)]
    fn get_environment_variables<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        #[starlark(require = named)] variables: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        let heap = eval.heap();
        let mut map: SmallMap<Value<'v>, Value<'v>> = SmallMap::new();
        let get_var = |key: &str| -> Option<Value<'v>> {
            if let Ok(Some(v)) = variables.get_attr(key, heap) {
                return Some(v);
            }
            if let Some(dict_ref) = DictRef::from_value(variables) {
                return dict_ref.get_str(key);
            }
            None
        };

        // On Windows, provide MSVC environment variables for compilation/linking
        #[cfg(target_os = "windows")]
        if let Some(tools) = get_msvc_tool_paths() {
            let include_val = format!(
                "{};{};{};{}",
                tools.msvc_include, tools.ucrt_include, tools.um_include, tools.shared_include
            );
            map.insert_hashed(
                heap.alloc_str("INCLUDE").to_value().get_hashed().unwrap(),
                heap.alloc_str(&include_val).to_value(),
            );

            let lib_val = format!("{};{};{}", tools.msvc_lib, tools.ucrt_lib, tools.um_lib);
            map.insert_hashed(
                heap.alloc_str("LIB").to_value().get_hashed().unwrap(),
                heap.alloc_str(&lib_val).to_value(),
            );
        }

        if let Some(fc) = feature_configuration.downcast_ref::<FeatureConfiguration>() {
            let action_name = normalize_action_name(action_name);
            for env_set in &fc.env_sets {
                if !env_set.applies_to_action(&action_name) || !env_set.with_features_match(fc) {
                    continue;
                }
                for entry in &env_set.env_entries {
                    if let Some(gate) = &entry.expand_if_available {
                        if get_var(gate).is_none_or(|value| value.is_none()) {
                            continue;
                        }
                    }
                    let value = expand_cc_scalar_template(&entry.value, heap, get_var)?;
                    map.insert_hashed(
                        heap.alloc_str(&entry.key).to_value().get_hashed().unwrap(),
                        heap.alloc_str(&value).to_value(),
                    );
                }
            }
        }

        Ok(heap.alloc(Dict::new(map)))
    }

    /// Creates empty toolchain variables.
    fn empty_variables<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        Ok(eval.heap().alloc(CcToolchainVariablesGen {
            vars: Value::new_none(),
        }))
    }

    /// Gets legacy CC_FLAGS make variable value.
    #[allow(unused_variables)]
    fn legacy_cc_flags_make_variable_do_not_use<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<String> {
        // TODO(cc_common): Extract from toolchain
        Ok(String::new())
    }

    /// Checks if experimental cc_shared_library is enabled.
    fn check_experimental_cc_shared_library(
        #[starlark(this)] _this: &CcCommonModule,
    ) -> starlark::Result<bool> {
        Ok(true)
    }

    /// Checks if objc_library transition is disabled.
    fn incompatible_disable_objc_library_transition(
        #[starlark(this)] _this: &CcCommonModule,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Checks if Go exec groups should be added to binary rules.
    fn add_go_exec_groups_to_binary_rules(
        #[starlark(this)] _this: &CcCommonModule,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Checks if implementation_deps is allowed by allowlist.
    #[allow(unused_variables)]
    fn implementation_deps_allowed_by_allowlist<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] ctx: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<bool> {
        Ok(true)
    }

    /// Creates a compilation action (allowlisted, public API).
    ///
    /// This is the simplified public version of cc_internal.create_cc_compile_action.
    /// It accepts fewer parameters and is access-controlled in Bazel.
    #[allow(unused_variables)]
    fn create_compile_action<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = named, default = NoneType)] actions: Value<'v>,
        #[starlark(require = named, default = NoneType)] cc_toolchain: Value<'v>,
        #[starlark(require = named, default = NoneType)] feature_configuration: Value<'v>,
        #[starlark(require = named, default = NoneType)] source_file: Value<'v>,
        #[starlark(require = named, default = NoneType)] output_file: Value<'v>,
        #[starlark(require = named, default = NoneType)] variables: Value<'v>,
        #[starlark(require = named, default = NoneOr::None)] action_name: NoneOr<&str>,
        #[starlark(require = named, default = NoneType)] compilation_context: Value<'v>,
        #[starlark(require = named, default = NoneType)] additional_inputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] additional_outputs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        // Delegate to the internal cc_common compile infrastructure
        // This creates a real compile action using the same path as cc_common.compile()
        let heap = eval.heap();

        if source_file.is_none() || output_file.is_none() {
            return Ok(NoneType);
        }

        // Get the actions object - try ctx.actions first, then the value itself
        let actions_value = if !actions.is_none() {
            match actions.get_attr("actions", heap) {
                Ok(Some(a)) => a,
                _ => actions,
            }
        } else {
            return Ok(NoneType);
        };

        // Try to get run method and register the compile action
        let run_method = match actions_value.get_attr("run", heap) {
            Ok(Some(method)) => method,
            _ => return Ok(NoneType),
        };

        // Get compiler from toolchain
        let is_cpp = action_name
            .into_option()
            .map(|n| n.contains("c++") || n.contains("cpp"))
            .unwrap_or(false);

        let default_compiler = match std::env::consts::OS {
            "windows" => {
                if let Some(tools) = get_msvc_tool_paths() {
                    tools.cl.clone()
                } else {
                    "cl.exe".to_owned()
                }
            }
            "macos" => "/usr/bin/clang++".to_owned(),
            _ => {
                if is_cpp {
                    "/usr/bin/g++".to_owned()
                } else {
                    "/usr/bin/gcc".to_owned()
                }
            }
        };

        let compiler_path = if !cc_toolchain.is_none() {
            cc_toolchain
                .get_attr("compiler_executable", heap)
                .ok()
                .flatten()
                .and_then(|v| v.unpack_str().map(|s| s.to_owned()))
                .unwrap_or(default_compiler)
        } else {
            default_compiler
        };

        // Mark output as output artifact
        let output_artifact = match output_file.get_attr("as_output", heap) {
            Ok(Some(m)) => eval.eval_function(m, &[], &[]).unwrap_or(output_file),
            _ => output_file,
        };

        // Build command line
        let msvc = is_msvc_compiler(&compiler_path);
        let mut args_vec: Vec<Value<'v>> = Vec::new();

        let output_path_str = output_file
            .get_attr("path", heap)
            .ok()
            .flatten()
            .and_then(|v| v.unpack_str().map(|s| s.to_owned()))
            .unwrap_or_default();

        if msvc {
            args_vec.push(heap.alloc_str(&compiler_path).to_value());
            args_vec.push(heap.alloc_str("/nologo").to_value());
            args_vec.push(heap.alloc_str("/EHsc").to_value());
            args_vec.push(heap.alloc_str("/c").to_value());
            args_vec.push(source_file);
            args_vec.push(
                heap.alloc_str(&format!("/Fo{}", output_path_str))
                    .to_value(),
            );
        } else {
            args_vec.push(heap.alloc_str("-c").to_value());
            args_vec.push(source_file);
            args_vec.push(heap.alloc_str("-o").to_value());
            args_vec.push(output_artifact);
            args_vec.push(heap.alloc_str("-fPIC").to_value());
        }

        // Plan 19.6: always-on compile flags + compilation-mode flag set,
        // derived from the feature configuration so exec-cfg tool builds
        // (llvm-tblgen etc.) get the opt defaults that
        // `platform(exec_properties=...)` declares while target-cfg builds
        // get the mode the user requested via `--compilation_mode`.
        if is_compile_action_name(action_name) && !msvc {
            for flag in [
                "-U_FORTIFY_SOURCE",
                "-fstack-protector",
                "-Wall",
                "-fno-omit-frame-pointer",
            ] {
                args_vec.push(heap.alloc_str(flag).to_value());
            }
            let mode = compilation_mode_from_features(feature_configuration);
            match mode.as_str() {
                "opt" => {
                    for flag in [
                        "-g0",
                        "-O2",
                        "-D_FORTIFY_SOURCE=1",
                        "-DNDEBUG",
                        "-ffunction-sections",
                        "-fdata-sections",
                    ] {
                        args_vec.push(heap.alloc_str(flag).to_value());
                    }
                }
                "dbg" => {
                    args_vec.push(heap.alloc_str("-g").to_value());
                    args_vec.push(heap.alloc_str("-O0").to_value());
                }
                _ => {
                    // fastbuild: rules_cc appends -g0 as the minimal default.
                    args_vec.push(heap.alloc_str("-g0").to_value());
                }
            }
        }

        // Add include dirs from compilation context
        if !compilation_context.is_none() {
            for attr_name in &["includes", "system_includes", "quote_includes"] {
                if let Ok(Some(includes_val)) = compilation_context.get_attr(attr_name, heap) {
                    if !includes_val.is_none() {
                        for elem in depset_values(includes_val, heap)? {
                            let dir = elem.to_str();
                            if !dir.is_empty() {
                                let flag = include_flag_for_context_attr(attr_name, &dir, msvc);
                                args_vec.push(heap.alloc_str(&flag).to_value());
                            }
                        }
                    }
                }
            }
        }

        let args_list = heap.alloc(args_vec);
        let executable = heap.alloc_str(&compiler_path).to_value();
        let action_name_str = action_name.into_option().unwrap_or("CppCompile");
        let mnemonic = heap.alloc_str(action_name_str).to_value();

        // Register the action
        eval.eval_function(
            run_method,
            &[args_list],
            &[("executable", executable), ("mnemonic", mnemonic)],
        )
        .ok();

        Ok(NoneType)
    }

    /// Creates a linker input.
    #[allow(unused_variables)]
    fn create_linker_input<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] owner: Value<'v>,
        #[starlark(require = named, default = NoneType)] libraries: Value<'v>,
        #[starlark(require = named, default = NoneType)] user_link_flags: Value<'v>,
        #[starlark(require = named, default = NoneType)] additional_inputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] linkstamps: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        let _ = (this, linkstamps);
        if slug_util::memory_checkpoint::enabled() {
            let (libraries_direct, libraries_transitive, libraries_depth, libraries_empty) =
                depset_shape(libraries);
            let (flags_direct, flags_transitive, flags_depth, flags_empty) =
                depset_shape(user_link_flags);
            let (inputs_direct, inputs_transitive, inputs_depth, inputs_empty) =
                depset_shape(additional_inputs);
            cc_common_checkpoint(
                "cc_common_create_linker_input_start",
                [
                    ("libraries_is_depset", is_depset_value(libraries) as usize),
                    ("libraries_direct", libraries_direct),
                    ("libraries_transitive", libraries_transitive),
                    ("libraries_depth", libraries_depth),
                    ("libraries_empty", libraries_empty),
                    (
                        "user_flags_is_depset",
                        is_depset_value(user_link_flags) as usize,
                    ),
                    ("user_flags_direct", flags_direct),
                    ("user_flags_transitive", flags_transitive),
                    ("user_flags_depth", flags_depth),
                    ("user_flags_empty", flags_empty),
                    (
                        "additional_inputs_is_depset",
                        is_depset_value(additional_inputs) as usize,
                    ),
                    ("additional_inputs_direct", inputs_direct),
                    ("additional_inputs_transitive", inputs_transitive),
                    ("additional_inputs_depth", inputs_depth),
                    ("additional_inputs_empty", inputs_empty),
                ],
            );
        }
        let libraries = cc_internal_freeze_depset_or_iterable(libraries, heap)?;
        let user_flags = cc_freeze_user_link_flags(user_link_flags, heap)?;
        let additional_inputs = cc_internal_freeze_depset_or_iterable(additional_inputs, heap)?;
        let value = heap.alloc(LinkerInputStubGen {
            owner,
            libraries,
            user_link_flags: user_flags,
            additional_inputs,
        });
        if slug_util::memory_checkpoint::enabled() {
            let (flags_direct, flags_transitive, flags_depth, flags_empty) =
                depset_shape(user_flags);
            cc_common_checkpoint(
                "cc_common_create_linker_input_result",
                [
                    ("user_flags_is_depset", is_depset_value(user_flags) as usize),
                    ("user_flags_direct", flags_direct),
                    ("user_flags_transitive", flags_transitive),
                    ("user_flags_depth", flags_depth),
                    ("user_flags_empty", flags_empty),
                ],
            );
        }
        Ok(value)
    }

    /// Creates a linking context.
    #[allow(unused_variables)]
    fn create_linking_context<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named, default = NoneType)] linker_inputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] owner: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        if slug_util::memory_checkpoint::enabled() {
            let (direct, transitive, depth, is_empty) = depset_shape(linker_inputs);
            cc_common_checkpoint(
                "cc_common_create_linking_context",
                [
                    (
                        "linker_inputs_is_depset",
                        is_depset_value(linker_inputs) as usize,
                    ),
                    ("linker_inputs_direct", direct),
                    ("linker_inputs_transitive", transitive),
                    ("linker_inputs_depth", depth),
                    ("linker_inputs_empty", is_empty),
                ],
            );
        }
        Ok(heap.alloc(LinkingContextWithInputsGen { linker_inputs }))
    }

    /// Checks if a feature is enabled in the feature configuration.
    #[allow(unused_variables)]
    fn is_enabled<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] feature_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<bool> {
        let _ = (this, eval);
        // Try to downcast to our FeatureConfiguration type
        if let Some(fc) = feature_configuration.downcast_ref::<FeatureConfiguration>() {
            return Ok(fc.is_feature_enabled(feature_name));
        }
        // Fallback for non-FeatureConfiguration values (e.g., None passed from tests)
        let enabled = match feature_name {
            "supports_dynamic_linker" | "supports_interface_shared_libraries" => true,
            "pic" | "supports_pic" => !is_windows_host(),
            "targets_windows" => is_windows_host(),
            "static_link_cpp_runtimes" => true,
            _ => false,
        };
        Ok(enabled)
    }

    /// Creates a compilation context from headers, includes, and defines.
    ///
    /// This is used by rules_cc to construct the compilation context that
    /// gets propagated to dependents via CcInfo.
    #[allow(unused_variables)]
    fn create_compilation_context<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named, default = NoneType)] headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] quote_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] system_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] external_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] framework_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] defines: Value<'v>,
        #[starlark(require = named, default = NoneType)] local_defines: Value<'v>,
        #[starlark(require = named, default = NoneType)] direct_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] direct_public_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] direct_private_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] direct_textual_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] purpose: Value<'v>,
        #[starlark(kwargs)] kwargs: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Merge `dependent_cc_compilation_contexts` (passed via kwargs) into
        // the new context — Bazel's API guarantees that the resulting
        // `headers` / `includes` / etc. include both the values explicitly
        // passed AND every dependent context's. rules_cc's
        // `cc_compilation_helper.init_cc_compilation_context` relies on
        // this: it builds the new context with `headers = depset(declared_include_srcs)`
        // (only THIS rule's hdrs) and passes `dependent_cc_compilation_contexts = deps`
        // expecting Bazel to merge transitive headers in. Without the
        // merge, a cc_library compile only sees its own hdrs and the
        // first failing transitive include (e.g. `:config` exposing
        // `llvm-config.h` to a Demangle compile via DemangleConfig.h)
        // can't be resolved on RE.
        let dep_contexts = kwargs
            .get_attr("dependent_cc_compilation_contexts", heap)
            .ok()
            .flatten()
            .unwrap_or(Value::new_none());
        let exported_dep_contexts = kwargs
            .get_attr("exported_dependent_cc_compilation_contexts", heap)
            .ok()
            .flatten()
            .unwrap_or(Value::new_none());

        let mut all_dep_contexts: Vec<Value<'v>> = Vec::new();
        for ctxs_val in &[dep_contexts, exported_dep_contexts] {
            if !ctxs_val.is_none() {
                if let Ok(iter) = ctxs_val.iterate(heap) {
                    for c in iter {
                        if !c.is_none() {
                            all_dep_contexts.push(c);
                        }
                    }
                }
            }
        }

        let merge_field = |direct_val: Value<'v>, attr: &str| -> starlark::Result<Value<'v>> {
            let mut transitive_depsets: Vec<Value<'v>> = Vec::new();
            for c in &all_dep_contexts {
                if let Ok(Some(v)) = c.get_attr(attr, heap) {
                    if !v.is_none() {
                        transitive_depsets.push(v);
                    }
                }
            }
            // Build a depset whose direct elements are the explicit value
            // (collected here) and whose transitive children are the dep
            // contexts' depsets. If only transitives, return a depset
            // wrapping just those; if neither, return None.
            let direct_elems = depset_or_iterable_values(direct_val, heap)?;

            if direct_elems.is_empty() && transitive_depsets.is_empty() {
                return Ok(Value::new_none());
            }
            Ok(
                crate::interpreter::rule_defs::depset::make_depset_from_lists(
                    heap,
                    direct_elems,
                    transitive_depsets,
                    "default",
                )
                .unwrap_or(Value::new_none()),
            )
        };

        Ok(heap.alloc(CcCompilationContextGen {
            headers: merge_field(headers, "headers")?,
            includes: merge_field(includes, "includes")?,
            quote_includes: merge_field(quote_includes, "quote_includes")?,
            system_includes: merge_field(system_includes, "system_includes")?,
            external_includes: merge_field(external_includes, "external_includes")?,
            framework_includes: merge_field(framework_includes, "framework_includes")?,
            defines: merge_field(defines, "defines")?,
            local_defines,
        }))
    }

    /// Creates compilation outputs.
    #[allow(unused_variables)]
    fn create_compilation_outputs<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named, default = NoneType)] objects: Value<'v>,
        #[starlark(require = named, default = NoneType)] pic_objects: Value<'v>,
        #[starlark(require = named, default = NoneType)] lto_compilation_context: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        Ok(heap.alloc(CompilationOutputsGen {
            objects: cc_internal_freeze_depset_or_iterable(objects, heap)?,
            pic_objects: cc_internal_freeze_depset_or_iterable(pic_objects, heap)?,
        }))
    }

    /// Merges multiple compilation outputs into one.
    #[allow(unused_variables)]
    fn merge_compilation_outputs<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named, default = NoneType)] compilation_outputs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        // Collect all objects and pic_objects from the list of compilation outputs
        let mut all_objects: Vec<Value<'v>> = Vec::new();
        let mut all_pic_objects: Vec<Value<'v>> = Vec::new();

        if !compilation_outputs.is_none() {
            if let Ok(iter) = compilation_outputs.iterate(heap) {
                for co in iter {
                    if let Ok(Some(objects)) = co.get_attr("objects", heap) {
                        if !objects.is_none() {
                            if let Ok(obj_iter) = objects.iterate(heap) {
                                all_objects.extend(obj_iter);
                            }
                        }
                    }
                    if let Ok(Some(pic_objects)) = co.get_attr("pic_objects", heap) {
                        if !pic_objects.is_none() {
                            if let Ok(pic_iter) = pic_objects.iterate(heap) {
                                all_pic_objects.extend(pic_iter);
                            }
                        }
                    }
                }
            }
        }

        Ok(heap.alloc(CompilationOutputsGen {
            objects: if all_objects.is_empty() {
                cc_internal_freeze_values(Vec::new(), heap)?
            } else {
                cc_internal_freeze_values(all_objects, heap)?
            },
            pic_objects: if all_pic_objects.is_empty() {
                cc_internal_freeze_values(Vec::new(), heap)?
            } else {
                cc_internal_freeze_values(all_pic_objects, heap)?
            },
        }))
    }

    /// Creates a linking context from compilation outputs.
    ///
    /// Returns a tuple of (linking_context, linking_outputs).
    #[allow(unused_variables)]
    fn create_linking_context_from_compilation_outputs<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] actions: Value<'v>,
        #[starlark(require = named)] name: &str,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named, default = NoneType)] compilation_outputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] user_link_flags: Value<'v>,
        #[starlark(require = named, default = NoneType)] linking_contexts: Value<'v>,
        #[starlark(require = named, default = NoneType)] language: Value<'v>,
        #[starlark(require = named, default = false)] disallow_static_libraries: bool,
        #[starlark(require = named, default = false)] disallow_dynamic_library: bool,
        #[starlark(require = named, default = NoneType)] additional_inputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] grep_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] stamp: Value<'v>,
        #[starlark(require = named, default = NoneType)] linked_dll_name_suffix: Value<'v>,
        #[starlark(require = named, default = NoneType)] win_def_file: Value<'v>,
        #[starlark(require = named, default = NoneType)] test_only_target: Value<'v>,
        #[starlark(require = named, default = false)] alwayslink: bool,
        #[starlark(require = named, default = NoneType)] variables_extension: Value<'v>,
        #[starlark(require = named, default = NoneType)] main_output: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        // Create library_to_link from compilation outputs
        let library_to_link = if compilation_outputs.is_none() {
            Value::new_none()
        } else {
            // Extract objects and pic_objects from compilation_outputs
            let objects = compilation_outputs
                .get_attr("objects", heap)
                .ok()
                .flatten()
                .unwrap_or(Value::new_none());
            let pic_objects = compilation_outputs
                .get_attr("pic_objects", heap)
                .ok()
                .flatten()
                .unwrap_or(Value::new_none());
            heap.alloc(LibraryToLinkGen {
                static_library: Value::new_none(),
                pic_static_library: Value::new_none(),
                dynamic_library: Value::new_none(),
                interface_library: Value::new_none(),
                objects: cc_internal_freeze_depset_or_iterable(objects, heap)?,
                pic_objects: cc_internal_freeze_depset_or_iterable(pic_objects, heap)?,
                alwayslink,
            })
        };

        // Create linking outputs
        let linking_outputs = heap.alloc(CcLinkingOutputsGen {
            library_to_link,
            executable: Value::new_none(),
        });

        let libraries_depset = if library_to_link.is_none() {
            cc_internal_freeze_values(Vec::new(), heap)?
        } else {
            cc_internal_freeze_values([library_to_link], heap)?
        };
        let user_link_flags_depset = cc_freeze_user_link_flags(user_link_flags, heap)?;
        let additional_inputs_depset =
            cc_internal_freeze_depset_or_iterable(additional_inputs, heap)?;

        let linker_input = heap.alloc(LinkerInputStubGen {
            owner: Value::new_none(), // No owner label available in this context
            libraries: libraries_depset,
            user_link_flags: user_link_flags_depset,
            additional_inputs: additional_inputs_depset,
        });

        // Create linker_inputs depset containing this LinkerInput
        // Also include transitive linker_inputs from provided linking_contexts
        let mut transitive_depsets: Vec<Value<'v>> = Vec::new();
        if !linking_contexts.is_none() {
            if let Ok(iter) = linking_contexts.iterate(heap) {
                for ctx_val in iter {
                    if let Ok(Some(li)) = ctx_val.get_attr("linker_inputs", heap) {
                        if !li.is_none() {
                            transitive_depsets.push(li);
                        }
                    }
                }
            }
        }

        let linker_inputs = crate::interpreter::rule_defs::depset::make_depset_from_lists(
            heap,
            vec![linker_input],
            transitive_depsets,
            "default",
        )?;

        // Create linking context
        let linking_context = heap.alloc(LinkingContextWithInputsGen { linker_inputs });
        if slug_util::memory_checkpoint::enabled() {
            let (libraries_direct, libraries_transitive, libraries_depth, libraries_empty) =
                depset_shape(libraries_depset);
            let (flags_direct, flags_transitive, flags_depth, flags_empty) =
                depset_shape(user_link_flags_depset);
            let (linker_direct, linker_transitive, linker_depth, linker_empty) =
                depset_shape(linker_inputs);
            cc_common_checkpoint(
                "cc_common_create_linking_context_from_outputs",
                [
                    ("has_library_to_link", (!library_to_link.is_none()) as usize),
                    ("libraries_direct", libraries_direct),
                    ("libraries_transitive", libraries_transitive),
                    ("libraries_depth", libraries_depth),
                    ("libraries_empty", libraries_empty),
                    ("user_flags_direct", flags_direct),
                    ("user_flags_transitive", flags_transitive),
                    ("user_flags_depth", flags_depth),
                    ("user_flags_empty", flags_empty),
                    ("linker_inputs_direct", linker_direct),
                    ("linker_inputs_transitive", linker_transitive),
                    ("linker_inputs_depth", linker_depth),
                    ("linker_inputs_empty", linker_empty),
                ],
            );
        }

        // Return tuple
        Ok(heap.alloc((linking_context, linking_outputs)))
    }

    /// Merges multiple linking contexts into one.
    ///
    /// Collects linker_inputs from all provided linking contexts into a
    /// single merged linking context with transitive depset.
    #[allow(unused_variables)]
    fn merge_linking_contexts<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named, default = NoneType)] linking_contexts: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Collect all linker_inputs depsets as transitive children
        let mut transitive_depsets: Vec<Value<'v>> = Vec::new();
        let mut input_context_count = 0usize;

        if !linking_contexts.is_none() {
            if let Ok(iter) = linking_contexts.iterate(heap) {
                for ctx_val in iter {
                    input_context_count += 1;
                    if let Ok(Some(linker_inputs)) = ctx_val.get_attr("linker_inputs", heap) {
                        if !linker_inputs.is_none() {
                            transitive_depsets.push(linker_inputs);
                        }
                    }
                }
            }
        }

        if slug_util::memory_checkpoint::enabled() {
            let max_child_depth = transitive_depsets
                .iter()
                .filter_map(|value| depset_summary(*value))
                .map(|summary| summary.depth as usize)
                .max()
                .unwrap_or(0);
            cc_common_checkpoint(
                "cc_common_merge_linking_contexts",
                [
                    ("input_contexts", input_context_count),
                    ("transitive_depsets", transitive_depsets.len()),
                    ("max_child_depth", max_child_depth),
                ],
            );
        }

        // Create merged depset with all inputs as transitive children
        let linker_inputs = crate::interpreter::rule_defs::depset::make_depset_from_lists(
            heap,
            Vec::new(), // no direct elements
            transitive_depsets,
            "default",
        )?;

        Ok(heap.alloc(LinkingContextWithInputsGen { linker_inputs }))
    }

    /// Creates a library_to_link struct.
    #[allow(unused_variables)]
    fn create_library_to_link<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] actions: Value<'v>,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named, default = NoneType)] static_library: Value<'v>,
        #[starlark(require = named, default = NoneType)] pic_static_library: Value<'v>,
        #[starlark(require = named, default = NoneType)] dynamic_library: Value<'v>,
        #[starlark(require = named, default = NoneType)] interface_library: Value<'v>,
        #[starlark(require = named, default = NoneType)] pic_objects: Value<'v>,
        #[starlark(require = named, default = NoneType)] objects: Value<'v>,
        #[starlark(require = named, default = false)] alwayslink: bool,
        #[starlark(require = named, default = NoneType)] dynamic_library_symlink_path: Value<'v>,
        #[starlark(require = named, default = NoneType)] interface_library_symlink_path: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        if slug_util::memory_checkpoint::enabled() {
            let (objects_direct, objects_transitive, objects_depth, objects_empty) =
                depset_shape(objects);
            let (pic_objects_direct, pic_objects_transitive, pic_objects_depth, pic_objects_empty) =
                depset_shape(pic_objects);
            cc_common_checkpoint(
                "cc_common_create_library_to_link",
                [
                    ("has_static_library", (!static_library.is_none()) as usize),
                    (
                        "has_pic_static_library",
                        (!pic_static_library.is_none()) as usize,
                    ),
                    ("has_dynamic_library", (!dynamic_library.is_none()) as usize),
                    (
                        "has_interface_library",
                        (!interface_library.is_none()) as usize,
                    ),
                    ("objects_is_depset", is_depset_value(objects) as usize),
                    ("objects_direct", objects_direct),
                    ("objects_transitive", objects_transitive),
                    ("objects_depth", objects_depth),
                    ("objects_empty", objects_empty),
                    (
                        "pic_objects_is_depset",
                        is_depset_value(pic_objects) as usize,
                    ),
                    ("pic_objects_direct", pic_objects_direct),
                    ("pic_objects_transitive", pic_objects_transitive),
                    ("pic_objects_depth", pic_objects_depth),
                    ("pic_objects_empty", pic_objects_empty),
                    ("alwayslink", alwayslink as usize),
                ],
            );
        }
        Ok(heap.alloc(LibraryToLinkGen {
            static_library,
            pic_static_library,
            dynamic_library,
            interface_library,
            objects: cc_internal_freeze_depset_or_iterable(objects, heap)?,
            pic_objects: cc_internal_freeze_depset_or_iterable(pic_objects, heap)?,
            alwayslink,
        }))
    }

    /// Returns tool execution requirements for an action.
    ///
    /// Returns a list of execution requirements (strings like "requires-network")
    /// that should be added to actions using the specified tool.
    #[allow(unused_variables)]
    fn get_tool_requirement_for_action<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Return an empty list - no special execution requirements
        Ok(eval.heap().alloc(Vec::<String>::new()))
    }

    /// Creates compile variables for use with get_memory_inefficient_command_line.
    ///
    /// Returns CcToolchainVariables with compilation-related settings.
    #[allow(unused_variables)]
    fn create_compile_variables<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] source_file: Value<
            'v,
        >,
        #[starlark(require = named, default = starlark::values::none::NoneType)] output_file: Value<
            'v,
        >,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        user_compile_flags: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        include_directories: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        quote_include_directories: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        system_include_directories: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        framework_include_directories: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        preprocessor_defines: Value<'v>,
        #[starlark(require = named, default = false)] use_pic: bool,
        #[starlark(require = named, default = false)] add_legacy_cxx_options: bool,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        variables_extension: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        let mut map: SmallMap<Value<'v>, Value<'v>> = SmallMap::new();

        insert_if_set(&mut map, heap, "source_file", source_file);
        insert_if_set(&mut map, heap, "output_file", output_file);
        insert_if_set(&mut map, heap, "user_compile_flags", user_compile_flags);
        insert_if_set(&mut map, heap, "include_paths", include_directories);
        insert_if_set(
            &mut map,
            heap,
            "quote_include_paths",
            quote_include_directories,
        );
        insert_if_set(
            &mut map,
            heap,
            "system_include_paths",
            system_include_directories,
        );
        insert_if_set(
            &mut map,
            heap,
            "framework_include_directories",
            framework_include_directories,
        );
        insert_if_set(&mut map, heap, "preprocessor_defines", preprocessor_defines);
        if use_pic {
            map.insert_hashed(
                heap.alloc_str("pic").to_value().get_hashed().unwrap(),
                Value::new_bool(true),
            );
        }

        let direct_target_system_name = cc_toolchain_target_system_name(cc_toolchain, heap);
        let direct_target_libc = cc_toolchain_target_libc(cc_toolchain, heap);
        if let Some(target_system_name) = direct_target_system_name {
            insert_if_set(&mut map, heap, "target_system_name", target_system_name);
        }
        if let Some(target_libc) = direct_target_libc {
            insert_if_set(&mut map, heap, "target_libc", target_libc);
        }
        // Merge variables_extension dict into the variables
        if !variables_extension.is_none() {
            if let Some(dict_ref) = DictRef::from_value(variables_extension) {
                for (k, v) in dict_ref.iter() {
                    if let Ok(hashed) = k.get_hashed() {
                        map.insert_hashed(hashed, v);
                    }
                }
            }
        }

        let vars = heap.alloc(Dict::new(map));
        Ok(heap.alloc(CcToolchainVariablesGen { vars }))
    }

    /// Creates link variables for use with get_memory_inefficient_command_line.
    ///
    /// Used by rules_rust to get linker command line from cc toolchain.
    #[allow(unused_variables)]
    fn create_link_variables<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named, default = false)] is_linking_dynamic_library: bool,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        runtime_library_search_directories: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        user_link_flags: Value<'v>,
        // Bazel-compatible: output file path for the linker output.
        // Sets `output_execpath` variable used by get_memory_inefficient_command_line.
        #[starlark(require = named, default = starlark::values::none::NoneType)] output_file: Value<
            'v,
        >,
        // Bazel-compatible: library search directories for -L flags.
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        library_search_directories: Value<'v>,
        // Bazel-compatible: param file path for long command lines.
        #[starlark(require = named, default = starlark::values::none::NoneType)] param_file: Value<
            'v,
        >,
        // Bazel-compatible: whether the linker (not archiver) is being used.
        #[starlark(require = named, default = true)] is_using_linker: bool,
        // Bazel-compatible: whether to keep debug symbols.
        #[starlark(require = named, default = true)] must_keep_debug: bool,
        // Bazel-compatible: whether to use test-only flags.
        #[starlark(require = named, default = false)] use_test_only_flags: bool,
        // Bazel-compatible: vestigial parameter (unused in modern Bazel).
        #[starlark(require = named, default = true)] is_static_linking_mode: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let _ = (
            feature_configuration,
            is_using_linker,
            is_static_linking_mode,
        );
        let heap = eval.heap();
        // Build a dict with the link variables for get_memory_inefficient_command_line
        let mut map: SmallMap<Value<'v>, Value<'v>> = SmallMap::new();

        // output_file sets output_execpath - critical for /OUT: or -o flags
        if !output_file.is_none() {
            let path_str = if let Some(s) = output_file.unpack_str() {
                s.to_owned()
            } else {
                format!("{}", output_file)
            };
            map.insert_hashed(
                heap.alloc_str("output_execpath")
                    .to_value()
                    .get_hashed()
                    .unwrap(),
                heap.alloc_str(&path_str).to_value(),
            );
        }

        insert_if_set(&mut map, heap, "user_link_flags", user_link_flags);
        insert_if_set(
            &mut map,
            heap,
            "runtime_library_search_directories",
            runtime_library_search_directories,
        );
        insert_if_set(
            &mut map,
            heap,
            "library_search_directories",
            library_search_directories,
        );
        insert_if_set(&mut map, heap, "linker_param_file", param_file);

        if let Some(target_system_name) = cc_toolchain_target_system_name(cc_toolchain, heap) {
            insert_if_set(&mut map, heap, "target_system_name", target_system_name);
        }
        if let Some(target_libc) = cc_toolchain_target_libc(cc_toolchain, heap) {
            insert_if_set(&mut map, heap, "target_libc", target_libc);
        }

        if is_linking_dynamic_library {
            map.insert_hashed(
                heap.alloc_str("is_linking_dynamic_library")
                    .to_value()
                    .get_hashed()
                    .unwrap(),
                Value::new_bool(true),
            );
        }
        if use_test_only_flags {
            map.insert_hashed(
                heap.alloc_str("is_cc_test")
                    .to_value()
                    .get_hashed()
                    .unwrap(),
                Value::new_bool(true),
            );
        }
        if !must_keep_debug {
            map.insert_hashed(
                heap.alloc_str("strip_debug_symbols")
                    .to_value()
                    .get_hashed()
                    .unwrap(),
                Value::new_bool(true),
            );
        }

        let vars = heap.alloc(Dict::new(map));
        Ok(heap.alloc(CcToolchainVariablesGen { vars }))
    }

    /// Merges multiple CcInfo providers into a single CcInfo.
    ///
    /// Collects compilation contexts and linking contexts from all input
    /// CcInfo providers and merges them into a single CcInfo.
    #[allow(unused_variables)]
    fn merge_cc_infos<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = named, default = NoneType)] cc_infos: Value<'v>,
        #[starlark(require = named, default = NoneType)] direct_cc_infos: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Collect compilation contexts and linking contexts from all inputs
        let mut linking_contexts: Vec<Value<'v>> = Vec::new();
        let mut headers_depsets: Vec<Value<'v>> = Vec::new();
        let mut includes_depsets: Vec<Value<'v>> = Vec::new();
        let mut quote_includes_depsets: Vec<Value<'v>> = Vec::new();
        let mut system_includes_depsets: Vec<Value<'v>> = Vec::new();
        let mut framework_includes_depsets: Vec<Value<'v>> = Vec::new();
        let mut defines_depsets: Vec<Value<'v>> = Vec::new();
        let mut local_defines_depsets: Vec<Value<'v>> = Vec::new();
        let mut external_includes_depsets: Vec<Value<'v>> = Vec::new();
        let mut cc_infos_count = 0usize;
        let mut direct_cc_infos_count = 0usize;

        // Helper closure to extract contexts from a CcInfo
        let mut process_info = |info: Value<'v>| {
            if let Ok(Some(comp_ctx)) = info.get_attr("compilation_context", heap) {
                if !comp_ctx.is_none() {
                    // Extract all depset fields from compilation context
                    if let Ok(Some(h)) = comp_ctx.get_attr("headers", heap) {
                        if !h.is_none() {
                            headers_depsets.push(h);
                        }
                    }
                    if let Ok(Some(i)) = comp_ctx.get_attr("includes", heap) {
                        if !i.is_none() {
                            includes_depsets.push(i);
                        }
                    }
                    if let Ok(Some(qi)) = comp_ctx.get_attr("quote_includes", heap) {
                        if !qi.is_none() {
                            quote_includes_depsets.push(qi);
                        }
                    }
                    if let Ok(Some(si)) = comp_ctx.get_attr("system_includes", heap) {
                        if !si.is_none() {
                            system_includes_depsets.push(si);
                        }
                    }
                    if let Ok(Some(ei)) = comp_ctx.get_attr("external_includes", heap) {
                        if !ei.is_none() {
                            external_includes_depsets.push(ei);
                        }
                    }
                    if let Ok(Some(fi)) = comp_ctx.get_attr("framework_includes", heap) {
                        if !fi.is_none() {
                            framework_includes_depsets.push(fi);
                        }
                    }
                    if let Ok(Some(d)) = comp_ctx.get_attr("defines", heap) {
                        if !d.is_none() {
                            defines_depsets.push(d);
                        }
                    }
                    if let Ok(Some(ld)) = comp_ctx.get_attr("local_defines", heap) {
                        if !ld.is_none() {
                            local_defines_depsets.push(ld);
                        }
                    }
                }
            }
            if let Ok(Some(link_ctx)) = info.get_attr("linking_context", heap) {
                if !link_ctx.is_none() {
                    linking_contexts.push(link_ctx);
                }
            }
        };

        // Process cc_infos (transitive)
        if !cc_infos.is_none() {
            if let Ok(iter) = cc_infos.iterate(heap) {
                for info in iter {
                    cc_infos_count += 1;
                    process_info(info);
                }
            }
        }

        // Process direct_cc_infos
        if !direct_cc_infos.is_none() {
            if let Ok(iter) = direct_cc_infos.iterate(heap) {
                for info in iter {
                    direct_cc_infos_count += 1;
                    process_info(info);
                }
            }
        }
        drop(process_info);

        if slug_util::memory_checkpoint::enabled() {
            let compilation_depsets = headers_depsets
                .len()
                .saturating_add(includes_depsets.len())
                .saturating_add(quote_includes_depsets.len())
                .saturating_add(system_includes_depsets.len())
                .saturating_add(external_includes_depsets.len())
                .saturating_add(framework_includes_depsets.len())
                .saturating_add(defines_depsets.len())
                .saturating_add(local_defines_depsets.len());
            let max_compilation_child_depth = [
                &headers_depsets,
                &includes_depsets,
                &quote_includes_depsets,
                &system_includes_depsets,
                &external_includes_depsets,
                &framework_includes_depsets,
                &defines_depsets,
                &local_defines_depsets,
            ]
            .into_iter()
            .flat_map(|depsets| depsets.iter())
            .filter_map(|value| depset_summary(*value))
            .map(|summary| summary.depth as usize)
            .max()
            .unwrap_or(0);
            let max_linking_child_depth = linking_contexts
                .iter()
                .filter_map(|ctx_val| ctx_val.get_attr("linker_inputs", heap).ok().flatten())
                .filter_map(depset_summary)
                .map(|summary| summary.depth as usize)
                .max()
                .unwrap_or(0);
            cc_common_checkpoint(
                "cc_common_merge_cc_infos_collected",
                [
                    ("cc_infos", cc_infos_count),
                    ("direct_cc_infos", direct_cc_infos_count),
                    ("linking_contexts", linking_contexts.len()),
                    ("compilation_depsets", compilation_depsets),
                    ("headers_depsets", headers_depsets.len()),
                    ("includes_depsets", includes_depsets.len()),
                    ("quote_includes_depsets", quote_includes_depsets.len()),
                    ("system_includes_depsets", system_includes_depsets.len()),
                    ("external_includes_depsets", external_includes_depsets.len()),
                    (
                        "framework_includes_depsets",
                        framework_includes_depsets.len(),
                    ),
                    ("defines_depsets", defines_depsets.len()),
                    ("local_defines_depsets", local_defines_depsets.len()),
                    ("max_compilation_child_depth", max_compilation_child_depth),
                    ("max_linking_child_depth", max_linking_child_depth),
                ],
            );
        }

        // Merge compilation contexts by combining all depset fields
        let has_any = !headers_depsets.is_empty()
            || !includes_depsets.is_empty()
            || !quote_includes_depsets.is_empty()
            || !system_includes_depsets.is_empty()
            || !external_includes_depsets.is_empty()
            || !framework_includes_depsets.is_empty()
            || !defines_depsets.is_empty()
            || !local_defines_depsets.is_empty();

        let merged_compilation_context = if !has_any {
            Value::new_none()
        } else {
            let merge_field = |depsets: Vec<Value<'v>>| -> starlark::Result<Value<'v>> {
                if depsets.is_empty() {
                    Ok(Value::new_none())
                } else {
                    crate::interpreter::rule_defs::depset::make_depset_from_lists(
                        heap,
                        Vec::new(),
                        depsets,
                        "default",
                    )
                }
            };
            heap.alloc(CcCompilationContextGen {
                headers: merge_field(headers_depsets)?,
                includes: merge_field(includes_depsets)?,
                quote_includes: merge_field(quote_includes_depsets)?,
                system_includes: merge_field(system_includes_depsets)?,
                external_includes: merge_field(external_includes_depsets)?,
                framework_includes: merge_field(framework_includes_depsets)?,
                defines: merge_field(defines_depsets)?,
                local_defines: merge_field(local_defines_depsets)?,
            })
        };

        // Merge linking contexts into a single one
        let merged_linking_context = if linking_contexts.is_empty() {
            Value::new_none()
        } else {
            // Collect all linker_inputs depsets as transitive children
            let mut transitive_depsets: Vec<Value<'v>> = Vec::new();
            for ctx_val in &linking_contexts {
                if let Ok(Some(linker_inputs)) = ctx_val.get_attr("linker_inputs", heap) {
                    if !linker_inputs.is_none() {
                        transitive_depsets.push(linker_inputs);
                    }
                }
            }
            let merged_linker_inputs =
                crate::interpreter::rule_defs::depset::make_depset_from_lists(
                    heap,
                    Vec::new(),
                    transitive_depsets,
                    "default",
                )?;
            heap.alloc(LinkingContextWithInputsGen {
                linker_inputs: merged_linker_inputs,
            })
        };

        if slug_util::memory_checkpoint::enabled() {
            let (linker_direct, linker_transitive, linker_depth, linker_empty) =
                merged_linking_context
                    .get_attr("linker_inputs", heap)
                    .ok()
                    .flatten()
                    .map(depset_shape)
                    .unwrap_or((0, 0, 0, merged_linking_context.is_none() as usize));
            cc_common_checkpoint(
                "cc_common_merge_cc_infos_result",
                [
                    (
                        "has_compilation_context",
                        (!merged_compilation_context.is_none()) as usize,
                    ),
                    (
                        "has_linking_context",
                        (!merged_linking_context.is_none()) as usize,
                    ),
                    ("linker_inputs_direct", linker_direct),
                    ("linker_inputs_transitive", linker_transitive),
                    ("linker_inputs_depth", linker_depth),
                    ("linker_inputs_empty", linker_empty),
                ],
            );
        }

        Ok(heap.alloc(CcInfoInstanceGen {
            compilation_context: merged_compilation_context,
            linking_context: merged_linking_context,
        }))
    }

    /// Merges multiple CcCompilationContexts into one.
    ///
    /// Combines headers, includes, quote_includes, system_includes,
    /// framework_includes, defines, and local_defines from all input contexts.
    #[allow(unused_variables)]
    fn merge_compilation_contexts<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = named, default = NoneType)] compilation_contexts: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let mut headers_depsets: Vec<Value<'v>> = Vec::new();
        let mut includes_depsets: Vec<Value<'v>> = Vec::new();
        let mut quote_includes_depsets: Vec<Value<'v>> = Vec::new();
        let mut system_includes_depsets: Vec<Value<'v>> = Vec::new();
        let mut external_includes_depsets: Vec<Value<'v>> = Vec::new();
        let mut framework_includes_depsets: Vec<Value<'v>> = Vec::new();
        let mut defines_depsets: Vec<Value<'v>> = Vec::new();
        let mut local_defines_depsets: Vec<Value<'v>> = Vec::new();

        if !compilation_contexts.is_none() {
            if let Ok(iter) = compilation_contexts.iterate(heap) {
                for ctx_val in iter {
                    if let Ok(Some(h)) = ctx_val.get_attr("headers", heap) {
                        if !h.is_none() {
                            headers_depsets.push(h);
                        }
                    }
                    if let Ok(Some(i)) = ctx_val.get_attr("includes", heap) {
                        if !i.is_none() {
                            includes_depsets.push(i);
                        }
                    }
                    if let Ok(Some(qi)) = ctx_val.get_attr("quote_includes", heap) {
                        if !qi.is_none() {
                            quote_includes_depsets.push(qi);
                        }
                    }
                    if let Ok(Some(si)) = ctx_val.get_attr("system_includes", heap) {
                        if !si.is_none() {
                            system_includes_depsets.push(si);
                        }
                    }
                    if let Ok(Some(ei)) = ctx_val.get_attr("external_includes", heap) {
                        if !ei.is_none() {
                            external_includes_depsets.push(ei);
                        }
                    }
                    if let Ok(Some(fi)) = ctx_val.get_attr("framework_includes", heap) {
                        if !fi.is_none() {
                            framework_includes_depsets.push(fi);
                        }
                    }
                    if let Ok(Some(d)) = ctx_val.get_attr("defines", heap) {
                        if !d.is_none() {
                            defines_depsets.push(d);
                        }
                    }
                    if let Ok(Some(ld)) = ctx_val.get_attr("local_defines", heap) {
                        if !ld.is_none() {
                            local_defines_depsets.push(ld);
                        }
                    }
                }
            }
        }

        let merge_field = |depsets: Vec<Value<'v>>| -> starlark::Result<Value<'v>> {
            if depsets.is_empty() {
                Ok(Value::new_none())
            } else {
                crate::interpreter::rule_defs::depset::make_depset_from_lists(
                    heap,
                    Vec::new(),
                    depsets,
                    "default",
                )
            }
        };

        Ok(heap.alloc(CcCompilationContextGen {
            headers: merge_field(headers_depsets)?,
            includes: merge_field(includes_depsets)?,
            quote_includes: merge_field(quote_includes_depsets)?,
            system_includes: merge_field(system_includes_depsets)?,
            external_includes: merge_field(external_includes_depsets)?,
            framework_includes: merge_field(framework_includes_depsets)?,
            defines: merge_field(defines_depsets)?,
            local_defines: merge_field(local_defines_depsets)?,
        }))
    }

    /// Creates a debug context from compilation outputs.
    #[allow(unused_variables)]
    fn create_debug_context<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = pos, default = NoneType)] compilation_outputs: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(CcDebugContext))
    }

    /// Merges multiple debug contexts into one.
    #[allow(unused_variables)]
    fn merge_debug_context<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = pos, default = NoneType)] debug_contexts: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(CcDebugContext))
    }

    /// Creates a CcToolchainConfigInfo provider instance.
    ///
    /// This is the main entry point for defining C++ toolchain configurations
    /// in Starlark. Called from cc_toolchain_config rule implementations.
    #[allow(unused_variables)]
    fn create_cc_toolchain_config_info<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = named)] ctx: Value<'v>,
        #[starlark(require = named, default = "")] toolchain_identifier: &str,
        #[starlark(require = named, default = "")] host_system_name: &str,
        #[starlark(require = named, default = "")] target_system_name: &str,
        #[starlark(require = named, default = "")] target_cpu: &str,
        #[starlark(require = named, default = "")] target_libc: &str,
        #[starlark(require = named, default = "")] compiler: &str,
        #[starlark(require = named, default = "")] abi_version: &str,
        #[starlark(require = named, default = "")] abi_libc_version: &str,
        #[starlark(require = named, default = NoneType)] tool_paths: Value<'v>,
        #[starlark(require = named, default = NoneType)] make_variables: Value<'v>,
        #[starlark(require = named, default = "")] builtin_sysroot: &str,
        #[starlark(require = named, default = "")] cc_target_os: &str,
        #[starlark(require = named, default = NoneType)] features: Value<'v>,
        #[starlark(require = named, default = NoneType)] action_configs: Value<'v>,
        #[starlark(require = named, default = NoneType)] artifact_name_patterns: Value<'v>,
        #[starlark(require = named, default = NoneType)] cxx_builtin_include_directories: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Store all fields in a dict for the CcToolchainConfigInfo instance
        let fields = heap.alloc(starlark::values::dict::AllocDict([
            ("toolchain_identifier", heap.alloc(toolchain_identifier)),
            ("host_system_name", heap.alloc(host_system_name)),
            ("target_system_name", heap.alloc(target_system_name)),
            ("target_cpu", heap.alloc(target_cpu)),
            ("target_libc", heap.alloc(target_libc)),
            ("compiler", heap.alloc(compiler)),
            ("abi_version", heap.alloc(abi_version)),
            ("abi_libc_version", heap.alloc(abi_libc_version)),
            (
                "tool_paths",
                if tool_paths.is_none() {
                    heap.alloc(Vec::<Value>::new())
                } else {
                    tool_paths
                },
            ),
            (
                "make_variables",
                if make_variables.is_none() {
                    heap.alloc(Vec::<Value>::new())
                } else {
                    make_variables
                },
            ),
            ("builtin_sysroot", heap.alloc(builtin_sysroot)),
            ("cc_target_os", heap.alloc(cc_target_os)),
            (
                "features",
                if features.is_none() {
                    heap.alloc(Vec::<Value>::new())
                } else {
                    features
                },
            ),
            (
                "action_configs",
                if action_configs.is_none() {
                    heap.alloc(Vec::<Value>::new())
                } else {
                    action_configs
                },
            ),
            (
                "artifact_name_patterns",
                if artifact_name_patterns.is_none() {
                    heap.alloc(Vec::<Value>::new())
                } else {
                    artifact_name_patterns
                },
            ),
            (
                "cxx_builtin_include_directories",
                if cxx_builtin_include_directories.is_none() {
                    heap.alloc(Vec::<Value>::new())
                } else {
                    cxx_builtin_include_directories
                },
            ),
        ]));
        Ok(heap.alloc(CcToolchainConfigInfoInstanceGen { fields }))
    }
}
