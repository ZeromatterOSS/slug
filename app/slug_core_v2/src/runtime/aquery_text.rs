//! Text projection of retained semantic actions, never an execution plan.
//!
//! Bazel 9.2 ActionGraphTextOutputFormatterCallback, AbstractAction,
//! CommandFailureUtils and ScriptUtil define the admitted Linux text fields.
//! Configuration/path domains remain Slug-native; Spawn ActionKey is deferred
//! to M9 and deliberately omitted. No query-specific retained string pool.

use std::borrow::Cow;
use std::fmt::Write;

use slug_analysis_v2::ConfigurationKind;
use slug_analysis_v2::ConfiguredActionView;
use slug_analysis_v2::ConfiguredBuildInfoActionView;
use slug_analysis_v2::ConfiguredCppCompileActionView;
use slug_analysis_v2::ConfiguredNodeResult;
use slug_analysis_v2::ConfiguredRunfilesSupportActionView;
use slug_analysis_v2::ConfiguredSolibActionView;
use slug_analysis_v2::ConfiguredSpawnActionView;
use slug_analysis_v2::ConfiguredSymlinkActionView;
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ActionOutputKind;
use slug_query_v2::AqueryScope;
use slug_workspace_v2::NormalizedAbsolutePath;
use starlark_map::small_set::SmallSet;

use super::build_info_aquery_text::append_build_info;
use super::cpp_compile_aquery_text::append_cpp_compile;
use super::dice::BuildCommandEvaluation;
use super::dice::ResolvedFileWriteSemanticView;
use super::file_write_aquery_text::aquery_label;
use super::file_write_aquery_text::format_file_write_aquery_text;
use super::file_write_aquery_text::validate_file_write_query_context;
use super::runfiles_support_aquery_text::append_runfiles_support;
use super::solib_aquery_text::append_solib;
use super::spawn_query_inputs::visit_declared_artifacts;
use super::symlink_aquery_text::append_symlink;

/// Only final command text is owned beyond one action's projection scratch.
/// Unsupported families fail the complete command; no partial output escapes.
pub fn format_aquery_text_output_for_scope(
    evaluation: &BuildCommandEvaluation,
    scope: AqueryScope,
) -> Result<String, &'static str> {
    // Preserve the old FileWrite guard-before-render error priority across all
    // selected owners. This pass only constructs borrowed views.
    visit_owners(evaluation, scope, |owner| {
        validate_owner(owner, evaluation.workspace())
    })?;
    let mut output = String::new();
    visit_owners(evaluation, scope, |owner| {
        append_owner(&mut output, owner, evaluation.workspace())
    })?;
    if output.is_empty() {
        return Err("FileWrite aquery text requires at least one resolved action");
    }
    Ok(output)
}

fn visit_owners(
    evaluation: &BuildCommandEvaluation,
    scope: AqueryScope,
    mut visitor: impl FnMut(&ConfiguredNodeResult) -> Result<(), &'static str>,
) -> Result<(), &'static str> {
    match scope {
        AqueryScope::Literal => visitor(evaluation.sole_requested_analysis()?),
        AqueryScope::Deps => {
            for owner in evaluation.analyses() {
                visitor(owner)?;
            }
            Ok(())
        }
    }
}

fn validate_owner(
    owner: &ConfiguredNodeResult,
    workspace: &NormalizedAbsolutePath,
) -> Result<(), &'static str> {
    for action in owner.actions() {
        if matches!(action.kind(), ActionKind::Write { .. }) {
            validate_file_write_query_context(&ConfiguredActionView::from_configured_action(
                action,
            )?)?;
        } else if action.symlink_spec().is_some() {
            ConfiguredSymlinkActionView::from_configured_action(action)?;
        } else if action.runfiles_support_spec().is_some() {
            ConfiguredRunfilesSupportActionView::from_configured_action(action)?;
        } else if action.solib_symlink_spec().is_some() {
            ConfiguredSolibActionView::from_configured_action(action)?;
        } else if action.build_info_transform_spec().is_some() {
            ConfiguredBuildInfoActionView::from_configured_action(action, workspace)?;
        } else if action.cpp_compile_spec().is_some() {
            ConfiguredCppCompileActionView::from_configured_action(action)?;
        } else {
            ConfiguredSpawnActionView::from_configured_action(action)?;
        }
    }
    Ok(())
}

fn append_owner(
    output: &mut String,
    owner: &ConfiguredNodeResult,
    workspace: &NormalizedAbsolutePath,
) -> Result<(), &'static str> {
    for action in owner.actions() {
        if matches!(action.kind(), ActionKind::Write { .. }) {
            let view = ResolvedFileWriteSemanticView::from_configured_action(
                ConfiguredActionView::from_configured_action(action)?,
            );
            output.push_str(&format_file_write_aquery_text(&view)?);
            output.push_str("\n\n");
        } else if action.symlink_spec().is_some() {
            append_symlink(
                output,
                ConfiguredSymlinkActionView::from_configured_action(action)?,
            )?;
        } else if action.runfiles_support_spec().is_some() {
            append_runfiles_support(
                output,
                ConfiguredRunfilesSupportActionView::from_configured_action(action)?,
            )?;
        } else if action.solib_symlink_spec().is_some() {
            append_solib(
                output,
                ConfiguredSolibActionView::from_configured_action(action)?,
            )?;
        } else if action.build_info_transform_spec().is_some() {
            append_build_info(
                output,
                ConfiguredBuildInfoActionView::from_configured_action(action, workspace)?,
            )?;
        } else if action.cpp_compile_spec().is_some() {
            append_cpp_compile(
                output,
                ConfiguredCppCompileActionView::from_configured_action(action)?,
            )?;
        } else {
            append_spawn(
                output,
                ConfiguredSpawnActionView::from_configured_action(action)?,
            )?;
        }
    }
    Ok(())
}

fn append_spawn(
    output: &mut String,
    view: ConfiguredSpawnActionView<'_>,
) -> Result<(), &'static str> {
    if cfg!(windows) {
        return Err("Spawn aquery Windows command text is not admitted");
    }
    let action = view.action();
    let spawn = view.spec();
    let owner = action.context().owner();
    let configuration = owner
        .configuration()
        .slug_configuration()
        .ok_or("Spawn aquery text rejects legacy configuration")?;
    let configuration_token = configuration.projection().display_token();
    let target = aquery_label(owner.label());
    let platform = aquery_label(view.execution_platform().label());

    let mut outputs = Vec::with_capacity(spawn.outputs().len());
    for artifact in spawn.outputs() {
        let path = match artifact.kind() {
            ActionOutputKind::Directory => {
                Cow::Owned(format!("{} (TreeArtifact)", artifact.path()))
            }
            ActionOutputKind::File | ActionOutputKind::Symlink => Cow::Borrowed(artifact.path()),
            ActionOutputKind::RunfilesTree => {
                return Err("Spawn aquery runfiles-tree output is not admitted");
            }
        };
        outputs.push(match path {
            Cow::Borrowed(path) => escaped_unicode(path),
            Cow::Owned(path) => {
                if path.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
                    Cow::Owned(path)
                } else {
                    Cow::Owned(escaped_unicode(&path).into_owned())
                }
            }
        });
    }
    outputs.sort_unstable();

    let primary = &spawn.outputs()[0];
    let mut progress = match spawn.progress_message() {
        Some(message) => Cow::Borrowed(message),
        None => Cow::Owned(format!(
            "{} {}",
            spawn.mnemonic(),
            primary.root_relative_path()
        )),
    };
    // AbstractAction replaces these sequentially, not in a template parser.
    for (placeholder, replacement) in [
        ("%{label}", target.as_str()),
        ("%{output}", primary.root_relative_path()),
    ] {
        if progress.contains(placeholder) {
            progress = Cow::Owned(progress.replace(placeholder, replacement));
        }
    }
    if progress.contains("%{input}") {
        return Err("Spawn aquery progress input requires an original-primary-input producer");
    }
    write!(output, "action '{progress}").unwrap();
    if owner.configuration().kind() == ConfigurationKind::Exec {
        output.push_str(" [for tool]");
    }
    writeln!(output, "'\n  Mnemonic: {}\n  Target: {target}\n  Configuration: {}\n  Execution platform: {platform}", spawn.mnemonic(), configuration_token).unwrap();
    append_inputs(output, spawn)?;
    append_list(output, "Outputs", &outputs);
    drop(outputs);

    append_environment(output, spawn.environment());

    // allArguments query expansion, not execution's spilled argv/virtual files.
    // Drop the complete rendered argv at the end of this action, not command.
    {
        let argv = spawn.render_argv();
        append_command_line(
            output,
            &configuration_token,
            &platform,
            argv.iter().map(|argument| argument.as_str()),
        );
    }
    append_execution_info(output, spawn.execution_requirements().iter());
    output.push('\n');
    Ok(())
}

pub(super) fn append_command_line<'a>(
    output: &mut String,
    configuration: &str,
    platform: &str,
    arguments: impl Iterator<Item = &'a str>,
) {
    output.push_str("  Command Line: (exec ");
    for (index, argument) in arguments.enumerate() {
        if index != 0 {
            output.push_str(" \\\n    ");
        }
        append_shell_escaped(output, &escaped_unicode(argument));
    }
    writeln!(
        output,
        ")\n# Configuration: {configuration}\n# Execution platform: {platform}"
    )
    .unwrap();
}

pub(super) fn append_environment(
    output: &mut String,
    environment: &slug_configuration_v2::RetainedActionEnvironment,
) {
    let mut environment: Vec<_> = environment
        .fixed()
        .iter()
        .map(|(key, value)| {
            let mut field = String::with_capacity(key.len() + 1 + value.len());
            append_unicode(&mut field, key);
            field.push('=');
            append_unicode(&mut field, value);
            field
        })
        .collect();
    environment.sort_unstable();
    if !environment.is_empty() {
        append_list(output, "Environment", &environment);
    }

    drop(environment);
}

pub(super) fn append_execution_info<'a>(
    output: &mut String,
    entries: impl Iterator<Item = (&'a str, &'a str)>,
) {
    let mut requirements: Vec<_> = entries.collect();
    requirements
        .sort_unstable_by(|(left, _), (right, _)| left.encode_utf16().cmp(right.encode_utf16()));
    if !requirements.is_empty() {
        output.push_str("  ExecutionInfo: {");
        for (index, (key, value)) in requirements.iter().enumerate() {
            if index != 0 {
                output.push_str(", ");
            }
            append_shell_escaped(output, key);
            output.push_str(": ");
            append_shell_escaped(output, value);
        }
        output.push_str("}\n");
    }
}

fn append_inputs(
    output: &mut String,
    spawn: &slug_build_api_v2::SpawnSpec,
) -> Result<(), &'static str> {
    let mut inputs = SmallSet::new();
    visit_declared_artifacts(spawn, |artifact| {
        // Preserve artifact identity before display projection. Source keys
        // own labels; copy only distinct keys, and release all after this field.
        // Derived/workspace-status keys share their existing retained storage.
        if !inputs.contains(artifact) {
            inputs.insert(artifact.clone());
        }
    })
    .map_err(|_| "Spawn aquery requires complete declared artifact inputs")?;
    let paths: Vec<_> = inputs.iter().map(|artifact| artifact.path()).collect();
    let mut input_text: Vec<_> = paths.iter().map(|path| escaped_unicode(path)).collect();
    input_text.sort_unstable();
    append_list(output, "Inputs", &input_text);
    Ok(())
}

pub(super) fn append_list(output: &mut String, field: &str, values: &[impl AsRef<str>]) {
    write!(output, "  {field}: [").unwrap();
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        output.push_str(value.as_ref());
    }
    output.push_str("]\n");
}

pub(super) fn escaped_unicode(value: &str) -> Cow<'_, str> {
    if value.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
        return Cow::Borrowed(value);
    }
    let mut output = String::with_capacity(value.len());
    append_unicode(&mut output, value);
    Cow::Owned(output)
}

fn append_unicode(output: &mut String, value: &str) {
    for character in value.chars() {
        if (' '..='~').contains(&character) {
            output.push(character);
        } else if u32::from(character) <= 0xffff {
            write!(output, "\\u{:04X}", u32::from(character)).unwrap();
        } else {
            write!(output, "\\U{:08X}", u32::from(character)).unwrap();
        }
    }
}

fn append_shell_escaped(output: &mut String, value: &str) {
    if !value.is_empty()
        && value.chars().enumerate().all(|(index, character)| {
            character.is_ascii_alphanumeric()
                || "@%-_+:,./".contains(character)
                || (character == '~' && index != 0)
        })
    {
        output.push_str(value);
    } else {
        output.push('\'');
        for character in value.chars() {
            if character == '\'' {
                output.push_str("'\\''");
            } else {
                output.push(character);
            }
        }
        output.push('\'');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_inputs_dedup_identity_before_equal_path_display() {
        use slug_build_api_v2::AnalysisArtifact;
        use slug_build_api_v2::ArtifactInputSource;
        use slug_build_api_v2::ArtifactInputs;
        use slug_build_api_v2::RetainedCommandLine;
        use slug_build_api_v2::RetainedSpawnInvocation;
        use slug_build_api_v2::SpawnSpec;
        use slug_configuration_v2::CanonicalStringMap;
        use slug_configuration_v2::RetainedActionEnvironment;
        use slug_identity_v2::CanonicalLabel;
        let first = AnalysisArtifact::Source(CanonicalLabel::parse("@@//pkg:a/b").unwrap());
        let second = AnalysisArtifact::Source(CanonicalLabel::parse("@@//pkg/a:b").unwrap());
        assert_ne!(first, second);
        assert_eq!(first.path(), second.path());
        let spawn = SpawnSpec::new(
            RetainedSpawnInvocation::Shell {
                shell_executable: "/bin/bash".into(),
                command: "true".into(),
                pad_dollar_zero: false,
            },
            RetainedCommandLine::new(Vec::new()),
            ArtifactInputs::new(vec![
                ArtifactInputSource::Direct(first.clone()),
                ArtifactInputSource::Direct(second),
                ArtifactInputSource::Direct(first),
            ]),
            ArtifactInputs::new(Vec::new()),
            Vec::new(),
            None,
            RetainedActionEnvironment::default(),
            CanonicalStringMap::default(),
            "Spawn",
            None::<String>,
        );
        let mut text = String::new();
        append_inputs(&mut text, &spawn).unwrap();
        assert_eq!(text, "  Inputs: [pkg/a/b, pkg/a/b]\n");
    }

    #[test]
    fn pinned_bazel_unicode_then_shell_bytes() {
        let mut output = String::new();
        for value in [
            "",
            "safe@%_-+:,./",
            "~first",
            "x~later",
            "quote'with space",
            "line\nλ😀\\",
        ] {
            if !output.is_empty() {
                output.push('|');
            }
            append_shell_escaped(&mut output, &escaped_unicode(value));
        }
        assert_eq!(
            output,
            "''|safe@%_-+:,./|'~first'|x~later|'quote'\\''with space'|'line\\u000A\\u03BB\\U0001F600\\'"
        );
        assert!(matches!(escaped_unicode("ordinary[]'\\"), Cow::Borrowed(_)));
    }

    #[test]
    fn fields_sort_after_unicode_escape_and_execution_info_keeps_raw_values() {
        let mut fields = [
            escaped_unicode("z"),
            escaped_unicode("λ"),
            escaped_unicode("a"),
        ];
        fields.sort_unstable();
        let mut output = String::new();
        append_list(&mut output, "Inputs", &fields);
        assert_eq!(output, "  Inputs: [\\u03BB, a, z]\n");
        output.clear();
        append_shell_escaped(&mut output, "λ\n'");
        assert_eq!(output, "'λ\n'\\'''");
    }
}
