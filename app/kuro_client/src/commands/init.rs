/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::io::ErrorKind;
use std::io::Write;

use kuro_client_ctx::client_ctx::ClientCommandContext;
use kuro_client_ctx::common::BuckArgMatches;
use kuro_client_ctx::common::ui::CommonConsoleOptions;
use kuro_client_ctx::exit_result::ExitResult;
use kuro_client_ctx::final_console::FinalConsole;
use kuro_client_ctx::path_arg::PathArg;
use kuro_common::argv::Argv;
use kuro_common::argv::SanitizedArgv;
use kuro_error::BuckErrorContext;
use kuro_error::ErrorTag;
use kuro_error::kuro_error;
use kuro_fs::fs_util;
use kuro_fs::paths::abs_path::AbsPath;
use kuro_util::process::background_command;

/// Initializes a kuro project at the provided path.
#[derive(Debug, clap::Parser)]
#[clap(name = "init", about = "Initialize a kuro project")]
pub struct InitCommand {
    /// The path to initialize the project in. The folder does not need to exist.
    #[clap(default_value = ".")]
    path: PathArg,

    /// Don't include the standard prelude or generate toolchain definitions.
    #[clap(long)]
    no_prelude: bool,

    /// Initialize the project even if the git repo at \[PATH\] has uncommitted changes.
    #[clap(long)]
    allow_dirty: bool,

    /// Also initialize a git repository at the given path, and set up an appropriate `.gitignore`
    /// file.
    #[clap(long)]
    git: bool,

    #[clap(flatten)]
    console_opts: CommonConsoleOptions,
}

impl InitCommand {
    pub fn exec(self, _matches: BuckArgMatches<'_>, ctx: ClientCommandContext<'_>) -> ExitResult {
        let console = self.console_opts.final_console();

        match exec_impl(self, ctx, &console) {
            Ok(_) => ExitResult::success(),
            Err(e) => {
                // include the backtrace with the error output
                // (same behaviour as returning the Error from main)
                kuro_error!(ErrorTag::Tier0, "{:?}", e).into()
            }
        }
    }

    pub fn sanitize_argv(&self, argv: Argv) -> SanitizedArgv {
        argv.no_need_to_sanitize()
    }
}

fn exec_impl(
    cmd: InitCommand,
    ctx: ClientCommandContext<'_>,
    console: &FinalConsole,
) -> kuro_error::Result<()> {
    let path = cmd.path.resolve(&ctx.working_dir);
    fs_util::create_dir_all(&path)?;
    let absolute = fs_util::canonicalize(&path)?;
    let git = cmd.git;

    if absolute.is_file() {
        return Err(kuro_error!(
            kuro_error::ErrorTag::Input,
            "Target path {} cannot be an existing file",
            absolute.display()
        ));
    }

    if git {
        let status = match background_command("git")
            .args(["status", "--porcelain"])
            .current_dir(&absolute)
            .output()
        {
            Err(e) if e.kind().eq(&ErrorKind::NotFound) => {
                console.print_error(
                    "Warning: no git found on path, can't check for dirty repo. Proceeding anyway.",
                )?;
                None
            }
            r => Some(r.buck_error_context("Couldn't detect dirty status of folder.")?),
        };

        let changes = status.filter(|o| o.status.success()).map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .lines()
                .any(|l| !l.starts_with("??"))
        });

        if let (Some(true), false) = (changes, cmd.allow_dirty) {
            return Err(kuro_error!(
                kuro_error::ErrorTag::Input,
                "Refusing to initialize in a dirty repo. Stash your changes or use `--allow-dirty` to override."
            ));
        }
    }

    set_up_project(&absolute, git, !cmd.no_prelude)
}

fn initialize_module_bazel(repo_root: &AbsPath) -> kuro_error::Result<()> {
    let mut module = std::fs::File::create(repo_root.join("MODULE.bazel"))?;
    writeln!(module, "module(name = \"root\")")?;
    Ok(())
}

fn initialize_bazelignore(repo_root: &AbsPath, git: bool) -> kuro_error::Result<()> {
    if !git {
        return Ok(());
    }
    let mut bazelignore = std::fs::File::create(repo_root.join(".bazelignore"))?;
    writeln!(bazelignore, ".git")?;
    Ok(())
}

fn initialize_root_build(repo_root: &AbsPath, prelude: bool) -> kuro_error::Result<()> {
    let mut build = std::fs::File::create(repo_root.join("BUILD.bazel"))?;

    if prelude {
        writeln!(build, "# Kuro build file - compatible with Bazel syntax")?;
        writeln!(build, "# See: https://bazel.build/concepts/build-files")?;
        writeln!(build)?;
        writeln!(build, "genrule(")?;
        writeln!(build, "    name = \"hello_world\",")?;
        writeln!(build, "    outs = [\"out.txt\"],")?;
        writeln!(build, "    cmd = \"echo 'Built by Kuro!' > $@\",")?;
        writeln!(build, ")")?;
    }
    Ok(())
}

fn set_up_gitignore(repo_root: &AbsPath) -> kuro_error::Result<()> {
    let gitignore = repo_root.join(".gitignore");
    // If .gitignore is empty or doesn't exist, add build output dirs
    if !gitignore.exists() || fs_util::metadata(&gitignore)?.len() == 0 {
        fs_util::write(
            gitignore,
            "/buck-out\n/bazel-external\n/bazel-bin\n/bazel-testlogs\n",
        )?;
    }
    Ok(())
}

fn set_up_buckroot(repo_root: &AbsPath) -> kuro_error::Result<()> {
    fs_util::write(repo_root.join(".buckroot"), "")?;
    Ok(())
}

fn set_up_project(repo_root: &AbsPath, git: bool, prelude: bool) -> kuro_error::Result<()> {
    set_up_buckroot(repo_root)?;

    if git {
        if !background_command("git")
            .arg("init")
            .current_dir(repo_root)
            .status()?
            .success()
        {
            return Err(kuro_error!(
                kuro_error::ErrorTag::Tier0,
                "Failure when running `git init`."
            ));
        };
        set_up_gitignore(repo_root)?;
    }

    // If MODULE.bazel already exists, leave the project alone — the user has
    // already initialized it (manually or via a previous `kuro init`).
    if repo_root.join("MODULE.bazel").exists() {
        kuro_client_ctx::println!(
            "MODULE.bazel already exists, not overwriting and not generating toolchains"
        )?;
        return Ok(());
    }

    initialize_module_bazel(repo_root)?;
    initialize_bazelignore(repo_root, git)?;
    // Create BUILD.bazel if neither BUILD.bazel nor BUILD exists.
    if !repo_root.join("BUILD.bazel").exists() && !repo_root.join("BUILD").exists() {
        initialize_root_build(repo_root, prelude)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use kuro_fs::fs_util;
    use kuro_fs::paths::abs_path::AbsPath;

    use crate::commands::init::initialize_bazelignore;
    use crate::commands::init::initialize_root_build;
    use crate::commands::init::set_up_gitignore;
    use crate::commands::init::set_up_project;

    #[test]
    fn test_set_up_project_with_prelude_no_git() -> kuro_error::Result<()> {
        let tempdir = tempfile::tempdir()?;
        let tempdir_path = tempdir.path();
        let tempdir_path = AbsPath::new(tempdir_path)?;
        fs_util::create_dir_all(tempdir_path)?;

        set_up_project(tempdir_path, false, true)?;
        assert!(!tempdir_path.join(".buckconfig").exists());
        assert!(tempdir_path.join("MODULE.bazel").exists());
        assert!(tempdir_path.join("BUILD.bazel").exists());
        Ok(())
    }

    #[test]
    fn test_default_gitignore() -> kuro_error::Result<()> {
        let tempdir = tempfile::tempdir()?;
        let tempdir_path = tempdir.path();
        let tempdir_path = AbsPath::new(tempdir_path)?;
        fs_util::create_dir_all(tempdir_path)?;

        // .gitignore does not exist yet
        set_up_gitignore(tempdir_path)?;
        let gitignore_path = tempdir_path.join(".gitignore");
        assert!(gitignore_path.exists());
        let actual = fs_util::read_to_string(&gitignore_path)?;
        let expected = "/buck-out\n/bazel-external\n/bazel-bin\n/bazel-testlogs\n";
        assert_eq!(actual, expected);

        // If an empty .gitignore exists (this is the case we would hit after running `git init`),
        // add the build-output dirs.
        fs_util::write(&gitignore_path, "")?;
        set_up_gitignore(tempdir_path)?;
        assert!(gitignore_path.exists());
        let actual = fs_util::read_to_string(&gitignore_path)?;
        assert_eq!(actual, expected);

        // If a non-empty .gitignore exists, don't touch it
        fs_util::write(&gitignore_path, "foo\nbar\n")?;
        set_up_gitignore(tempdir_path)?;
        assert!(gitignore_path.exists());
        let actual = fs_util::read_to_string(&gitignore_path)?;
        let expected = "foo\nbar\n";
        assert_eq!(actual, expected);
        Ok(())
    }

    #[test]
    fn test_bazelignore_generated_with_git() -> kuro_error::Result<()> {
        let tempdir = tempfile::tempdir()?;
        let tempdir_path = tempdir.path();
        let tempdir_path = AbsPath::new(tempdir_path)?;
        fs_util::create_dir_all(tempdir_path)?;

        initialize_bazelignore(tempdir_path, true)?;
        let actual = fs_util::read_to_string(tempdir_path.join(".bazelignore"))?;
        assert_eq!(actual, ".git\n");
        Ok(())
    }

    #[test]
    fn test_bazelignore_skipped_without_git() -> kuro_error::Result<()> {
        let tempdir = tempfile::tempdir()?;
        let tempdir_path = tempdir.path();
        let tempdir_path = AbsPath::new(tempdir_path)?;
        fs_util::create_dir_all(tempdir_path)?;

        initialize_bazelignore(tempdir_path, false)?;
        assert!(!tempdir_path.join(".bazelignore").exists());
        Ok(())
    }

    #[test]
    fn test_buildfile_generation_with_prelude() -> kuro_error::Result<()> {
        let tempdir = tempfile::tempdir()?;
        let tempdir_path = tempdir.path();
        let tempdir_path = AbsPath::new(tempdir_path)?;
        fs_util::create_dir_all(tempdir_path)?;

        let build_path = tempdir_path.join("BUILD.bazel");
        initialize_root_build(tempdir_path, true)?;
        let actual_build = fs_util::read_to_string(build_path)?;
        let expected_build = "# Kuro build file - compatible with Bazel syntax\n# See: https://bazel.build/concepts/build-files\n\ngenrule(\n    name = \"hello_world\",\n    outs = [\"out.txt\"],\n    cmd = \"echo 'Built by Kuro!' > $@\",\n)\n";
        assert_eq!(actual_build, expected_build);
        Ok(())
    }
}
