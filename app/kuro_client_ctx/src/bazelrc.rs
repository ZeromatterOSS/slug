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

impl BazelRcData {
    /// Extract flags applicable to `command`. Returns flags from:
    /// - `common` lines (no config)
    /// - `<command>` lines (no config)
    /// - `<command>:<config>` lines for each `config` in `active_configs`
    fn flags_for(&self, command: &str, active_configs: &[String]) -> Vec<String> {
        let mut result = Vec::new();
        for (cmd, cfg, flags) in &self.entries {
            let matches_command = cmd == "common" || cmd == command;
            if !matches_command {
                continue;
            }
            match cfg {
                None => result.extend_from_slice(flags),
                Some(config_name) if active_configs.contains(config_name) => {
                    result.extend_from_slice(flags)
                }
                _ => {}
            }
        }
        result
    }
}

/// Parse a `.bazelrc` file at `path`, merging results into `data`.
/// If `required` is false (try-import), silently ignores missing files.
fn parse_bazelrc_file(path: &Path, data: &mut BazelRcData, required: bool) {
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
                data.entries.push((command, config, flags));
            }
            Some(BazelRcLine::Import(import_path)) => {
                let resolved = if import_path.is_absolute() {
                    import_path
                } else {
                    path.parent()
                        .map(|p| p.join(&import_path))
                        .unwrap_or(import_path)
                };
                parse_bazelrc_file(&resolved, data, true);
            }
            Some(BazelRcLine::TryImport(import_path)) => {
                let resolved = if import_path.is_absolute() {
                    import_path
                } else {
                    path.parent()
                        .map(|p| p.join(&import_path))
                        .unwrap_or(import_path)
                };
                parse_bazelrc_file(&resolved, data, false);
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
pub fn normalize_args(args: Vec<String>) -> Vec<String> {
    args.into_iter()
        .filter(|a| !is_bazel_transitional_flag(a))
        .map(|a| normalize_flag(&a))
        .collect()
}

/// Returns true for Bazel-only transitional flags that kuro should silently ignore.
///
/// These include:
/// - `--incompatible_*` / `--noincompatible_*`: Bazel migration flags
/// - `--legacy_*` / `--nolegacy_*`: Legacy behavior toggles
/// - `--experimental_*` / `--noexperimental_*`: Experimental features
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
    base.starts_with("incompatible_")
        || base.starts_with("incompatible-")
        || base.starts_with("legacy_")
        || base.starts_with("legacy-")
        || base.starts_with("experimental_")
        || base.starts_with("experimental-")
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
            parse_bazelrc_file(&user_bazelrc, &mut data, false);
        }
    }

    // 2. Workspace-level: <project_root>/.bazelrc
    if let Some(root) = project_root {
        let workspace_bazelrc = root.join(".bazelrc");
        if workspace_bazelrc.exists() {
            parse_bazelrc_file(&workspace_bazelrc, &mut data, false);
        }
    }

    // Collect active --config= names from the user's command-line args
    let active_configs = find_active_configs(&args);

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
