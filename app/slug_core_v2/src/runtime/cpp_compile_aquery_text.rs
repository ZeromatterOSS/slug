//! Pinned pre-discovery CppCompileAction text; no discovery or execution authority.
//! ActionKey is deferred. Configuration, labels and paths retain native domains.

use std::borrow::Cow;
use std::fmt::Write;

use slug_analysis_v2::ConfigurationKind;
use slug_analysis_v2::ConfiguredCppCompileActionView;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::ArtifactInputs;
use starlark_map::small_set::SmallSet;

use super::aquery_text::append_command_line;
use super::aquery_text::append_environment;
use super::aquery_text::append_execution_info;
use super::aquery_text::append_list;
use super::aquery_text::escaped_unicode;
use super::file_write_aquery_text::aquery_label;
use super::symlink_aquery_text::primary_input_relative;
use super::symlink_aquery_text::progress_message;

fn add_inputs(
    inputs: &mut SmallSet<AnalysisArtifact>,
    role: &ArtifactInputs,
) -> Result<(), &'static str> {
    role.visit(|artifact| {
        // Visitor loans cannot escape. Clone only each first full identity;
        // Source labels may copy bytes, Derived handles share their storage.
        if !inputs.contains(artifact) {
            inputs.insert(artifact.clone());
        }
    })
    .map_err(|_| "CppCompile aquery requires retained File input roles")
}

pub(super) fn append_cpp_compile(
    output: &mut String,
    view: ConfiguredCppCompileActionView<'_>,
) -> Result<(), &'static str> {
    if cfg!(windows) {
        return Err("CppCompile aquery Windows command text is not admitted");
    }
    let owner = view.owner();
    let configuration = owner
        .configuration()
        .slug_configuration()
        .ok_or("CppCompile aquery text rejects legacy configuration")?;
    let configuration_token = configuration.projection().display_token();
    let target = aquery_label(owner.label());
    let platform = aquery_label(view.execution_platform().label());
    {
        let mut inputs = SmallSet::new();
        add_inputs(&mut inputs, view.mandatory_spawn_inputs())?;
        // Remember the original position before adding cache/scheduling roles.
        // SmallSet preserves insertion order; do not borrow across its growth.
        let original_primary_index = 0;
        add_inputs(&mut inputs, view.cache_key_inputs())?;
        add_inputs(&mut inputs, view.declared_headers())?;
        let original = inputs
            .iter()
            .nth(original_primary_index)
            .expect("checked nonempty source-present mandatory inputs");
        let source = primary_input_relative(view.source());
        let progress = progress_message(
            Some(&source),
            "CppCompile",
            &target,
            view.object_output().root_relative_path(),
            Some(original),
        );
        // The fixed prefix contains no placeholders; replacing the source
        // portion follows the same sequential AbstractAction substitutions.
        write!(output, "action 'Compiling {progress}").unwrap();
        if owner.configuration().kind() == ConfigurationKind::Exec {
            output.push_str(" [for tool]");
        }
        writeln!(output, "'\n  Mnemonic: CppCompile\n  Target: {target}\n  Configuration: {configuration_token}\n  Execution platform: {platform}").unwrap();
        let mut paths: Vec<_> = inputs
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
        paths.sort_unstable();
        append_list(output, "Inputs", &paths);
        // Paths and identity scratch release before environment/options/info.
    }
    let mut outputs = [
        escaped_unicode(view.object_output().path()),
        escaped_unicode(view.dotd_output().path()),
    ];
    outputs.sort_unstable();
    append_list(output, "Outputs", &outputs);
    drop(outputs);
    append_environment(output, &view.data().environment);
    {
        let options = view
            .spec()
            .expand_command_line_options()
            .map_err(|_| "CppCompile aquery command expansion failed")?;
        append_command_line(
            output,
            &configuration_token,
            &platform,
            std::iter::once(view.data().selected_tool.as_str())
                .chain(options.iter().map(|option| option.as_str())),
        );
    }
    let requirements = &view.data().execution_requirements;
    append_execution_info(
        output,
        view.context()
            .platform_fact()
            .expect("checked effective platform")
            .exec_properties
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .filter(|(key, _)| requirements.get(key).is_none())
            .chain(requirements.iter()),
    );
    output.push('\n');
    Ok(())
}
