/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Step A of §15.5.23: synthesize a runfiles symlink tree for
//! `DefaultInfo(executable = X, default_runfiles = R)`.
//!
//! Bazel's `SymlinkTreeAction` lays out a `<exe>.runfiles/<workspace>/<short_path>`
//! directory tree at action time so that a py_binary stub (or any other runfiles-aware
//! executable) can locate its runtime dependencies. Kuro didn't emit this tree from
//! any rule — rules_python's `py_binary` asserts `Cannot find .runfiles directory
//! for <exe>` when invoked via `run_binary(tool = :py_binary)`.
//!
//! We fill that gap without modifying rules_python by reaching into the
//! `DefaultInfo(...)` constructor: when the caller sets both `executable` and a
//! non-empty `default_runfiles`, we emit a `symlinked_dir` action, wrap the
//! executable via `with_associated_artifacts([tree])`, and return both values to
//! `default_info_creator`. `ctx.actions.run(executable = X)` in the consumer
//! rule then walks the associated tree via the existing `visit_artifacts` path
//! in `StarlarkArtifact` (see `starlark_artifact.rs:315`), picking the tree up
//! as an action input automatically.

use dupe::Dupe;
use indexmap::indexset;
use kuro_build_api::interpreter::rule_defs::artifact::associated::AssociatedArtifacts;
use kuro_build_api::interpreter::rule_defs::artifact::starlark_artifact::StarlarkArtifact;
use kuro_build_api::interpreter::rule_defs::artifact::starlark_artifact_like::StarlarkArtifactLike;
use kuro_build_api::interpreter::rule_defs::artifact::starlark_artifact_like::StarlarkInputArtifactLike;
use kuro_build_api::interpreter::rule_defs::artifact::starlark_declared_artifact::StarlarkDeclaredArtifact;
use kuro_build_api::interpreter::rule_defs::context::AnalysisActions;
use kuro_build_api::interpreter::rule_defs::depset::collect_depset_elements;
use kuro_build_api::interpreter::rule_defs::provider::builtin::default_info::Runfiles;
use kuro_build_api::interpreter::rule_defs::provider::builtin::default_info::SYNTHESIZE_RUNFILES_TREE;
use kuro_core::fs::buck_out_path::BuckOutPathKind;
use kuro_execute::execute::request::OutputType;
use starlark::eval::Evaluator;
use starlark::values::AllocValue;
use starlark::values::Heap;
use starlark::values::UnpackValue;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::dict::DictRef;

use crate::actions::impls::copy::CopyMode;
use crate::actions::impls::symlinked_dir::UnregisteredSymlinkedDirAction;

pub(crate) fn init_synthesize_runfiles_tree() {
    SYNTHESIZE_RUNFILES_TREE.init(synthesize_runfiles_tree);
}

/// Entry point registered in `SYNTHESIZE_RUNFILES_TREE`. Returns `(wrapped_exe, tree)`.
fn synthesize_runfiles_tree<'v>(
    eval: &mut Evaluator<'v, '_, '_>,
    actions: &AnalysisActions<'v>,
    executable: Value<'v>,
    runfiles_val: Value<'v>,
    workspace_name: &str,
) -> kuro_error::Result<(Value<'v>, Value<'v>)> {
    let heap = eval.heap();
    let loc = eval.call_stack_top_location();

    let exe_artifact = <&dyn StarlarkInputArtifactLike<'v>>::unpack_value(executable)
        .map_err(|e| kuro_error::kuro_error!(kuro_error::ErrorTag::Input, "{e}"))?
        .ok_or_else(|| {
            kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "DefaultInfo(executable=...): expected an artifact, got `{}`",
                executable.get_type()
            )
        })?;

    // Key into which every runfile is laid out. Bazel places runfiles at
    // `<exe>.runfiles/<workspace>/<short_path>`. Kuro's `short_path` for
    // declare_file(..., sibling=) drops the package prefix — self-consistent
    // because rules_python's generated stub is computed from the same short_path
    // (see §15.5.23 scope note).
    let mut srcs: Vec<(String, Value<'v>)> = Vec::new();
    let workspace_prefix: String = if workspace_name.is_empty() {
        String::new()
    } else {
        format!("{workspace_name}/")
    };

    let runfiles = runfiles_val.downcast_ref::<Runfiles<'v>>().ok_or_else(|| {
        kuro_error::kuro_error!(
            kuro_error::ErrorTag::Input,
            "DefaultInfo(default_runfiles=...): expected runfiles, got `{}`",
            runfiles_val.get_type()
        )
    })?;

    // files: depset of File objects. Key by `<workspace>/<short_path>`.
    let mut collected = Vec::new();
    collect_depset_elements(runfiles.files().to_value(), &mut collected, heap);
    for v in collected {
        let artifact_like = <&dyn StarlarkInputArtifactLike<'v>>::unpack_value(v)
            .map_err(|e| kuro_error::kuro_error!(kuro_error::ErrorTag::Input, "{e}"))?
            .ok_or_else(|| {
                kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "runfiles.files: expected File, got `{}`",
                    v.get_type()
                )
            })?;
        let short = artifact_short_path_string(artifact_like)?;
        let key = runfile_key(&workspace_prefix, &short);
        srcs.push((key, v));
    }

    // symlinks: dict<string, File> — keyed at `<workspace>/<k>` (workspace-relative).
    if let Some(dict) = DictRef::from_value(runfiles.symlinks().to_value()) {
        for (k, v) in dict.iter() {
            let key_str = k.unpack_str().ok_or_else(|| {
                kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "runfiles.symlinks: key must be string, got `{}`",
                    k.get_type()
                )
            })?;
            srcs.push((format!("{workspace_prefix}{key_str}"), v));
        }
    }

    // root_symlinks: dict<string, File> — keyed at the runfiles root (no workspace prefix).
    if let Some(dict) = DictRef::from_value(runfiles.root_symlinks().to_value()) {
        for (k, v) in dict.iter() {
            let key_str = k.unpack_str().ok_or_else(|| {
                kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "runfiles.root_symlinks: key must be string, got `{}`",
                    k.get_type()
                )
            })?;
            srcs.push((key_str.to_owned(), v));
        }
    }

    // Note: empty_filenames (for implicit __init__.py) is not materialised here.
    // Python 3 namespace packages cover most cases; if a consumer actually needs
    // zero-byte files in the tree we'll plumb `ctx.actions.write("", ...)` through
    // this path. See §15.5.23 for rationale.

    if srcs.is_empty() {
        return Ok((executable, Value::new_none()));
    }

    // Deduplicate by key, favouring the first entry (Bazel semantics: explicit
    // symlinks override implicit file placements).
    let mut seen = std::collections::HashSet::new();
    srcs.retain(|(k, _)| seen.insert(k.clone()));

    // Declare the tree directory as a sibling of the executable:
    // <exe_dir>/<exe_filename>.runfiles.
    let (exe_prefix, exe_filename) = exe_parent_and_filename(exe_artifact)?;
    let tree_filename = format!("{exe_filename}.runfiles");
    let tree_artifact_obj = actions.state()?.declare_output(
        exe_prefix.as_deref(),
        &tree_filename,
        OutputType::Directory,
        loc.dupe(),
        BuckOutPathKind::Configuration,
        heap,
    )?;

    // Build the srcs dict using the unpack format the action expects.
    let unpack_entries = build_unpack_entries(heap, srcs);

    let action = UnregisteredSymlinkedDirAction::new(CopyMode::Symlink, unpack_entries)?;
    let unioned = action.unioned_associated_artifacts();

    let tree_output = tree_artifact_obj.as_output();
    let mut registry = actions.state()?;
    registry.register_action(
        indexset![tree_output],
        action,
        None,
        None,
        None,
        std::sync::Arc::new(std::collections::BTreeMap::new()),
    )?;
    drop(registry);

    let tree_declared = StarlarkDeclaredArtifact::new(loc.dupe(), tree_artifact_obj, unioned);
    let tree_value: Value<'v> = tree_declared.alloc_value(heap);

    // Wrap the executable so consumers see the tree as an associated artifact.
    let wrapped = {
        let tree_like = <&dyn StarlarkInputArtifactLike<'v>>::unpack_value(tree_value)
            .map_err(|e| kuro_error::kuro_error!(kuro_error::ErrorTag::Input, "{e}"))?
            .ok_or_else(|| {
                kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "synthesized tree is not an artifact value"
                )
            })?;
        let tree_group = tree_like.get_artifact_group()?;
        let original_assocs = exe_artifact.get_associated_artifacts();
        let combined = match original_assocs {
            Some(a) => a.union(AssociatedArtifacts::from(std::iter::once(tree_group))),
            None => AssociatedArtifacts::from(std::iter::once(tree_group)),
        };
        wrap_exe_with_associated(heap, exe_artifact, combined)?
    };

    Ok((wrapped, tree_value))
}

/// Build the runfiles-tree key for a file given the runfiles' workspace prefix
/// and the file's `short_path`.
///
/// Bazel's runfiles layout keys main-workspace files under `<workspace>/<short_path>`
/// and external-repo files under `<external_repo>/...`. The latter case is already
/// encoded in the `File.short_path` as a leading `../<external_repo>/...`, so when
/// we see that form we strip the `../` and drop the workspace prefix — otherwise
/// the generated key would be `<workspace>/../<external_repo>/...`, which is not a
/// valid forward-relative path and is rejected by `symlinked_dir`.
fn runfile_key(workspace_prefix: &str, short_path: &str) -> String {
    if let Some(rest) = short_path.strip_prefix("../") {
        rest.to_owned()
    } else {
        format!("{workspace_prefix}{short_path}")
    }
}

fn exe_parent_and_filename<'v>(
    exe: &'v dyn StarlarkInputArtifactLike<'v>,
) -> kuro_error::Result<(Option<String>, String)> {
    // Snapshot the short path into owned pieces while the callback borrows it.
    // The runfiles tree is declared in *this* rule's artifact namespace, so we
    // need a forward-relative path. Bazel-form short_path for an external-repo
    // executable starts with `../<repo>/...`, which is not a valid
    // forward-relative path component. Strip the leading `../<repo>` so the
    // tree lands as a sibling of the exe within the rule's namespace.
    let pieces = std::cell::RefCell::new((None::<String>, String::new()));
    exe.with_short_path(&|path| {
        let mut slot = pieces.borrow_mut();
        let parent_str = path.parent().map(|p| p.as_str().to_owned());
        let cleaned_parent = parent_str.and_then(|s| {
            // External-repo Bazel short_path: strip `../<repo>` (and optional
            // package suffix) so the tree declaration uses a valid
            // forward-relative path within this rule's namespace. If the
            // exe sits at the package root of an external repo, the parent
            // collapses to `None`.
            let cleaned = if let Some(rest) = s.strip_prefix("../") {
                match rest.split_once('/') {
                    Some((_, after)) => after.to_owned(),
                    None => String::new(),
                }
            } else {
                s
            };
            if cleaned.is_empty() {
                None
            } else {
                Some(cleaned)
            }
        });
        slot.0 = cleaned_parent;
        slot.1 = path
            .file_name()
            .map(|f| f.as_str().to_owned())
            .unwrap_or_default();
        starlark::values::StringValue::default()
    })?;
    let (prefix, filename) = pieces.into_inner();
    if filename.is_empty() {
        return Err(kuro_error::kuro_error!(
            kuro_error::ErrorTag::Input,
            "DefaultInfo(executable=...): artifact has no file name"
        ));
    }
    Ok((prefix, filename))
}

fn artifact_short_path_string<'v>(
    artifact: &'v dyn StarlarkInputArtifactLike<'v>,
) -> kuro_error::Result<String> {
    let slot = std::cell::RefCell::new(String::new());
    artifact.with_short_path(&|p| {
        *slot.borrow_mut() = p.as_str().to_owned();
        starlark::values::StringValue::default()
    })?;
    Ok(slot.into_inner())
}

/// Build the `UnpackDictEntries<&str, ValueAsInputArtifactLike>` that
/// `UnregisteredSymlinkedDirAction::new` expects. Keys are leaked onto the
/// analysis heap via `heap.alloc_str` so they outlive this call.
fn build_unpack_entries<'v>(
    heap: Heap<'v>,
    entries: Vec<(String, Value<'v>)>,
) -> starlark::values::dict::UnpackDictEntries<
    &'v str,
    kuro_build_api::interpreter::rule_defs::artifact::starlark_artifact_like::ValueAsInputArtifactLike<'v>,
>{
    use kuro_build_api::interpreter::rule_defs::artifact::starlark_artifact_like::ValueAsInputArtifactLike;
    use starlark::values::UnpackValue;
    let mut out = Vec::with_capacity(entries.len());
    for (k, v) in entries {
        let k_str = heap.alloc_str(&k).as_str();
        let v_as = ValueAsInputArtifactLike::unpack_value(v)
            .ok()
            .flatten()
            .expect("caller guarantees v is an artifact-like Value");
        out.push((k_str, v_as));
    }
    starlark::values::dict::UnpackDictEntries { entries: out }
}

fn wrap_exe_with_associated<'v>(
    heap: Heap<'v>,
    exe: &'v dyn StarlarkInputArtifactLike<'v>,
    assocs: AssociatedArtifacts,
) -> kuro_error::Result<Value<'v>> {
    // Bound-exe path: freeze-friendly StarlarkArtifact; preserves associated on freeze.
    if let Ok(bound) = exe.get_bound_starlark_artifact() {
        let wrapped = StarlarkArtifact::new_with_associated_artifacts(bound.artifact(), assocs);
        return Ok(heap.alloc(wrapped));
    }
    Err(kuro_error::kuro_error!(
        kuro_error::ErrorTag::Input,
        "DefaultInfo(executable=...): executable must be a bound artifact"
    ))
}
