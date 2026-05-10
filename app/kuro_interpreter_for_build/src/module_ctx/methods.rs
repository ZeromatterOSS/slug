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
//! execute, file) are fully implemented. Watch/template/patch methods remain
//! as no-ops (acceptable for most extensions).

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
use crate::repository_ctx::DownloadInfo;
use crate::repository_ctx::DownloadToken;
use crate::repository_ctx::ExecutionResult;
use crate::repository_ctx::RepositoryPath;
use crate::repository_ctx::ensure_label_path_materialized;
use crate::repository_ctx::extract_archive;
use crate::repository_ctx::get_urls_from_value;
use crate::repository_ctx::resolve_label_to_path;

/// Module context methods for Bazel module extensions.
/// I/O operations (download, execute, file) are fully implemented.
/// Watch/template/patch methods remain as no-ops (acceptable for most extensions).
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
    #[allow(unused_variables)]
    fn read(
        this: &ModuleContext,
        #[starlark(require = pos)] path: Value,
        #[starlark(require = named, default = "auto")] watch: &str,
    ) -> starlark::Result<String> {
        let resolved = if let Some(s) = path.unpack_str() {
            if Path::new(s).is_absolute() {
                PathBuf::from(s)
            } else if let Some(ref wd) = this.working_dir {
                wd.join(s)
            } else {
                PathBuf::from(s)
            }
        } else if let Some(repo_path) = path.downcast_ref::<RepositoryPath>() {
            repo_path.absolute_path()
        } else if path.get_type() == "Label" {
            let label_str = format!("{}", path);
            let resolved = if let Some(resolved) = this.resolve_label_to_filesystem_path(&label_str)
            {
                resolved
            } else {
                let workspace_root = this
                    .working_dir
                    .as_ref()
                    .map(|wd| wd.as_ref().as_path())
                    .unwrap_or_else(|| Path::new("."));
                PathBuf::from(resolve_label_to_path(&label_str, workspace_root))
            };
            // Plan 36: drive lazy spoke materialization before the read.
            ensure_label_path_materialized(&resolved);
            resolved
        } else {
            return Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "module_ctx.read() requires a string, Label, or path object, got {}",
                path.get_type()
            )
            .into());
        };
        let path_str = resolved.to_string_lossy().to_string();
        let content = std::fs::read_to_string(&resolved).map_err(|e| {
            let working_dir = this
                .working_dir
                .as_ref()
                .map(|wd| wd.display().to_string())
                .unwrap_or_else(|| "<none>".to_owned());
            kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
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
            return Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "module_ctx.file() requires a working directory or absolute path"
            )
            .into());
        };

        // Ensure parent directory exists
        if let Some(parent) = resolved.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                starlark::Error::from(kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "Failed to create parent directory for {}: {}",
                    resolved.display(),
                    e
                ))
            })?;
        }

        std::fs::write(&resolved, content).map_err(|e| {
            starlark::Error::from(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
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
        #[allow(unused_variables)]
        #[starlark(require = named, default = "")]
        canonical_id: &str,
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
            return Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
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

        match crate::repository_ctx::perform_download_to_path(
            &urls,
            &output_path,
            sha256,
            integrity,
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
        #[starlark(require = named)] _rename_files: Option<Value<'v>>,
        #[starlark(require = named)] _auth: Option<Value<'v>>,
        #[starlark(require = named)] _headers: Option<Value<'v>>,
        #[starlark(require = named, default = "")] _canonical_id: &str,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let urls = get_urls_from_value(url);
        if urls.is_empty() {
            return Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
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
        match crate::repository_ctx::perform_download_and_extract_to_dir(
            &urls,
            &output_dir,
            sha256,
            integrity,
            strip,
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
            .map(|v| {
                if v.get_type() == "Label" {
                    // Resolve Labels via cell path map (Bazel's getPathFromLabel)
                    let label_str = v.to_str();
                    if let Some(path) = this.resolve_label_to_filesystem_path(&label_str) {
                        ensure_label_path_materialized(&path);
                        path.to_string_lossy().to_string()
                    } else {
                        label_str
                    }
                } else if let Some(rp) = v.downcast_ref::<crate::repository_ctx::RepositoryPath>() {
                    // RepositoryPath objects (from mctx.path()) → extract path string
                    rp.path_str().to_owned()
                } else {
                    v.unpack_str()
                        .map(|s| s.to_owned())
                        .unwrap_or_else(|| v.to_str())
                }
            })
            .collect();

        if args.is_empty() {
            return Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "arguments cannot be empty"
            )
            .into());
        }

        let program = &args[0];
        let cmd_args = &args[1..];

        let mut cmd = Command::new(program);
        cmd.args(cmd_args);

        // Set working directory
        if !working_directory.is_empty() {
            cmd.current_dir(working_directory);
        } else if let Some(ref wd) = this.working_dir {
            cmd.current_dir(wd.as_path());
        }

        // Set environment variables if provided
        if let Some(env_val) = environment {
            if let Some(env_dict) = starlark::values::dict::DictRef::from_value(env_val) {
                for (k, v) in env_dict.iter() {
                    if let (Some(key), Some(val)) = (k.unpack_str(), v.unpack_str()) {
                        cmd.env(key, val);
                    }
                }
            }
        }

        let output = cmd.output().map_err(|e| {
            starlark::Error::from(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
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
        let _ = this;
        if let Ok(path_var) = std::env::var("PATH") {
            let separator = if cfg!(windows) { ';' } else { ':' };
            for dir in path_var.split(separator) {
                let candidates: Vec<PathBuf> = if cfg!(windows) {
                    let base = Path::new(dir).join(program);
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
                    vec![Path::new(dir).join(program)]
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
                            return Ok(heap.alloc(RepositoryPath::new(
                                full_path.to_string_lossy().to_string(),
                            )));
                        }
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
        let _ = this;
        match std::env::var(name) {
            Ok(v) => Ok(heap.alloc(v)),
            Err(_) => match default {
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
            if let Some(resolved) = this.resolve_label_to_filesystem_path(&label_str) {
                // Plan 36: ensure the spoke is on disk before the caller
                // dereferences the returned path (e.g. with `mctx.execute`).
                ensure_label_path_materialized(&resolved);
                return Ok(heap.alloc(RepositoryPath::new(resolved.to_string_lossy().to_string())));
            }
            // Fallback to legacy resolution if cell paths not available
            let workspace_root = this
                .working_dir
                .as_ref()
                .map(|wd| wd.as_ref().as_path())
                .unwrap_or_else(|| Path::new("."));
            let legacy = resolve_label_to_path(&label_str, workspace_root);
            ensure_label_path_materialized(Path::new(&legacy));
            legacy
        } else {
            return Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
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
        #[starlark(require = named)] _rename_files: Option<Value<'v>>,
        #[starlark(require = named, default = false)] _watch_archive: bool,
    ) -> starlark::Result<Value<'v>> {
        let archive_str = archive.unpack_str().unwrap_or("");
        let archive_path = if Path::new(archive_str).is_absolute() {
            PathBuf::from(archive_str)
        } else if let Some(ref wd) = this.working_dir {
            wd.join(archive_str)
        } else {
            PathBuf::from(archive_str)
        };

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

        let data = std::fs::read(&archive_path).map_err(|e| {
            starlark::Error::from(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "Failed to read archive {}: {}",
                archive_path.display(),
                e
            ))
        })?;

        std::fs::create_dir_all(&output_dir).map_err(|e| {
            starlark::Error::from(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "Failed to create directory: {}",
                e
            ))
        })?;

        let strip = if strip_prefix.is_empty() {
            None
        } else {
            Some(strip_prefix)
        };
        extract_archive(&data, &output_dir, strip).map_err(|e| {
            starlark::Error::from(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "{}",
                e
            ))
        })?;

        Ok(Value::new_none())
    }

    /// Watch a file or directory for changes.
    /// STUB: Returns None.
    fn watch<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] _path: Value<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(Value::new_none())
    }

    /// Report an extension's metadata for IDE integration.
    /// STUB: Returns None. Accepts arbitrary kwargs for forward compatibility.
    fn extension_metadata<'v>(
        this: &ModuleContext,
        #[starlark(kwargs)] _kwargs: Value<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(Value::new_none())
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
        let target_str = target.unpack_str().unwrap_or("");
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
            std::os::unix::fs::symlink(target_str, &resolved_link).map_err(|e| {
                starlark::Error::from(kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
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
                    starlark::Error::from(kuro_error::kuro_error!(
                        kuro_error::ErrorTag::Input,
                        "Failed to copy directory: {}",
                        e
                    ))
                })?;
            } else {
                std::fs::copy(&target_path, &resolved_link).map_err(|e| {
                    starlark::Error::from(kuro_error::kuro_error!(
                        kuro_error::ErrorTag::Input,
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
    ) -> starlark::Result<Value<'v>> {
        let path_str = path.unpack_str().unwrap_or("");
        let template_str = template.unpack_str().unwrap_or("");

        // Read the template file
        let template_path = if Path::new(template_str).is_absolute() {
            PathBuf::from(template_str)
        } else if let Some(ref wd) = this.working_dir {
            wd.join(template_str)
        } else {
            PathBuf::from(template_str)
        };

        let mut content = std::fs::read_to_string(&template_path).map_err(|e| {
            starlark::Error::from(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
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
            starlark::Error::from(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
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
    /// STUB: Returns None.
    fn patch<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] _patch_file: Value<'v>,
        #[starlark(require = named, default = 0)] _strip: i32,
    ) -> starlark::Result<Value<'v>> {
        Ok(Value::new_none())
    }
}
