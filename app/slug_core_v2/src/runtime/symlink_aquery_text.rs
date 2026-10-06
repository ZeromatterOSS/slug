//! Pinned Bazel 9.2 SymlinkAction/AbstractAction text over retained owners.
//! AbsolutePath is toAbsolutePath, not UnresolvedSymlinkAction. ActionKey is
//! omitted to M9; configuration, labels and paths retain Slug-native domains.

use std::borrow::Cow;
use std::fmt::Write;

use slug_analysis_v2::ConfigurationKind;
use slug_analysis_v2::ConfiguredSymlinkActionView;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::SymlinkTarget;

use super::aquery_text::append_list;
use super::aquery_text::escaped_unicode;
use super::file_write_aquery_text::aquery_label;

pub(super) fn append_symlink(
    output: &mut String,
    view: ConfiguredSymlinkActionView<'_>,
) -> Result<(), &'static str> {
    if cfg!(windows) {
        return Err("Symlink aquery Windows text is not admitted");
    }
    let spec = view.spec();
    let owner = view.owner();
    let configuration = owner
        .configuration()
        .slug_configuration()
        .ok_or("Symlink aquery text rejects legacy configuration")?;
    let target = aquery_label(owner.label());
    let platform = aquery_label(view.execution_platform().label());
    let mnemonic = view.mnemonic();
    let input = match spec.target() {
        SymlinkTarget::Artifact { input, .. } => Some(input),
        SymlinkTarget::AbsolutePath { .. } => None,
    };
    let progress = progress_message(
        spec.progress_message(),
        mnemonic,
        &target,
        spec.output().root_relative_path(),
        input,
    );
    write!(output, "action '{progress}").unwrap();
    // AbstractAction.describe's null-message fallback does not add this suffix.
    if spec.progress_message().is_some() && owner.configuration().kind() == ConfigurationKind::Exec
    {
        output.push_str(" [for tool]");
    }
    writeln!(output, "'\n  Mnemonic: {mnemonic}\n  Target: {target}\n  Configuration: {}\n  Execution platform: {platform}", configuration.projection().display_token()).unwrap();
    if let Some(input) = input {
        let path = input.path();
        append_list(output, "Inputs", &[escaped_unicode(&path)]);
    } else {
        append_list(output, "Inputs", &[] as &[&str]);
    }
    append_list(output, "Outputs", &[escaped_unicode(spec.output().path())]);
    output.push('\n');
    Ok(())
}

pub(super) fn progress_message<'a>(
    message: Option<&'a str>,
    mnemonic: &str,
    target: &str,
    output: &str,
    input: Option<&AnalysisArtifact>,
) -> Cow<'a, str> {
    let Some(message) = message else {
        return Cow::Owned(format!("{mnemonic} {output}"));
    };
    let mut progress = Cow::Borrowed(message);
    // AbstractAction replaces sequentially, including placeholders introduced
    // by an earlier replacement. Do not Unicode-escape the prettyPrint header.
    for (placeholder, replacement) in [("%{label}", target), ("%{output}", output)] {
        if progress.contains(placeholder) {
            progress = Cow::Owned(progress.replace(placeholder, replacement));
        }
    }
    if progress.contains("%{input}") {
        if let Some(input) = input {
            let relative = primary_input_relative(input);
            progress = Cow::Owned(progress.replace("%{input}", &relative));
        }
    }
    progress
}

pub(super) fn primary_input_relative(input: &AnalysisArtifact) -> Cow<'_, str> {
    match input {
        // SourceArtifact.getRootRelativePath strips external/<repo> from the
        // admitted nonsibling execution path. Borrow the canonical fields.
        AnalysisArtifact::Source(label) => {
            let package = label.package().package().as_str();
            let target = label.target().as_str();
            if package.is_empty() {
                Cow::Borrowed(target)
            } else {
                Cow::Owned(format!("{package}/{target}"))
            }
        }
        AnalysisArtifact::Derived { output, .. } => Cow::Borrowed(output.root_relative_path()),
        AnalysisArtifact::WorkspaceStatus(_) => unreachable!("checked Symlink query input kind"),
    }
}

#[cfg(test)]
mod tests {
    use slug_identity_v2::CanonicalLabel;

    use super::*;

    #[test]
    fn progress_uses_original_source_root_and_sequential_substitution() {
        let source = AnalysisArtifact::Source(CanonicalLabel::parse("@@dep+//pkg:in").unwrap());
        assert_eq!(source.path(), "external/dep+/pkg/in");
        assert_eq!(primary_input_relative(&source), "pkg/in");
        assert_eq!(
            progress_message(
                Some("%{label} %{output} %{input}"),
                "Symlink",
                "//:%{output}",
                "%{input}",
                Some(&source)
            ),
            "//:pkg/in pkg/in pkg/in"
        );
        let root = AnalysisArtifact::Source(CanonicalLabel::parse("@@dep+//:in").unwrap());
        let Cow::Borrowed(relative) = primary_input_relative(&root) else {
            panic!("root target must borrow")
        };
        let AnalysisArtifact::Source(label) = &root else {
            unreachable!()
        };
        assert!(std::ptr::eq(
            relative.as_ptr(),
            label.target().as_str().as_ptr()
        ));
    }

    #[test]
    fn absent_empty_and_absolute_progress_remain_distinct() {
        assert_eq!(
            progress_message(None, "ExecutableSymlink", "//:x", "out", None),
            "ExecutableSymlink out"
        );
        assert!(matches!(
            progress_message(Some(""), "Symlink", "//:x", "out", None),
            Cow::Borrowed("")
        ));
        assert_eq!(
            progress_message(Some("%{input} %{output}"), "Symlink", "//:x", "out", None),
            "%{input} out"
        );
    }

    fn test_configuration(tool: bool) -> slug_configuration_v2::SlugConfiguration {
        use std::sync::Arc;

        use slug_configuration_v2::SlugConfiguration;
        use slug_configuration_v2::native::host::AutoCpuToken;
        use slug_configuration_v2::native::host::HostConversionInputs;
        use slug_configuration_v2::native::host::HostPathFlavor;
        let host = HostConversionInputs::new(
            Some(AutoCpuToken::K8),
            Some(HostPathFlavor::Unix),
            None,
            Arc::from([]),
            Arc::from([]),
        )
        .unwrap();
        if tool {
            SlugConfiguration::default_exec(&host).unwrap()
        } else {
            SlugConfiguration::default_target(&host).unwrap()
        }
    }

    fn test_root(tool: bool) -> slug_build_api_v2::DerivedArtifactRoot {
        slug_build_api_v2::DerivedArtifactRoot::bin(test_configuration(tool)).unwrap()
    }

    // Synthetic configured metadata controls only: no absolute-link execution.
    fn configured(
        specs: Vec<slug_build_api_v2::ActionSpec>,
        tool: bool,
        resolved: bool,
    ) -> slug_analysis_v2::ConfiguredNodeResult {
        use std::sync::Arc;

        use slug_analysis_v2::*;
        use slug_build_api_v2::*;
        let target = test_configuration(false);
        let exec = test_configuration(true);
        let owner = ConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//:owner").unwrap(),
            ConfigurationKey::from_slug(if tool { exec.clone() } else { target }),
        );
        let context = if resolved {
            ConfiguredActionOwnerContext::new(
                owner.clone(),
                ConfiguredExecGroup::Default,
                ConfiguredTargetKey::new(
                    CanonicalLabel::parse("@@//:platform").unwrap(),
                    ConfigurationKey::from_slug(exec),
                ),
                PlatformSemanticFact {
                    exec_properties: Arc::from([]),
                    missing_toolchain_error: None,
                },
                &Default::default(),
                &Default::default(),
                Vec::new(),
                None,
                ConfiguredActionAspectProvenance::Absent,
            )
            .unwrap()
        } else {
            ConfiguredActionOwnerContext::unresolved_default(owner.clone()).unwrap()
        };
        ConfiguredNodeResult::new_rule(
            owner,
            ProviderCollection::from_values(Vec::new(), false).unwrap(),
            None,
            RunfilesPackageDepset::empty(),
        )
        .with_action_specs(specs, vec![Arc::new(context)])
        .unwrap()
    }

    fn absolute(path: &str, message: Option<&str>, tool: bool) -> slug_build_api_v2::ActionSpec {
        use slug_build_api_v2::*;
        use slug_configuration_v2::NormalizedAbsoluteBazelPath;
        use slug_configuration_v2::native::host::HostPathFlavor;
        let root = test_root(tool);
        ActionSpec::symlink(SymlinkSpec::new(
            ActionOutput::new(root, "absolute", ActionOutputKind::File),
            SymlinkTarget::AbsolutePath {
                target: NormalizedAbsoluteBazelPath::new(HostPathFlavor::Unix, path).unwrap(),
            },
            message,
        ))
    }

    #[test]
    fn absolute_targets_and_none_tool_headers_preserve_retained_owners() {
        let a = configured(
            vec![
                absolute("/a", Some("%{label} %{output} %{input}"), true),
                absolute("/a", None, true),
                absolute("/a", Some(""), true),
            ],
            true,
            true,
        );
        let render = |node: &slug_analysis_v2::ConfiguredNodeResult| {
            let mut text = String::new();
            for action in node.actions() {
                append_symlink(
                    &mut text,
                    ConfiguredSymlinkActionView::from_configured_action(action).unwrap(),
                )
                .unwrap();
            }
            text
        };
        let text = render(&a);
        assert!(text.starts_with("action '//:owner absolute %{input} [for tool]'\n"));
        assert!(text.contains("action 'Symlink absolute'\n"));
        assert!(text.contains("action ' [for tool]'\n"));
        assert_eq!(text.matches("  Inputs: []\n").count(), 3);
        for absent in [
            "UnresolvedSymlinkTarget",
            "Command Line:",
            "Environment:",
            "ExecutionInfo:",
            "IsExecutable:",
            "ActionKey:",
            "SlugActionToken:",
        ] {
            assert!(!text.contains(absent));
        }
        let b = configured(
            vec![
                absolute("/b", Some("%{label} %{output} %{input}"), true),
                absolute("/b", None, true),
                absolute("/b", Some(""), true),
            ],
            true,
            true,
        );
        assert_ne!(a.actions()[0], b.actions()[0]);
        assert_eq!(text, render(&b));
        let restored = configured(
            vec![
                absolute("/a", Some("%{label} %{output} %{input}"), true),
                absolute("/a", None, true),
                absolute("/a", Some(""), true),
            ],
            true,
            true,
        );
        assert_eq!(a.actions()[0], restored.actions()[0]);
        drop(b);
        drop(restored);
        drop(a);
        assert!(text.ends_with("\n\n"));
    }

    #[test]
    fn checked_symlink_view_rejects_unadmitted_kinds_and_source_policy() {
        use slug_build_api_v2::*;
        let root = test_root(false);
        let source = AnalysisArtifact::Source(CanonicalLabel::parse("@@//:in").unwrap());
        let check = |out: ActionOutput, input: AnalysisArtifact, policy: bool| {
            let node = configured(
                vec![ActionSpec::symlink(SymlinkSpec::new(
                    out,
                    SymlinkTarget::Artifact {
                        input,
                        require_executable: false,
                        use_exec_root_for_source: policy,
                    },
                    None::<&str>,
                ))],
                false,
                true,
            );
            ConfiguredSymlinkActionView::from_configured_action(&node.actions()[0]).unwrap_err()
        };
        let output = || ActionOutput::new(root.clone(), "out", ActionOutputKind::File);
        for kind in [
            ActionOutputKind::Directory,
            ActionOutputKind::Symlink,
            ActionOutputKind::RunfilesTree,
        ] {
            assert_eq!(
                check(
                    ActionOutput::new(root.clone(), "out", kind),
                    source.clone(),
                    false
                ),
                "configured Symlink action requires one regular File output"
            );
            let owner = configured(vec![absolute("/a", None, false)], false, true)
                .configured_target_key()
                .unwrap()
                .artifact_owner();
            assert_eq!(
                check(
                    output(),
                    AnalysisArtifact::Derived {
                        owner,
                        output: ActionOutput::new(root.clone(), "in", kind)
                    },
                    false
                ),
                "configured Symlink query requires a Source or Derived regular File target"
            );
        }
        assert_eq!(
            check(
                ActionOutput::new_constant(root.clone(), "out"),
                source.clone(),
                false
            ),
            "configured Symlink action does not support constant output metadata"
        );
        assert_eq!(
            check(output(), source, true),
            "configured Symlink query does not support alternate source-root policy"
        );
        let statuses = WorkspaceStatusArtifacts::new(
            slug_workspace_v2::NormalizedAbsolutePath::new("/workspace").unwrap(),
        );
        assert_eq!(
            check(
                output(),
                AnalysisArtifact::WorkspaceStatus(statuses.stable().clone()),
                false
            ),
            "configured Symlink query requires a Source or Derived regular File target"
        );
        let unresolved = configured(vec![absolute("/a", None, false)], false, false);
        assert_eq!(
            ConfiguredSymlinkActionView::from_configured_action(&unresolved.actions()[0])
                .unwrap_err(),
            "configured Symlink action requires a selected execution platform"
        );
    }
}
