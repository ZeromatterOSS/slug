//! Pinned Bazel 9.2 BuildInfoFileWriteAction metadata, without callback execution.
//! ActionKey is deferred; configuration, labels and paths remain Slug-native.

use std::borrow::Cow;
use std::fmt::Write;

use slug_analysis_v2::ConfigurationKind;
use slug_analysis_v2::ConfiguredBuildInfoActionView;

use super::aquery_text::append_execution_info;
use super::aquery_text::append_list;
use super::aquery_text::escaped_unicode;
use super::file_write_aquery_text::aquery_label;

pub(super) fn append_build_info(
    output: &mut String,
    view: ConfiguredBuildInfoActionView<'_>,
) -> Result<(), &'static str> {
    if cfg!(windows) {
        return Err("TranslateBuildInfo aquery Windows text is not admitted");
    }
    let owner = view.owner();
    let configuration = owner
        .configuration()
        .slug_configuration()
        .ok_or("TranslateBuildInfo aquery text rejects legacy configuration")?;
    let target = aquery_label(owner.label());
    let platform = aquery_label(view.execution_platform().label());
    write!(output, "action '{}", view.spec().progress_message()).unwrap();
    if owner.configuration().kind() == ConfigurationKind::Exec {
        output.push_str(" [for tool]");
    }
    writeln!(output, "'\n  Mnemonic: TranslateBuildInfo\n  Target: {target}\n  Configuration: {}\n  Execution platform: {platform}", configuration.projection().display_token()).unwrap();
    {
        // Typed input identity was deduplicated by the spec. Preserve different
        // artifacts with equal displayed paths; transfer printable Source paths.
        let mut inputs: Vec<_> = view
            .spec()
            .inputs()
            .iter()
            .map(|artifact| {
                let path = artifact.path();
                if path.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
                    path
                } else {
                    Cow::Owned(escaped_unicode(&path).into_owned())
                }
            })
            .collect();
        inputs.sort_unstable();
        append_list(output, "Inputs", &inputs);
    }
    append_list(output, "Outputs", &[escaped_unicode(view.output().path())]);
    append_execution_info(
        output,
        view.context()
            .platform_fact()
            .expect("checked effective platform")
            .exec_properties
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str())),
    );
    // Execute-time template delegation does not expose FileWrite/template fields.
    output.push('\n');
    Ok(())
}
