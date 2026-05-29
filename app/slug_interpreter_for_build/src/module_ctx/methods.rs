/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Starlark methods available on `module_ctx`. I/O operations (download,
//! execute, file) are implemented directly; label-taking path operations share
//! the same materialization boundary as `repository_ctx`.

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use starlark::environment::MethodsBuilder;
use starlark::starlark_module;
use starlark::values::Heap;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::list_or_tuple::UnpackListOrTuple;

use crate::module_ctx::context::ModuleContext;
use crate::module_ctx::context::ShouldWatch;
use crate::module_ctx::metadata::StarlarkModuleExtensionMetadata;
use crate::module_ctx::metadata::validate_facts_value;
use crate::repository_ctx::DownloadInfo;
use crate::repository_ctx::DownloadToken;
use crate::repository_ctx::ExecutionResult;
use crate::repository_ctx::RepositoryPath;
use crate::repository_ctx::apply_unified_patch;
use crate::repository_ctx::ensure_label_path_materialized;
use crate::repository_ctx::extract_archive;
use crate::repository_ctx::get_urls_from_value;
use crate::repository_ctx::parse_rename_files;
use crate::repository_ctx::try_ensure_label_path_materialized;

/// Module context methods for Bazel module extensions.
/// I/O operations (download, execute, file) are fully implemented.
/// Label-taking path operations trigger lazy materialization before use.
#[starlark_module]
pub(super) fn module_ctx_methods(builder: &mut MethodsBuilder) {
    /// Report progress to the user.
    #[allow(unused_variables)]
    fn report_progress<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] status: &str,
    ) -> starlark::Result<Value<'v>> {
        tracing::info!("Extension progress: {}", status);
        Ok(Value::new_none())
    }

    /// Returns whether the given module uses this extension as a dev dependency.
    ///
    /// In Bazel, module extensions can check if a particular bazel_module has
    /// declared the extension as a dev dependency. Dev dependencies are only
    /// visible in the root module.
    fn is_dev_dependency<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] _module: Value<'v>,
    ) -> starlark::Result<bool> {
        // For now, return false (not a dev dependency).
        // A full implementation would check the module's use_extension() declaration.
        let _ = this;
        Ok(false)
    }

    /// Read a file and return its contents as a string.
    fn read(
        this: &ModuleContext,
        #[starlark(require = pos)] path: Value,
        #[starlark(require = named, default = "auto")] watch: &str,
    ) -> starlark::Result<String> {
        let should_watch = ShouldWatch::parse(watch)?;
        let resolved = resolve_module_ctx_input_path(this, path, "module_ctx.read()")?;
        let path_str = resolved.to_string_lossy().to_string();
        this.maybe_record_file_input(&resolved, should_watch)?;
        let content = std::fs::read_to_string(&resolved).map_err(|e| {
            let working_dir = this
                .working_dir
                .as_ref()
                .map(|wd| wd.display().to_string())
                .unwrap_or_else(|| "<none>".to_owned());
            slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "module_ctx.read() failed to read '{}' (requested path: '{}', working_dir: {}): {}",
                resolved.display(),
                path_str,
                working_dir,
                e
            )
        })?;
        Ok(content)
    }

    /// Write a file with the given content.
    fn file<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] path: Value<'v>,
        #[starlark(require = named, default = "")] content: &str,
        #[starlark(require = named, default = false)] executable: bool,
        #[starlark(require = named, default = false)] _legacy_utf8: bool,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let path_str = path.unpack_str().unwrap_or("");
        let resolved = if Path::new(path_str).is_absolute() {
            PathBuf::from(path_str)
        } else if let Some(ref wd) = this.working_dir {
            wd.join(path_str)
        } else {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "module_ctx.file() requires a working directory or absolute path"
            )
            .into());
        };

        // Ensure parent directory exists
        if let Some(parent) = resolved.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                starlark::Error::from(slug_error::slug_error!(
                    slug_error::ErrorTag::Input,
                    "Failed to create parent directory for {}: {}",
                    resolved.display(),
                    e
                ))
            })?;
        }

        std::fs::write(&resolved, content).map_err(|e| {
            starlark::Error::from(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "Failed to write file {}: {}",
                resolved.display(),
                e
            ))
        })?;

        // Set executable permission on Unix
        #[cfg(unix)]
        if executable {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(&resolved, perms).ok();
        }
        #[cfg(not(unix))]
        let _ = executable;

        Ok(heap.alloc(RepositoryPath::new(resolved.to_string_lossy().to_string())))
    }

    /// Download a file from a URL.
    fn download<'v>(
        this: &ModuleContext,
        url: Value<'v>,
        #[starlark(default = "")] output: &str,
        #[starlark(require = named, default = "")] sha256: &str,
        #[starlark(require = named, default = "")] integrity: &str,
        #[starlark(require = named, default = false)] executable: bool,
        #[starlark(require = named, default = true)] allow_fail: bool,
        #[starlark(require = named, default = "")] canonical_id: &str,
        #[allow(unused_variables)]
        #[starlark(require = named)]
        auth: Option<Value<'v>>,
        #[allow(unused_variables)]
        #[starlark(require = named)]
        headers: Option<Value<'v>>,
        #[allow(unused_variables)]
        #[starlark(require = named, default = true)]
        block: bool,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let urls = get_urls_from_value(url);
        if urls.is_empty() {
            if allow_fail {
                return Ok(heap.alloc(DownloadInfo {
                    success: false,
                    integrity: String::new(),
                    sha256: String::new(),
                }));
            }
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "No URL provided for download"
            )
            .into());
        }

        // Determine output path
        let output_path = if output.is_empty() {
            let filename = urls[0].split('/').last().unwrap_or("downloaded");
            if let Some(ref wd) = this.working_dir {
                wd.join(filename)
            } else {
                PathBuf::from(filename)
            }
        } else if Path::new(output).is_absolute() {
            PathBuf::from(output)
        } else if let Some(ref wd) = this.working_dir {
            wd.join(output)
        } else {
            PathBuf::from(output)
        };

        module_ctx_record_unpinned_download_file_url_inputs(this, &urls, sha256, integrity)?;
        match crate::repository_ctx::perform_download_to_path(
            &urls,
            &output_path,
            sha256,
            integrity,
            canonical_id,
            executable,
        ) {
            Ok(info) => {
                if block {
                    Ok(heap.alloc(info))
                } else {
                    Ok(heap.alloc(DownloadToken { info }))
                }
            }
            Err(_) if allow_fail => Ok(heap.alloc(DownloadInfo {
                success: false,
                integrity: String::new(),
                sha256: String::new(),
            })),
            Err(e) => Err(e.into()),
        }
    }

    /// Download and extract an archive from a URL.
    fn download_and_extract<'v>(
        this: &ModuleContext,
        url: Value<'v>,
        #[starlark(require = named, default = "")] output: &str,
        #[starlark(require = named, default = "")] sha256: &str,
        #[starlark(require = named, default = "")] integrity: &str,
        #[starlark(require = named, default = "")] strip_prefix: &str,
        #[starlark(require = named, default = "")] _type: &str,
        #[starlark(require = named)] rename_files: Option<Value<'v>>,
        #[starlark(require = named)] _auth: Option<Value<'v>>,
        #[starlark(require = named)] _headers: Option<Value<'v>>,
        #[starlark(require = named, default = "")] canonical_id: &str,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let urls = get_urls_from_value(url);
        if urls.is_empty() {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "No URL provided for download_and_extract"
            )
            .into());
        }

        // Determine output directory
        let output_dir = if output.is_empty() {
            if let Some(ref wd) = this.working_dir {
                wd.as_ref().clone()
            } else {
                PathBuf::from(".")
            }
        } else if Path::new(output).is_absolute() {
            PathBuf::from(output)
        } else if let Some(ref wd) = this.working_dir {
            wd.join(output)
        } else {
            PathBuf::from(output)
        };

        let strip = if strip_prefix.is_empty() {
            None
        } else {
            Some(strip_prefix)
        };
        let rename_files = parse_rename_files(rename_files, "module_ctx.download_and_extract()")?;
        module_ctx_record_unpinned_download_file_url_inputs(this, &urls, sha256, integrity)?;
        match crate::repository_ctx::perform_download_and_extract_to_dir(
            &urls,
            &output_dir,
            sha256,
            integrity,
            canonical_id,
            strip,
            &rename_files,
        ) {
            Ok(info) => Ok(heap.alloc(info)),
            Err(e) => Err(e.into()),
        }
    }

    /// Execute a command and return its output.
    fn execute<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] arguments: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named, default = 600)] _timeout: i32,
        #[starlark(require = named)] environment: Option<Value<'v>>,
        #[starlark(require = named, default = true)] quiet: bool,
        #[starlark(require = named, default = "")] working_directory: &str,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let args: Vec<String> = arguments
            .items
            .iter()
            .map(|v| -> starlark::Result<String> {
                if v.get_type() == "Label" {
                    // Resolve Labels via cell path map (Bazel's getPathFromLabel)
                    let label_str = v.to_str();
                    let path = resolve_module_ctx_label(this, &label_str, "module_ctx.execute()")?;
                    ensure_label_path_materialized(&path);
                    Ok(path.to_string_lossy().to_string())
                } else if let Some(rp) = v.downcast_ref::<crate::repository_ctx::RepositoryPath>() {
                    // RepositoryPath objects (from mctx.path()) → extract path string
                    Ok(rp.path_str().to_owned())
                } else {
                    Ok(v.unpack_str()
                        .map(|s| s.to_owned())
                        .unwrap_or_else(|| v.to_str()))
                }
            })
            .collect::<starlark::Result<_>>()?;

        if args.is_empty() {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "arguments cannot be empty"
            )
            .into());
        }

        let program = &args[0];
        let cmd_args = &args[1..];

        // Set working directory. Canonicalizing avoids executing inside the
        // workspace-root `bazel-external` symlink; Bazel runs external repos
        // from the output base, outside the source workspace.
        let work_dir = if !working_directory.is_empty() {
            Some(std::path::PathBuf::from(working_directory))
        } else {
            this.working_dir
                .as_ref()
                .map(|wd| wd.as_path().to_path_buf())
        };
        let work_dir = work_dir
            .map(|work_dir| crate::repository_ctx::prepare_execute_working_directory(&work_dir))
            .transpose()?;

        let mut cmd = Command::new(match &work_dir {
            Some(work_dir) => crate::repository_ctx::prepare_execute_program(program, work_dir),
            None => std::path::PathBuf::from(program),
        });
        cmd.args(cmd_args);
        if let Some(work_dir) = work_dir {
            cmd.current_dir(work_dir);
        }

        crate::repository_ctx::apply_execute_environment(&mut cmd, this.repo_env(), environment)?;

        let output = cmd.output().map_err(|e| {
            starlark::Error::from(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "Failed to execute command: {}",
                e
            ))
        })?;

        let return_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !quiet {
            if !stdout.is_empty() {
                eprintln!("{}", stdout);
            }
            if !stderr.is_empty() {
                eprintln!("{}", stderr);
            }
        }

        Ok(heap.alloc(ExecutionResult::new(return_code, stdout, stderr)))
    }

    /// Find the path to a program on PATH.
    fn which<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] program: &str,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        if program.contains('/') || program.contains('\\') {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "Program argument of which() may not contain a / or a \\ ('{}' given)",
                program
            )
            .into());
        }
        if program.is_empty() {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "Program argument of which() may not be empty"
            )
            .into());
        }

        this.record_env_input("PATH")?;
        let Some(path_var) = this.repo_env().get("PATH") else {
            return Ok(Value::new_none());
        };
        for dir in std::env::split_paths(path_var).filter(|path| path.is_absolute()) {
            let candidates: Vec<PathBuf> = if cfg!(windows) {
                let base = dir.join(program.trim());
                if base.extension().is_some() {
                    vec![base]
                } else {
                    vec![
                        base.with_extension("exe"),
                        base.with_extension("cmd"),
                        base.with_extension("bat"),
                        base.with_extension("com"),
                        base.clone(),
                    ]
                }
            } else {
                vec![dir.join(program.trim())]
            };

            for full_path in candidates {
                if full_path.is_file() {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if let Ok(meta) = std::fs::metadata(&full_path) {
                            if meta.permissions().mode() & 0o111 != 0 {
                                return Ok(heap.alloc(RepositoryPath::new(
                                    full_path.to_string_lossy().to_string(),
                                )));
                            }
                        }
                    }
                    #[cfg(not(unix))]
                    {
                        return Ok(heap
                            .alloc(RepositoryPath::new(full_path.to_string_lossy().to_string())));
                    }
                }
            }
        }
        Ok(Value::new_none())
    }

    /// Get an environment variable value.
    /// Returns the value as a string, or the default if not set.
    fn getenv<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] name: &str,
        #[starlark(default = starlark::values::none::NoneOr::None)]
        default: starlark::values::none::NoneOr<&str>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        this.record_env_input(name)?;
        match this.repo_env().get(name) {
            Some(v) => Ok(heap.alloc(v)),
            None => match default {
                starlark::values::none::NoneOr::Other(s) => Ok(heap.alloc(s)),
                starlark::values::none::NoneOr::None => Ok(Value::new_none()),
            },
        }
    }

    /// Convert a path or Label to a repository path object.
    ///
    /// Accepts both strings and Label objects. For Labels like
    /// `Label("@repo//:bin/cargo")`, resolves via cell/external paths.
    fn path<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] path: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let path_str = if let Some(s) = path.unpack_str() {
            s.to_owned()
        } else if let Some(repo_path) = path.downcast_ref::<RepositoryPath>() {
            repo_path.path_str().to_owned()
        } else if path.get_type() == "Label" {
            // Handle Label objects: resolve via cell path map (Bazel's getPathFromLabel).
            let label_str = format!("{}", path);
            let resolved = resolve_module_ctx_label(this, &label_str, "module_ctx.path()")?;
            // Plan 36: ensure the spoke is on disk before the caller
            // dereferences the returned path (e.g. with `mctx.execute`).
            ensure_label_path_materialized(&resolved);
            return Ok(heap.alloc(RepositoryPath::new(resolved.to_string_lossy().to_string())));
        } else {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "module_ctx.path() requires a string, Label, or path object, got {}",
                path.get_type()
            )
            .into());
        };

        let resolved = if Path::new(&path_str).is_absolute() {
            PathBuf::from(&path_str)
        } else if let Some(ref wd) = this.working_dir {
            wd.join(&path_str)
        } else {
            PathBuf::from(&path_str)
        };
        Ok(heap.alloc(RepositoryPath::new(resolved.to_string_lossy().to_string())))
    }

    /// Extract a local archive.
    fn extract<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] archive: Value<'v>,
        #[starlark(require = named, default = "")] output: &str,
        #[starlark(require = named, default = "")] strip_prefix: &str,
        #[starlark(require = named)] rename_files: Option<Value<'v>>,
        #[starlark(require = named, default = "auto")] watch_archive: &str,
    ) -> starlark::Result<Value<'v>> {
        let should_watch = ShouldWatch::parse(watch_archive)?;
        let archive_path = resolve_module_ctx_input_path(this, archive, "module_ctx.extract()")?;

        let output_dir = if output.is_empty() {
            if let Some(ref wd) = this.working_dir {
                wd.as_ref().clone()
            } else {
                PathBuf::from(".")
            }
        } else if Path::new(output).is_absolute() {
            PathBuf::from(output)
        } else if let Some(ref wd) = this.working_dir {
            wd.join(output)
        } else {
            PathBuf::from(output)
        };

        this.maybe_record_file_input(&archive_path, should_watch)?;
        let data = std::fs::read(&archive_path).map_err(|e| {
            starlark::Error::from(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "Failed to read archive {}: {}",
                archive_path.display(),
                e
            ))
        })?;

        std::fs::create_dir_all(&output_dir).map_err(|e| {
            starlark::Error::from(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "Failed to create directory: {}",
                e
            ))
        })?;

        let strip = if strip_prefix.is_empty() {
            None
        } else {
            Some(strip_prefix)
        };
        let rename_files = parse_rename_files(rename_files, "module_ctx.extract()")?;
        extract_archive(&data, &output_dir, strip, &rename_files).map_err(|e| {
            starlark::Error::from(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "{}",
                e
            ))
        })?;

        Ok(Value::new_none())
    }

    /// Watch a file or directory for changes.
    fn watch<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] path: Value<'v>,
    ) -> starlark::Result<Value<'v>> {
        let resolved = resolve_module_ctx_input_path(this, path, "module_ctx.watch()")?;
        this.record_file_input(&resolved)?;
        Ok(Value::new_none())
    }

    /// Report an extension's metadata for lockfile storage.
    fn extension_metadata<'v>(
        this: &ModuleContext,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        _root_module_direct_deps: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        _root_module_direct_dev_deps: Value<'v>,
        #[starlark(require = named, default = false)] _reproducible: bool,
        #[starlark(require = named, default = starlark::values::none::NoneType)] facts: Value<'v>,
        #[starlark(kwargs)] _kwargs: Value<'v>,
        eval: &mut starlark::eval::Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        let metadata = slug_bzlmod::ModuleExtensionMetadata {
            facts: validate_facts_value(facts)?,
        };
        Ok(eval
            .heap()
            .alloc(StarlarkModuleExtensionMetadata::new(metadata)))
    }

    /// Check if a path is a directory.
    fn is_dir<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] path: Value<'v>,
    ) -> starlark::Result<bool> {
        let path_str = path.unpack_str().unwrap_or("");
        let resolved = if Path::new(path_str).is_absolute() {
            PathBuf::from(path_str)
        } else if let Some(ref wd) = this.working_dir {
            wd.join(path_str)
        } else {
            PathBuf::from(path_str)
        };
        Ok(resolved.is_dir())
    }

    /// Delete a file or directory. Returns True if the path existed.
    fn delete<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] path: Value<'v>,
    ) -> starlark::Result<bool> {
        let path_str = path.unpack_str().unwrap_or("");
        let resolved = if Path::new(path_str).is_absolute() {
            PathBuf::from(path_str)
        } else if let Some(ref wd) = this.working_dir {
            wd.join(path_str)
        } else {
            PathBuf::from(path_str)
        };
        if resolved.is_dir() {
            std::fs::remove_dir_all(&resolved).ok();
            Ok(true)
        } else if resolved.is_file() {
            std::fs::remove_file(&resolved).ok();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Create a symlink.
    fn symlink<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] target: Value<'v>,
        #[starlark(require = pos)] link: Value<'v>,
    ) -> starlark::Result<Value<'v>> {
        let target_path = resolve_module_ctx_input_path(this, target, "module_ctx.symlink()")?;
        this.maybe_record_file_input(&target_path, ShouldWatch::Auto)?;
        let target_str = target_path.to_string_lossy().to_string();
        let link_str = link.unpack_str().unwrap_or("");

        let resolved_link = if Path::new(link_str).is_absolute() {
            PathBuf::from(link_str)
        } else if let Some(ref wd) = this.working_dir {
            wd.join(link_str)
        } else {
            PathBuf::from(link_str)
        };

        // Ensure parent directory exists
        if let Some(parent) = resolved_link.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        // On Windows, copy instead of symlink (symlinks require privileges)
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&target_str, &resolved_link).map_err(|e| {
                starlark::Error::from(slug_error::slug_error!(
                    slug_error::ErrorTag::Input,
                    "Failed to create symlink {} -> {}: {}",
                    resolved_link.display(),
                    target_str,
                    e
                ))
            })?;
        }
        #[cfg(not(unix))]
        {
            let target_path = if Path::new(target_str).is_absolute() {
                PathBuf::from(target_str)
            } else if let Some(ref wd) = this.working_dir {
                wd.join(target_str)
            } else {
                PathBuf::from(target_str)
            };
            if target_path.is_dir() {
                // Copy directory recursively as fallback
                fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
                    std::fs::create_dir_all(dst)?;
                    for entry in std::fs::read_dir(src)? {
                        let entry = entry?;
                        let ty = entry.file_type()?;
                        if ty.is_dir() {
                            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
                        } else {
                            std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
                        }
                    }
                    Ok(())
                }
                copy_dir_all(&target_path, &resolved_link).map_err(|e| {
                    starlark::Error::from(slug_error::slug_error!(
                        slug_error::ErrorTag::Input,
                        "Failed to copy directory: {}",
                        e
                    ))
                })?;
            } else {
                std::fs::copy(&target_path, &resolved_link).map_err(|e| {
                    starlark::Error::from(slug_error::slug_error!(
                        slug_error::ErrorTag::Input,
                        "Failed to copy file: {}",
                        e
                    ))
                })?;
            }
        }

        Ok(Value::new_none())
    }

    /// Create a file from a template with substitutions.
    fn template<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] path: Value<'v>,
        #[starlark(require = pos)] template: Value<'v>,
        #[starlark(require = named)] substitutions: Option<Value<'v>>,
        #[starlark(require = named, default = false)] executable: bool,
        #[starlark(require = named, default = "auto")] watch_template: &str,
    ) -> starlark::Result<Value<'v>> {
        let should_watch = ShouldWatch::parse(watch_template)?;
        let path_str = path.unpack_str().unwrap_or("");
        let template_path = resolve_module_ctx_input_path(this, template, "module_ctx.template()")?;

        this.maybe_record_file_input(&template_path, should_watch)?;
        let mut content = std::fs::read_to_string(&template_path).map_err(|e| {
            starlark::Error::from(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "Failed to read template {}: {}",
                template_path.display(),
                e
            ))
        })?;

        // Apply substitutions
        if let Some(subs) = substitutions {
            if let Some(dict) = starlark::values::dict::DictRef::from_value(subs) {
                for (k, v) in dict.iter() {
                    if let (Some(key), Some(val)) = (k.unpack_str(), v.unpack_str()) {
                        content = content.replace(key, val);
                    }
                }
            }
        }

        // Write the output file
        let output_path = if Path::new(path_str).is_absolute() {
            PathBuf::from(path_str)
        } else if let Some(ref wd) = this.working_dir {
            wd.join(path_str)
        } else {
            PathBuf::from(path_str)
        };

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        std::fs::write(&output_path, &content).map_err(|e| {
            starlark::Error::from(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "Failed to write template output {}: {}",
                output_path.display(),
                e
            ))
        })?;

        #[cfg(unix)]
        if executable {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(&output_path, perms).ok();
        }
        #[cfg(not(unix))]
        let _ = executable;

        Ok(Value::new_none())
    }

    /// Apply patches.
    fn patch<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] patch_file: Value<'v>,
        #[starlark(require = named, default = 0)] strip: i32,
        #[starlark(require = named, default = "auto")] watch_patch: &str,
    ) -> starlark::Result<Value<'v>> {
        let should_watch = ShouldWatch::parse(watch_patch)?;
        let patch_path = resolve_module_ctx_input_path(this, patch_file, "module_ctx.patch()")?;
        let Some(ref working_dir) = this.working_dir else {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "module_ctx.patch() requires a working directory"
            )
            .into());
        };
        this.maybe_record_file_input(&patch_path, should_watch)?;
        apply_unified_patch(&patch_path, strip, working_dir.as_ref()).map_err(|e| {
            starlark::Error::from(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "{}",
                e
            ))
        })?;
        Ok(Value::new_none())
    }
}

fn module_ctx_record_unpinned_download_file_url_inputs(
    this: &ModuleContext,
    urls: &[String],
    sha256: &str,
    integrity: &str,
) -> starlark::Result<()> {
    for path in slug_bzlmod::unpinned_local_file_url_paths(
        urls.iter().map(String::as_str),
        Some(sha256),
        Some(integrity),
    ) {
        this.record_file_input(&path)?;
    }
    Ok(())
}

fn resolve_module_ctx_input_path(
    this: &ModuleContext,
    value: Value,
    method: &str,
) -> starlark::Result<PathBuf> {
    if let Some(s) = value.unpack_str() {
        if Path::new(s).is_absolute() {
            return Ok(PathBuf::from(s));
        }
        if let Some(ref wd) = this.working_dir {
            return Ok(wd.join(s));
        }
        return Ok(PathBuf::from(s));
    }

    if let Some(repo_path) = value.downcast_ref::<RepositoryPath>() {
        return Ok(repo_path.absolute_path());
    }

    if value.get_type() == "Label" {
        let label_str = format!("{}", value);
        let resolved = resolve_module_ctx_label(this, &label_str, method)?;
        let _ = try_ensure_label_path_materialized(&resolved)?;
        return Ok(resolved);
    }

    Err(slug_error::slug_error!(
        slug_error::ErrorTag::Input,
        "{} requires a string, Label, or path object, got {}",
        method,
        value.get_type()
    )
    .into())
}

fn resolve_module_ctx_label(
    this: &ModuleContext,
    label_str: &str,
    method: &str,
) -> starlark::Result<PathBuf> {
    this.resolve_label_to_filesystem_path(label_str)
        .ok_or_else(|| {
            slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "{} requires resolver-owned bzlmod cell paths to resolve Label '{}'",
                method,
                label_str
            )
            .into()
        })
}
