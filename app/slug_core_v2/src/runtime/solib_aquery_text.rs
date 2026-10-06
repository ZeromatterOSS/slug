//! Pinned Bazel 9.2 SolibSymlinkAction text over retained generated libraries.
//! ActionKey is deferred to M9; paths, labels and configurations are Slug-native.

use std::fmt::Write;

use slug_analysis_v2::ConfiguredSolibActionView;

use super::aquery_text::append_list;
use super::aquery_text::escaped_unicode;
use super::file_write_aquery_text::aquery_label;

pub(super) fn append_solib(
    output: &mut String,
    view: ConfiguredSolibActionView<'_>,
) -> Result<(), &'static str> {
    if cfg!(windows) {
        return Err("Solib aquery Windows text is not admitted");
    }
    let owner = view.owner();
    let configuration = owner
        .configuration()
        .slug_configuration()
        .ok_or("Solib aquery text rejects legacy configuration")?;
    let target = aquery_label(owner.label());
    let platform = aquery_label(view.execution_platform().label());
    // Solib's raw progress is null. AbstractAction.describe's fallback bypasses
    // both placeholder replacement and the configuration's tool suffix.
    writeln!(output, "action 'SolibSymlink {}'\n  Mnemonic: SolibSymlink\n  Target: {target}\n  Configuration: {}\n  Execution platform: {platform}", view.output().root_relative_path(), configuration.projection().display_token()).unwrap();
    let library = view.library().path();
    append_list(output, "Inputs", &[escaped_unicode(&library)]);
    append_list(output, "Outputs", &[escaped_unicode(view.output().path())]);
    // AbstractAction environment and concrete Solib execution properties are
    // empty, regardless of the retained query owner's platform properties.
    output.push('\n');
    Ok(())
}
