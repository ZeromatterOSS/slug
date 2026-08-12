/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_build_api_v2::ActionKind;
use slug_identity_v2::CanonicalLabel;

use super::dice::BuildCommandEvaluation;
use super::dice::ResolvedFileWriteSemanticView;
use super::file_write_identity::FileWriteSemanticIdentity;

pub fn format_file_write_aquery_text(
    view: &ResolvedFileWriteSemanticView<'_>,
) -> Result<String, &'static str> {
    let action = view.action();
    let owner = action.owner();
    let output = action.output().path();
    validate_renderable_action(
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

    Ok(format!(
        "action 'Writing file {output}'\n  Mnemonic: FileWrite\n  Target: {target}\n  Configuration: {configuration}\n  Execution platform: {platform}\n  SlugActionToken: {action_token}\n  Inputs: []\n  Outputs: [bazel-out/{output_configuration}/bin/{output}]\n  IsExecutable: false",
        target = aquery_label(owner.label()),
        configuration = projection.display_token(),
        platform = aquery_label(action.execution_platform().label()),
        action_token = identity.aquery_display_token(),
        output_configuration = projection.path_component(),
    ))
}

pub fn format_file_write_aquery_text_output(
    evaluation: &BuildCommandEvaluation,
) -> Result<String, &'static str> {
    let views = evaluation.resolved_file_write_semantic_views()?;
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

fn aquery_label(label: &CanonicalLabel) -> String {
    if label.package().repo().is_root() {
        format!("//{}:{}", label.package().package(), label.target())
    } else {
        label.to_string()
    }
}

fn validate_renderable_action(
    owner: &CanonicalLabel,
    mnemonic: &str,
    kind: &ActionKind,
    exec_group: Option<&str>,
    output: &str,
) -> Result<(), &'static str> {
    if !owner.package().repo().is_root() {
        return Err("FileWrite aquery text requires a main-repository owner");
    }
    if mnemonic != "FileWrite" {
        return Err("FileWrite aquery text requires the FileWrite mnemonic");
    }
    let ActionKind::Write { is_executable, .. } = kind else {
        return Err("FileWrite aquery text requires a Write action");
    };
    if *is_executable {
        return Err("FileWrite aquery text does not support executable writes");
    }
    if exec_group.is_some() {
        return Err("FileWrite aquery text requires the default exec group");
    }
    if !is_admitted_output_path(output) {
        return Err("FileWrite aquery text requires a normalized relative output");
    }
    Ok(())
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
            Ok(())
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
            Err("FileWrite aquery text does not support executable writes")
        );
        assert_eq!(
            validate_renderable_action(&root, "FileWrite", &ordinary, Some("named"), "write.txt"),
            Err("FileWrite aquery text requires the default exec group")
        );
    }
}
