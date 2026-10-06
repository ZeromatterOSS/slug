//! Pinned Bazel 9.2 support-class text over immutable configured owners.
//! ActionKeys remain deferred; query grants no runfiles execution authority.

use std::fmt::Write;

use slug_analysis_v2::ConfigurationKind;
use slug_analysis_v2::ConfiguredRunfilesSupportActionView;
use slug_build_api_v2::RunfilesSupportActionSpec;

use super::aquery_text::append_environment;
use super::aquery_text::append_execution_info;
use super::aquery_text::append_list;
use super::aquery_text::escaped_unicode;
use super::file_write_aquery_text::aquery_label;
use super::symlink_aquery_text::progress_message;

pub(super) fn append_runfiles_support(
    output: &mut String,
    view: ConfiguredRunfilesSupportActionView<'_>,
) -> Result<(), &'static str> {
    let spec = view.spec();
    let owner = view.owner();
    let configuration = owner
        .configuration()
        .slug_configuration()
        .ok_or("runfiles support aquery text rejects legacy configuration")?;
    let target = aquery_label(owner.label());
    let platform = aquery_label(view.execution_platform().label());
    if matches!(spec, RunfilesSupportActionSpec::RunfilesTree { .. }) {
        // RunfilesTreeAction overrides prettyPrint; no AbstractAction suffix.
        writeln!(output, "runfiles for {target}").unwrap();
    } else {
        let raw = match spec {
            RunfilesSupportActionSpec::RepoMappingManifest { .. } => {
                format!("Writing repo mapping manifest for {target}")
            }
            RunfilesSupportActionSpec::SourceSymlinkManifest { .. } => {
                format!("Creating source manifest for {target}")
            }
            RunfilesSupportActionSpec::SymlinkTree { .. } => {
                let (parent, _) = spec
                    .output()
                    .path()
                    .rsplit_once('/')
                    .ok_or("runfiles support aquery requires an output execution parent")?;
                format!("Creating runfiles tree {parent}")
            }
            RunfilesSupportActionSpec::RunfilesTree { .. } => unreachable!(),
        };
        let primary = match spec {
            RunfilesSupportActionSpec::SymlinkTree { support, .. } => Some(&support.input_manifest),
            _ => None,
        };
        let mut progress = progress_message(
            Some(&raw),
            spec.mnemonic(),
            &target,
            spec.output().root_relative_path(),
            primary,
        );
        if matches!(
            spec,
            RunfilesSupportActionSpec::SourceSymlinkManifest { .. }
        ) && progress.contains("%{input}")
        {
            let mut first = true;
            spec.visit_declared_inputs(|input| {
                if first {
                    // Recompute from raw, preserving one sequential replacement
                    // pass. The returned Cow borrows raw, never this input loan.
                    progress = progress_message(
                        Some(&raw),
                        spec.mnemonic(),
                        &target,
                        spec.output().root_relative_path(),
                        Some(input),
                    );
                    first = false;
                }
            });
        }
        write!(output, "action '{progress}").unwrap();
        if owner.configuration().kind() == ConfigurationKind::Exec {
            output.push_str(" [for tool]");
        }
        output.push_str("'\n");
    }
    writeln!(
        output,
        "  Mnemonic: {}\n  Target: {target}\n  Configuration: {}\n  Execution platform: {platform}",
        spec.mnemonic(),
        configuration.projection().display_token()
    )
    .unwrap();
    {
        let mut inputs = Vec::new();
        spec.visit_declared_inputs(|artifact| {
            // Existing visitor dedups full identity before path projection.
            // Transfer printable Source path storage; callback loans stay local.
            let path = artifact.path();
            inputs.push(if path.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
                path.into_owned()
            } else {
                escaped_unicode(&path).into_owned()
            });
        });
        inputs.sort_unstable();
        append_list(output, "Inputs", &inputs);
    }
    // RunfilesTree is a distinct virtual artifact, not a generic TreeArtifact.
    append_list(output, "Outputs", &[escaped_unicode(spec.output().path())]);
    match spec {
        RunfilesSupportActionSpec::SymlinkTree { environment, .. } => {
            append_environment(output, environment);
            append_execution_info(
                output,
                view.context()
                    .platform_fact()
                    .expect("checked effective platform")
                    .exec_properties
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.as_str())),
            );
        }
        RunfilesSupportActionSpec::RepoMappingManifest { .. }
        | RunfilesSupportActionSpec::SourceSymlinkManifest { .. } => {
            output.push_str("  IsExecutable: false\n");
        }
        RunfilesSupportActionSpec::RunfilesTree { .. } => {}
    }
    output.push('\n');
    Ok(())
}
