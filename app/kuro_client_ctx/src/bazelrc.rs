/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! `.bazelrc` file parser and argument injector.
//!
//! Bazel loads `.bazelrc` files to apply default flags to each command.
//! Kuro reads these files in the same order Bazel does:
//!
//!  1. `$HOME/.bazelrc` (user-level)
//!  2. `<workspace>/.bazelrc` (workspace-level, highest priority)
//!
//! Flags from `.bazelrc` are injected right after the subcommand in the argument
//! list, so explicit command-line flags always override them.
//!
//! ## Syntax
//!
//! ```
//! # Comment line
//! common --verbose        # applies to all commands
//! build --jobs=8          # applies to `kuro build` only
//! test --test_output=all  # applies to `kuro test` only
//! import /path/to/other.bazelrc           # required include
//! try-import /path/to/optional.bazelrc    # optional include (silently ignored)
//! ```
//!
//! Named configs (`build:myconfig --flag`) are collected but only applied when
//! `--config=myconfig` is present in the args.

use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Known kuro subcommands (used to find injection point in argv).
static KNOWN_COMMANDS: &[&str] = &[
    "audit",
    "aquery",
    "bxl",
    "build",
    "clean",
    "clean-stale",
    "cquery",
    "ctargets",
    "debug",
    "expand-external-cell",
    "explain",
    "help",
    "help-env",
    "info",
    "init",
    "install",
    "kill",
    "killall",
    "log",
    "lsp",
    "profile",
    "query",
    "rage",
    "root",
    "run",
    "server",
    "starlark",
    "status",
    "subscribe",
    "targets",
    "test",
    "uquery",
];

/// A single line parsed from a `.bazelrc` file.
#[derive(Debug)]
enum BazelRcLine {
    /// `<command> [:<config>] <flags...>` - flags for a command (optionally a named config)
    Flags {
        command: String,
        config: Option<String>,
        flags: Vec<String>,
    },
    /// `import <path>` - required include
    Import(PathBuf),
    /// `try-import <path>` - optional include
    TryImport(PathBuf),
}

/// Parse a single `.bazelrc` line into a `BazelRcLine`, or `None` if it should be skipped.
fn parse_line(line: &str) -> Option<BazelRcLine> {
    // Trim inline comments (anything after ` #` that is not inside a flag value).
    // Per Bazel semantics, `#` starts a comment only when preceded by whitespace.
    let line = {
        let mut result = line;
        // Find ` #` that isn't inside a quoted string (simplified: just look for first " #")
        let mut in_quotes = false;
        let bytes = line.as_bytes();
        let mut comment_start = None;
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'"' | b'\'' => in_quotes = !in_quotes,
                b'#' if !in_quotes && i > 0 && bytes[i - 1] == b' ' => {
                    comment_start = Some(i);
                    break;
                }
                _ => {}
            }
            i += 1;
        }
        if let Some(pos) = comment_start {
            result = &line[..pos].trim_end();
        }
        result.trim()
    };

    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    // Handle `import` and `try-import`
    if let Some(rest) = line.strip_prefix("import ") {
        let path = rest.trim();
        return Some(BazelRcLine::Import(PathBuf::from(path)));
    }
    if let Some(rest) = line.strip_prefix("try-import ") {
        let path = rest.trim();
        return Some(BazelRcLine::TryImport(PathBuf::from(path)));
    }

    // Otherwise: `<command>[:<config>] <flags...>`
    let (command_part, rest) = match line.split_once(char::is_whitespace) {
        Some((cmd, rest)) => (cmd, rest.trim()),
        None => {
            // Line like `build` with no flags - valid but no flags to add
            return Some(BazelRcLine::Flags {
                command: line.to_owned(),
                config: None,
                flags: Vec::new(),
            });
        }
    };

    let (command, config) = if let Some((cmd, cfg)) = command_part.split_once(':') {
        (cmd.to_owned(), Some(cfg.to_owned()))
    } else {
        (command_part.to_owned(), None)
    };

    // Split rest into individual flags (respecting quoted strings)
    let flags = split_flags(rest);

    Some(BazelRcLine::Flags {
        command,
        config,
        flags,
    })
}

/// Split a string of flags (like `--jobs=4 --verbose "some value"`) into individual tokens,
/// respecting quoted strings.
fn split_flags(s: &str) -> Vec<String> {
    let mut flags = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double => {
                in_single = !in_single;
            }
            '"' if !in_single => {
                in_double = !in_double;
            }
            '\\' if in_double => {
                // Escape sequence inside double quotes
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            c if c.is_whitespace() && !in_single && !in_double => {
                if !current.is_empty() {
                    flags.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        flags.push(current);
    }
    flags
}

/// Parsed representation of a `.bazelrc` file (or multiple files merged).
#[derive(Default)]
struct BazelRcData {
    /// flags: (command, config, flags)
    entries: Vec<(String, Option<String>, Vec<String>)>,
}

/// Flags that are processed during bazelrc loading and should not be passed to clap.
/// These are either handled by the bazelrc system itself or conflict with
/// Buck2/kuro's own flag definitions.
const BAZELRC_ONLY_FLAGS: &[&str] = &[
    "--enable-platform-specific-config",
    "--enable_platform_specific_config",
    "--config", // Bazel named config - conflicts with Buck2's `-c SECTION.KEY=VALUE`
];

/// Flags that are only valid on the `test` command and should be stripped
/// when injecting `common` flags into other commands like `build`.
const TEST_ONLY_FLAGS: &[&str] = &[
    "--test-output",
    "--test_output",
    "--flaky-test-attempts",
    "--flaky_test_attempts",
    "--test-strategy",
    "--test_strategy",
    "--test-summary",
    "--test_summary",
    "--test-timeout",
    "--test_timeout",
    "--runs-per-test",
    "--runs_per_test",
    "--test-sharding-strategy",
    "--test_sharding_strategy",
];

/// Check if a flag is a test-only flag (should not be applied to non-test commands).
fn is_test_only_flag(flag: &str) -> bool {
    let flag_name = flag.split('=').next().unwrap_or(flag);
    TEST_ONLY_FLAGS.iter().any(|f| flag_name == *f)
}

impl BazelRcData {
    /// Extract flags applicable to `command`. Returns flags from:
    /// - `common` lines (no config)
    /// - `<command>` lines (no config)
    /// - `<command>:<config>` lines for each `config` in `active_configs`
    ///
    /// When `common` flags are applied to a non-test command, test-specific
    /// flags are filtered out to avoid "unknown argument" errors.
    fn flags_for(&self, command: &str, active_configs: &[String]) -> Vec<String> {
        let mut result = Vec::new();
        for (cmd, cfg, flags) in &self.entries {
            let matches_command = cmd == "common" || cmd == command;
            if !matches_command {
                continue;
            }
            let is_from_common = cmd == "common";
            match cfg {
                None => {
                    for flag in flags {
                        let flag_name = flag.split('=').next().unwrap_or(flag);
                        // Strip flags that are processed during bazelrc loading
                        if BAZELRC_ONLY_FLAGS.contains(&flag_name) {
                            continue;
                        }
                        // Strip test-only flags when injecting `common` into non-test commands
                        if is_from_common && command != "test" && is_test_only_flag(flag) {
                            continue;
                        }
                        result.push(flag.clone());
                    }
                }
                Some(config_name) if active_configs.contains(config_name) => {
                    for flag in flags {
                        let flag_name = flag.split('=').next().unwrap_or(flag);
                        if BAZELRC_ONLY_FLAGS.contains(&flag_name) {
                            continue;
                        }
                        if is_from_common && command != "test" && is_test_only_flag(flag) {
                            continue;
                        }
                        result.push(flag.clone());
                    }
                }
                _ => {}
            }
        }
        result
    }
}

/// Substitute `%workspace%` with the workspace root in a path string.
fn substitute_workspace(s: &str, workspace_root: Option<&Path>) -> String {
    if let Some(root) = workspace_root {
        s.replace("%workspace%", &root.to_string_lossy())
    } else {
        s.to_owned()
    }
}

/// Parse a `.bazelrc` file at `path`, merging results into `data`.
/// If `required` is false (try-import), silently ignores missing files.
/// `workspace_root` is used to substitute `%workspace%` in paths and flag values.
fn parse_bazelrc_file(
    path: &Path,
    data: &mut BazelRcData,
    required: bool,
    workspace_root: Option<&Path>,
) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => {
            if required {
                tracing::warn!("Could not read .bazelrc file: {}", path.display());
            }
            return;
        }
    };

    for line in content.lines() {
        match parse_line(line) {
            Some(BazelRcLine::Flags {
                command,
                config,
                flags,
            }) => {
                // Substitute %workspace% in flag values
                let flags = flags
                    .into_iter()
                    .map(|f| substitute_workspace(&f, workspace_root))
                    .collect();
                data.entries.push((command, config, flags));
            }
            Some(BazelRcLine::Import(import_path)) => {
                let substituted =
                    substitute_workspace(&import_path.to_string_lossy(), workspace_root);
                let import_path = PathBuf::from(substituted);
                let resolved = if import_path.is_absolute() {
                    import_path
                } else {
                    path.parent()
                        .map(|p| p.join(&import_path))
                        .unwrap_or(import_path)
                };
                parse_bazelrc_file(&resolved, data, true, workspace_root);
            }
            Some(BazelRcLine::TryImport(import_path)) => {
                let substituted =
                    substitute_workspace(&import_path.to_string_lossy(), workspace_root);
                let import_path = PathBuf::from(substituted);
                let resolved = if import_path.is_absolute() {
                    import_path
                } else {
                    path.parent()
                        .map(|p| p.join(&import_path))
                        .unwrap_or(import_path)
                };
                parse_bazelrc_file(&resolved, data, false, workspace_root);
            }
            None => {}
        }
    }
}

/// Find any `--config=name` values in the args list.
fn find_active_configs(args: &[String]) -> Vec<String> {
    let mut configs = Vec::new();
    for arg in args {
        if let Some(config) = arg.strip_prefix("--config=") {
            configs.push(config.to_owned());
        }
    }
    configs
}

/// Normalize a Bazel-style flag argument to kuro-compatible form.
///
/// Bazel allows both `--keep_going` and `--keep-going` interchangeably.
/// Kuro's clap flags use hyphens, so we normalize underscores → hyphens
/// in the flag name part (before any `=` sign).
///
/// Only normalizes `--long_flags`, not values, short flags, or bare args.
///
/// Examples:
/// - `--keep_going` → `--keep-going`
/// - `--test_output=all` → `--test-output=all`
/// - `-c opt` → unchanged (short flag)
/// - `//my:target` → unchanged (not a flag)
fn normalize_flag(arg: &str) -> String {
    // Only process long flags with underscores
    if !arg.starts_with("--") || !arg.contains('_') {
        return arg.to_owned();
    }
    // Don't normalize single-element args that look like `--` (separator)
    if arg == "--" {
        return arg.to_owned();
    }
    // Split on = to get flag name and value parts
    let (flag_part, value_part) = match arg.find('=') {
        Some(pos) => (&arg[..pos], &arg[pos..]),
        None => (arg, ""),
    };
    // Replace underscores with hyphens in the flag name only
    let normalized_flag = flag_part.replace('_', "-");
    format!("{}{}", normalized_flag, value_part)
}

/// Normalize all args: convert Bazel underscore flags to hyphen flags, and
/// strip Bazel-only transitional flags that kuro doesn't understand.
///
/// Bazel treats `--flag_name` and `--flag-name` as equivalent; kuro uses hyphens.
///
/// Transitional flags like `--incompatible_*` and `--noincompatible_*` are Bazel
/// migration flags that enable future-default behavior. Kuro already implements
/// current Bazel semantics, so these are silently stripped.
///
/// NOTE: normalization stops after `--`. Everything after `--` is passed to a
/// subcommand (e.g., BXL script args) and must be preserved verbatim.
/// Process-level storage for Starlark build flags extracted from the command line.
/// Flags like `--//pkg:target=value` are extracted before clap parsing since
/// clap can't handle the `--//` prefix.
static STARLARK_FLAGS: OnceLock<Vec<String>> = OnceLock::new();

/// Get the Starlark build flags that were extracted from the command line.
pub fn get_starlark_flags_from_args() -> &'static [String] {
    STARLARK_FLAGS.get().map(|v| v.as_slice()).unwrap_or(&[])
}

pub fn normalize_args(args: Vec<String>) -> Vec<String> {
    let mut result = Vec::with_capacity(args.len());
    let mut starlark_flags = Vec::new();
    let mut past_separator = false;
    for arg in args {
        if past_separator {
            result.push(arg);
        } else if arg == "--" {
            past_separator = true;
            result.push(arg);
        } else if arg.starts_with("--//") || arg.starts_with("--@") {
            // Starlark build flag: --//pkg:target=value or --@repo//pkg:target=value
            // Extract and store, don't pass to clap
            starlark_flags.push(arg[2..].to_owned()); // strip leading --
        } else if !is_bazel_transitional_flag(&arg) {
            // Handle Bazel --noflag_name boolean negation pattern
            if let Some(replacement) = normalize_bazel_negation(&arg) {
                if !replacement.is_empty() {
                    result.push(replacement);
                }
                // else: drop the flag entirely (negation of a boolean = default)
            } else {
                result.push(normalize_flag(&arg));
            }
        }
    }
    let _ = STARLARK_FLAGS.set(starlark_flags);
    result
}

/// Returns true for Bazel-only transitional flags that kuro should silently ignore.
///
/// These include:
/// - `--incompatible_*` / `--noincompatible_*`: Bazel migration flags
/// - `--legacy_*` / `--nolegacy_*`: Legacy behavior toggles
/// - `--experimental_*` / `--noexperimental_*`: Experimental features
/// - Various Bazel-specific build flags (sandboxing, PIC, runfiles, etc.)
fn is_bazel_transitional_flag(arg: &str) -> bool {
    // Must start with -- (long flag)
    let flag_name = if let Some(stripped) = arg.strip_prefix("--") {
        stripped
    } else {
        return false;
    };
    // Get just the flag name part (before =)
    let flag_name = flag_name.split('=').next().unwrap_or(flag_name);
    // Strip leading "no" prefix for boolean flags
    let base = flag_name.strip_prefix("no").unwrap_or(flag_name);
    // Check against transitional prefixes
    if base.starts_with("incompatible_")
        || base.starts_with("incompatible-")
        || base.starts_with("legacy_")
        || base.starts_with("legacy-")
        || base.starts_with("experimental_")
        || base.starts_with("experimental-")
    {
        return true;
    }
    // Normalize for comparison
    let normalized = flag_name.replace('-', "_");
    // Bazel-specific flags that have no kuro equivalent and should be silently dropped.
    // These are internal Bazel build/execution flags that control sandboxing, caching,
    // compilation modes, and other Bazel-specific behavior.
    is_bazel_specific_flag(&normalized)
}

/// Bazel-specific flags that kuro should silently ignore from .bazelrc files.
///
/// These are flags that control Bazel-internal behavior (sandboxing, caching,
/// remote execution details, compilation modes) that have no direct kuro
/// equivalent. Silently dropping them allows kuro to work with existing
/// `.bazelrc` files without modification.
fn is_bazel_specific_flag(normalized_flag: &str) -> bool {
    // Strip leading "no" for boolean flags
    let base = normalized_flag
        .strip_prefix("no")
        .unwrap_or(normalized_flag);
    matches!(
        base,
        // Workspace/bzlmod toggles (kuro always uses bzlmod)
        "enable_workspace"
            | "enable_bzlmod"
            // Build behavior flags
            | "guard_against_concurrent_changes"
            | "force_pic"
            | "dynamic_mode"
            | "strip"
            | "features"
            | "build_runfile_links"
            | "build_runfile_manifests"
            | "process_headers_in_dependencies"
            // Sandbox flags
            | "sandbox_base"
            | "sandbox_default_allow_network"
            | "sandbox_fake_hostname"
            | "sandbox_fake_username"
            // Remote execution / caching flags
            | "remote_upload_local_results"
            | "remote_accept_cached"
            | "remote_cache"
            | "remote_executor"
            | "remote_default_exec_properties"
            | "remote_local_fallback"
            | "remote_timeout"
            // Compilation / output flags
            | "compilation_mode"
            | "copt"
            | "cxxopt"
            | "host_copt"
            | "host_cxxopt"
            | "linkopt"
            | "host_linkopt"
            | "repo_env"
            | "compiler"
            // Runfiles / symlinks
            | "build_runfile_links"
            // Test flags
            | "test_output"
            | "test_summary"
            | "test_tag_filters"
            | "build_tag_filters"
            | "test_sharding_strategy"
            // Misc Bazel flags
            | "keep_going"
            | "stamp"
            | "check_visibility"
    )
}

/// Convert Bazel `--noflag_name` boolean negation to a form clap can handle.
///
/// Bazel supports `--noflag_name` as negation of `--flag_name` for all boolean flags.
/// Clap doesn't understand this pattern, so we either:
/// - Strip it entirely (the flag was just "set to false" which is the default), or
/// - Map it to a known negation alias.
///
/// Returns `Some(normalized)` if the flag was a `--no<name>` pattern, `None` otherwise.
fn normalize_bazel_negation(arg: &str) -> Option<String> {
    // Must start with --no (but not --no-something which is already a valid clap long flag)
    let flag_part = arg.strip_prefix("--no")?;
    // Skip if it starts with '-' (already a --no-flag form) or is empty
    if flag_part.is_empty() || flag_part.starts_with('-') {
        return None;
    }
    // Get just the flag name part (before =)
    let flag_name = flag_part.split('=').next().unwrap_or(flag_part);
    // Known boolean flags that Bazel uses --no<name> for.
    // We silently drop these since the default behavior is already "off".
    let known_negatable = [
        "remote_upload_local_results",
        "remote-upload-local-results",
        "build_runfile_links",
        "build-runfile-links",
        "sandbox_default_allow_network",
        "sandbox-default-allow-network",
        "cache_test_results",
        "cache-test-results",
        "remote_accept_cached",
        "remote-accept-cached",
        "stamp",
        "check_visibility",
        "check-visibility",
        "enable_workspace",
        "enable-workspace",
        "enable_bzlmod",
        "enable-bzlmod",
    ];
    // Normalize underscores to hyphens for comparison
    let normalized_name = flag_name.replace('_', "-");
    let hyphen_names: Vec<String> = known_negatable
        .iter()
        .map(|n| n.replace('_', "-"))
        .collect();
    if hyphen_names.contains(&normalized_name) {
        // Drop the flag - it's just "set this boolean to false" which is the default
        Some(String::new())
    } else {
        None
    }
}

/// Inject flags from `.bazelrc` files into the args list.
///
/// Returns the modified args. If bazelrc loading is disabled via `--nobazelrc`
/// or `--bazelrc=none` in the args, returns the original args unchanged.
///
/// # Injection point
///
/// Flags are injected right after the subcommand, so they have lower precedence
/// than explicit command-line flags:
/// ```text
/// kuro build --from-cmdline
/// # ~/.bazelrc: build --from-bazelrc
/// # becomes: kuro build --from-bazelrc --from-cmdline
/// ```
pub fn inject_bazelrc_args(mut args: Vec<String>, project_root: Option<&Path>) -> Vec<String> {
    // Check for --nobazelrc or --bazelrc=none to disable loading
    for arg in &args {
        if arg == "--nobazelrc"
            || arg == "--bazelrc=none"
            || arg == "--no_bazelrc"
            || arg == "--ignore_all_rc_files"
        {
            return args;
        }
    }

    // Find the subcommand position (first non-flag arg after argv[0])
    let cmd_pos = args
        .iter()
        .enumerate()
        .skip(1) // skip argv[0]
        .find(|(_, a)| !a.starts_with('-') && KNOWN_COMMANDS.contains(&a.as_str()))
        .map(|(i, _)| i);

    let (command_name, insert_pos) = match cmd_pos {
        Some(pos) => (args[pos].clone(), pos + 1),
        None => return args, // no known subcommand found
    };

    // Load bazelrc data from standard locations
    let mut data = BazelRcData::default();

    // 1. User-level: ~/.bazelrc
    if let Some(home) = dirs::home_dir() {
        let user_bazelrc = home.join(".bazelrc");
        if user_bazelrc.exists() {
            parse_bazelrc_file(&user_bazelrc, &mut data, false, project_root);
        }
    }

    // 2. Workspace-level: <project_root>/.bazelrc
    if let Some(root) = project_root {
        let workspace_bazelrc = root.join(".bazelrc");
        if workspace_bazelrc.exists() {
            parse_bazelrc_file(&workspace_bazelrc, &mut data, false, project_root);
        }
    }

    // Collect active --config= names from the user's command-line args
    let mut active_configs = find_active_configs(&args);

    // Also find --config= in the bazelrc data itself (unconditional entries)
    // This handles `build --config=dev` in .bazelrc
    for (cmd, cfg, flags) in &data.entries {
        if cfg.is_none() && (cmd == "common" || cmd == &command_name) {
            for flag in flags {
                if let Some(config) = flag.strip_prefix("--config=") {
                    if !active_configs.contains(&config.to_owned()) {
                        active_configs.push(config.to_owned());
                    }
                }
            }
        }
    }

    // --enable_platform_specific_config: auto-activate build:<os> config
    // Check if this flag is present anywhere in the args or bazelrc data
    let has_platform_config = args.iter().any(|a| {
        a == "--enable-platform-specific-config" || a == "--enable_platform_specific_config"
    }) || data.entries.iter().any(|(_, _, flags)| {
        flags.iter().any(|f| {
            f == "--enable-platform-specific-config" || f == "--enable_platform_specific_config"
        })
    });
    if has_platform_config {
        let os_config = if cfg!(target_os = "linux") {
            "linux"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "macos") {
            "macos"
        } else {
            ""
        };
        if !os_config.is_empty() && !active_configs.contains(&os_config.to_owned()) {
            active_configs.push(os_config.to_owned());
        }
    }

    // Get flags applicable to this command
    let bazelrc_flags = data.flags_for(&command_name, &active_configs);

    if bazelrc_flags.is_empty() {
        return args;
    }

    // Insert the bazelrc flags right after the subcommand
    let tail = args.split_off(insert_pos);
    args.extend(bazelrc_flags);
    args.extend(tail);
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_line_skip_empty() {
        assert!(parse_line("").is_none());
        assert!(parse_line("   ").is_none());
        assert!(parse_line("# comment").is_none());
    }

    #[test]
    fn test_parse_line_flags() {
        let line = parse_line("build --jobs=4 --verbose");
        match line {
            Some(BazelRcLine::Flags {
                command,
                config,
                flags,
            }) => {
                assert_eq!(command, "build");
                assert!(config.is_none());
                assert_eq!(flags, vec!["--jobs=4", "--verbose"]);
            }
            _ => panic!("Expected Flags variant"),
        }
    }

    #[test]
    fn test_parse_line_config() {
        let line = parse_line("build:opt --copt=-O2");
        match line {
            Some(BazelRcLine::Flags {
                command,
                config,
                flags,
            }) => {
                assert_eq!(command, "build");
                assert_eq!(config.as_deref(), Some("opt"));
                assert_eq!(flags, vec!["--copt=-O2"]);
            }
            _ => panic!("Expected Flags variant"),
        }
    }

    #[test]
    fn test_parse_line_import() {
        match parse_line("import /etc/bazel.bazelrc") {
            Some(BazelRcLine::Import(path)) => {
                assert_eq!(path, PathBuf::from("/etc/bazel.bazelrc"));
            }
            _ => panic!("Expected Import variant"),
        }
    }

    #[test]
    fn test_parse_line_try_import() {
        match parse_line("try-import /etc/bazel.bazelrc") {
            Some(BazelRcLine::TryImport(path)) => {
                assert_eq!(path, PathBuf::from("/etc/bazel.bazelrc"));
            }
            _ => panic!("Expected TryImport variant"),
        }
    }

    #[test]
    fn test_inject_no_bazelrc_flag() {
        let args = vec![
            "kuro".to_owned(),
            "--nobazelrc".to_owned(),
            "build".to_owned(),
            "//...".to_owned(),
        ];
        let result = inject_bazelrc_args(args.clone(), None);
        assert_eq!(result, args);
    }

    #[test]
    fn test_flags_for_command() {
        let mut data = BazelRcData::default();
        data.entries
            .push(("common".to_owned(), None, vec!["--verbose".to_owned()]));
        data.entries
            .push(("build".to_owned(), None, vec!["--jobs=4".to_owned()]));
        data.entries.push((
            "test".to_owned(),
            None,
            vec!["--test_output=all".to_owned()],
        ));

        let flags = data.flags_for("build", &[]);
        assert_eq!(flags, vec!["--verbose", "--jobs=4"]);

        let flags = data.flags_for("test", &[]);
        assert_eq!(flags, vec!["--verbose", "--test_output=all"]);
    }

    #[test]
    fn test_flags_for_named_config() {
        let mut data = BazelRcData::default();
        data.entries
            .push(("build".to_owned(), None, vec!["--jobs=4".to_owned()]));
        data.entries.push((
            "build".to_owned(),
            Some("opt".to_owned()),
            vec!["--copt=-O2".to_owned()],
        ));

        // Without --config=opt
        let flags = data.flags_for("build", &[]);
        assert_eq!(flags, vec!["--jobs=4"]);

        // With --config=opt
        let flags = data.flags_for("build", &["opt".to_owned()]);
        assert_eq!(flags, vec!["--jobs=4", "--copt=-O2"]);
    }

    #[test]
    fn test_inline_comment_stripped() {
        let line = parse_line("build --jobs=4 # this is a comment");
        match line {
            Some(BazelRcLine::Flags { flags, .. }) => {
                assert_eq!(flags, vec!["--jobs=4"]);
            }
            _ => panic!("Expected Flags variant"),
        }
    }

    #[test]
    fn test_normalize_flag() {
        // Underscore flags get normalized
        assert_eq!(normalize_flag("--keep_going"), "--keep-going");
        assert_eq!(normalize_flag("--test_output=all"), "--test-output=all");
        assert_eq!(normalize_flag("--num_threads=4"), "--num-threads=4");
        // Short flags unchanged
        assert_eq!(normalize_flag("-c"), "-c");
        // Separator unchanged
        assert_eq!(normalize_flag("--"), "--");
        // Bare args unchanged
        assert_eq!(normalize_flag("//my:target"), "//my:target");
        // Already hyphenated unchanged
        assert_eq!(normalize_flag("--keep-going"), "--keep-going");
        // No underscores → unchanged
        assert_eq!(normalize_flag("--verbose"), "--verbose");
    }

    #[test]
    fn test_is_bazel_transitional_flag() {
        // incompatible_* flags are stripped
        assert!(is_bazel_transitional_flag(
            "--incompatible_enable_cc_toolchain_resolution"
        ));
        assert!(is_bazel_transitional_flag(
            "--incompatible-enable-cc-toolchain-resolution"
        ));
        assert!(is_bazel_transitional_flag(
            "--noincompatible_enable_cc_toolchain_resolution"
        ));
        // legacy_* flags are stripped
        assert!(is_bazel_transitional_flag("--legacy_whole_archive"));
        assert!(is_bazel_transitional_flag("--nolegacy_whole_archive"));
        // experimental_* flags are stripped
        assert!(is_bazel_transitional_flag(
            "--experimental_cc_implementation_deps"
        ));
        // Normal flags are NOT stripped
        assert!(!is_bazel_transitional_flag("--keep-going"));
        assert!(!is_bazel_transitional_flag("--verbose_failures"));
        assert!(!is_bazel_transitional_flag("--jobs=8"));
        assert!(!is_bazel_transitional_flag("//my:target"));
        assert!(!is_bazel_transitional_flag("-c"));
    }

    #[test]
    fn test_normalize_args_strips_transitional() {
        let args = vec![
            "kuro".to_owned(),
            "build".to_owned(),
            "--incompatible_enable_cc_toolchain_resolution".to_owned(),
            "--keep_going".to_owned(),
            "--noincompatible_use_toolchain_transition".to_owned(),
            "//...".to_owned(),
        ];
        let result = normalize_args(args);
        assert_eq!(result, vec!["kuro", "build", "--keep-going", "//..."]);
    }
}
