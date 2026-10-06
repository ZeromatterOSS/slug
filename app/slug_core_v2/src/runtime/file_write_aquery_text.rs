/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_analysis_v2::ConfigurationKind;
use slug_analysis_v2::ConfiguredActionAspectProvenance;
use slug_analysis_v2::ConfiguredActionView;
use slug_analysis_v2::ConfiguredExecGroup;
use slug_build_api_v2::ActionKind;
use slug_configuration_v2::HostPathFlavor;
use slug_identity_v2::CanonicalLabel;
use slug_query_v2::AqueryScope;

use super::aquery_text::escaped_unicode;
use super::dice::BuildCommandEvaluation;
use super::dice::ResolvedFileWriteSemanticView;
use super::file_write_identity::FileWriteSemanticIdentity;
use super::symlink_aquery_text::progress_message;

pub fn format_file_write_aquery_text(
    view: &ResolvedFileWriteSemanticView<'_>,
) -> Result<String, &'static str> {
    let action = view.action();
    validate_file_write_query_context(action)?;
    let owner = action.owner();
    let output = action.output().root_relative_path();
    let is_executable = validate_renderable_action(
        owner.label(),
        action.spec().mnemonic(),
        action.spec().kind(),
        action.spec().exec_group(),
        output,
    )?;
    let configuration = owner
        .configuration()
        .slug_configuration()
        .ok_or("FileWrite aquery text rejects legacy configuration")?;
    let projection = configuration.projection();
    let identity = FileWriteSemanticIdentity::from_resolved(view)?;
    let target = aquery_label(owner.label());
    let progress = progress_message(Some(output), "FileWrite", &target, output, None);
    let writing = if is_executable { "script" } else { "file" };
    let tool_suffix = if owner.configuration().kind() == ConfigurationKind::Exec {
        " [for tool]"
    } else {
        ""
    };

    Ok(format!(
        "action 'Writing {writing} {progress}{tool_suffix}'\n  Mnemonic: FileWrite\n  Target: {target}\n  Configuration: {configuration}\n  Execution platform: {platform}\n  SlugActionToken: {action_token}\n  Inputs: []\n  Outputs: [{exec_path}]\n  IsExecutable: {is_executable}",
        configuration = projection.display_token(),
        platform = aquery_label(action.execution_platform().label()),
        action_token = identity.aquery_display_token(),
        exec_path = escaped_unicode(action.output().path()),
    ))
}

pub fn format_file_write_aquery_text_output(
    evaluation: &BuildCommandEvaluation,
) -> Result<String, &'static str> {
    format_file_write_aquery_text_output_for_scope(evaluation, AqueryScope::Literal)
}

pub fn format_file_write_aquery_text_output_for_scope(
    evaluation: &BuildCommandEvaluation,
    scope: AqueryScope,
) -> Result<String, &'static str> {
    let views = match scope {
        AqueryScope::Literal => evaluation.resolved_file_write_semantic_views()?,
        AqueryScope::Deps => evaluation.resolved_file_write_semantic_views_in_closure()?,
    };
    if views.is_empty() {
        return Err("FileWrite aquery text requires at least one resolved action");
    }
    let mut output = String::new();
    for view in &views {
        output.push_str(&format_file_write_aquery_text(view)?);
        output.push_str("\n\n");
    }
    Ok(output)
}

pub(super) fn aquery_label(label: &CanonicalLabel) -> String {
    if label.package().repo().is_root() {
        format!("//{}:{}", label.package().package(), label.target())
    } else {
        label.to_string()
    }
}

// Query-only policy: shared FileWrite semantics and execution keep their own gates.
// Called during all-owner structural validation and by the direct formatter.
pub(super) fn validate_file_write_query_context(
    action: &ConfiguredActionView<'_>,
) -> Result<(), &'static str> {
    if action.context().aspect() != ConfiguredActionAspectProvenance::Absent {
        return Err("configured FileWrite query does not support aspect provenance");
    }
    if action.exec_group() != &ConfiguredExecGroup::Default || action.spec().exec_group().is_some()
    {
        return Err("configured FileWrite query requires the default exec group");
    }
    let configuration = action
        .owner()
        .configuration()
        .slug_configuration()
        .ok_or("FileWrite aquery text rejects legacy configuration")?;
    if configuration
        .configured_action_path_flavor()
        .map_err(|_| "configured FileWrite query requires retained Host path facts")?
        != HostPathFlavor::Unix
    {
        return Err("configured FileWrite query does not support Windows");
    }
    Ok(())
}

fn validate_renderable_action(
    owner: &CanonicalLabel,
    mnemonic: &str,
    kind: &ActionKind,
    exec_group: Option<&str>,
    output: &str,
) -> Result<bool, &'static str> {
    if !owner.package().repo().is_root() {
        return Err("FileWrite aquery text requires a main-repository owner");
    }
    if mnemonic != "FileWrite" {
        return Err("FileWrite aquery text requires the FileWrite mnemonic");
    }
    let ActionKind::Write { is_executable, .. } = kind else {
        return Err("FileWrite aquery text requires a Write action");
    };
    if exec_group.is_some() {
        return Err("FileWrite aquery text requires the default exec group");
    }
    if !is_admitted_output_path(output) {
        return Err("FileWrite aquery text requires a normalized relative output");
    }
    Ok(*is_executable)
}

fn is_admitted_output_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.ends_with('/')
        && !path
            .chars()
            .any(|character| character.is_control() || matches!(character, '\\' | '\'' | '[' | ']'))
        && path
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
}

#[cfg(test)]
mod tests {
    use slug_build_api_v2::ActionKind;
    use slug_identity_v2::CanonicalLabel;

    use super::aquery_label;
    use super::is_admitted_output_path;
    use super::validate_renderable_action;

    #[test]
    fn aquery_labels_use_bazel_root_and_canonical_external_spelling() {
        assert_eq!(
            aquery_label(&CanonicalLabel::parse("@@//pkg:target").unwrap()),
            "//pkg:target"
        );
        assert_eq!(
            aquery_label(&CanonicalLabel::parse("@@platforms//host:host").unwrap()),
            "@@platforms//host:host"
        );
    }

    #[test]
    fn output_path_validation_is_fail_closed_for_unescaped_text() {
        assert!(is_admitted_output_path("pkg/generated.txt"));
        for rejected in [
            "",
            "/absolute",
            "trailing/",
            "double//separator",
            "./dot",
            "pkg/../escape",
            "windows\\path",
            "quoted'name",
            "bracket]name",
            "tab\tname",
            "line\nbreak",
        ] {
            assert!(!is_admitted_output_path(rejected), "{rejected:?}");
        }
    }

    #[test]
    fn formatter_boundary_rejects_unadmitted_action_shapes() {
        let root = CanonicalLabel::parse("@@//:write").unwrap();
        let external = CanonicalLabel::parse("@@repo//:write").unwrap();
        let ordinary = ActionKind::Write {
            content: "content".to_owned(),
            is_executable: false,
        };
        let executable = ActionKind::Write {
            content: "content".to_owned(),
            is_executable: true,
        };
        assert_eq!(
            validate_renderable_action(&root, "FileWrite", &ordinary, None, "write.txt"),
            Ok(false)
        );
        assert_eq!(
            validate_renderable_action(&external, "FileWrite", &ordinary, None, "write.txt"),
            Err("FileWrite aquery text requires a main-repository owner")
        );
        assert_eq!(
            validate_renderable_action(&root, "Other", &ordinary, None, "write.txt"),
            Err("FileWrite aquery text requires the FileWrite mnemonic")
        );
        assert_eq!(
            validate_renderable_action(&root, "FileWrite", &ActionKind::Run, None, "write.txt"),
            Err("FileWrite aquery text requires a Write action")
        );
        assert_eq!(
            validate_renderable_action(&root, "FileWrite", &executable, None, "write.txt"),
            Ok(true)
        );
        assert_eq!(
            validate_renderable_action(&root, "FileWrite", &ordinary, Some("named"), "write.txt"),
            Err("FileWrite aquery text requires the default exec group")
        );
    }
}
