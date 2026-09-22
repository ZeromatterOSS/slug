//! Native construction snapshots plus the evaluator-end recipe publication seam.
//! No command expansion, discovery, execution, or ambient environment reads.
use slug_build_api_v2::CppCompileConstruction;
use slug_build_api_v2::CppCompileData;
use slug_build_api_v2::CppCompileSpec;
use slug_loading_v2::cc_compile_action::CcCompileRequest;
use slug_loading_v2::cc_compile_action::PendingCppCompile;
use slug_loading_v2::cc_compile_action::PreparedCcCompile;
use slug_loading_v2::cc_context::CcCompileActionContext;

use super::*;

#[cfg(test)]
mod tests;

fn fields<'v>(
    value: Value<'v>,
    expected: &str,
    suffix: &str,
) -> anyhow::Result<Vec<(CompactString, Value<'v>)>> {
    let (key, fields) = slug_loading_v2::provider::starlark_user_provider_fields(value)
        .ok_or_else(|| anyhow::anyhow!("CppCompile requires {expected} provider"))?;
    anyhow::ensure!(
        key.exported_name() == expected
            && [
                format!("@@rules_cc+//{suffix}"),
                format!("@@//{suffix}"),
                format!("@@//third_party/bazel_rules/rules_cc/{suffix}")
            ]
            .iter()
            .any(|label| label == key.source_label()),
        "CppCompile requires the native {expected} provider key"
    );
    Ok(fields)
}
fn field<'v>(fields: &[(CompactString, Value<'v>)], name: &str) -> anyhow::Result<Value<'v>> {
    fields
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| *value)
        .ok_or_else(|| anyhow::anyhow!("CppCompile missing field {name}"))
}
fn regular(value: Value<'_>, name: &str) -> anyhow::Result<AnalysisArtifact> {
    let artifact = AnalysisArtifactValue::from_starlark(value)
        .ok_or_else(|| anyhow::anyhow!("CppCompile {name} requires a File"))?;
    reject_directory_file(artifact, name)?;
    Ok(artifact.artifact().clone())
}
fn depset<'v>(
    value: Value<'v>,
    name: &str,
    lowerer: &mut AnalysisValueLowerer<'v>,
) -> anyhow::Result<AnalysisDepset> {
    let lowered = lowerer.lower(value, name).map_err(anyhow::Error::msg)?;
    let AnalysisValueKind::Depset(set) = lowered.kind() else {
        anyhow::bail!("CppCompile {name} requires a depset of Files");
    };
    let retained = RetainedArtifactInputs::new(set.clone())?;
    validate_input_artifacts(&retained, name, false)?;
    Ok(set.clone())
}
fn sequence(value: Option<Value<'_>>, name: &str) -> anyhow::Result<Vec<AnalysisArtifact>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    if value.get_type() == "range" && value.length().ok() == Some(0) {
        return Ok(Vec::new());
    }
    let values = sequence_values(value)
        .ok_or_else(|| anyhow::anyhow!("CppCompile {name} requires a sequence of Files"))?;
    values
        .into_iter()
        .map(|value| regular(value, name))
        .collect()
}
fn optional_depset<'v>(
    value: Option<Value<'v>>,
    name: &str,
    lowerer: &mut AnalysisValueLowerer<'v>,
) -> anyhow::Result<AnalysisDepset> {
    value
        .filter(|value| !value.is_none())
        .map(|value| depset(value, name, lowerer))
        .transpose()
        .map(|value| value.unwrap_or_else(|| AnalysisDepset::empty(DepsetOrder::Default)))
}
fn inputs(set: AnalysisDepset) -> anyhow::Result<ArtifactInputs> {
    Ok(ArtifactInputs::new(vec![ArtifactInputSource::Depset(
        RetainedArtifactInputs::new(set)?,
    )]))
}

// Builder:276–287 reads this depset (and its empty fallback) even when
// scanning is disabled. Keep validation independent of the selected role.
fn compiler_inputs<'v>(
    toolchain: &[(CompactString, Value<'v>)],
    specific: bool,
    lowerer: &mut AnalysisValueLowerer<'v>,
) -> anyhow::Result<(Value<'v>, AnalysisDepset)> {
    let without = depset(
        field(toolchain, "_compiler_files_without_includes")?,
        "_compiler_files_without_includes",
        lowerer,
    )?;
    if without.is_empty() {
        depset(
            field(toolchain, "_compiler_files")?,
            "_compiler_files",
            lowerer,
        )?;
    }
    let name = if specific {
        "_compiler_files"
    } else {
        "all_files"
    };
    let value = field(toolchain, name)?;
    Ok((value, depset(value, name, lowerer)?))
}

fn should_scan(explicit: Option<bool>, configured: bool, requested: bool) -> bool {
    explicit.unwrap_or(configured && requested)
}
fn union(
    order: DepsetOrder,
    direct: Vec<AnalysisArtifact>,
    transitive: Vec<AnalysisDepset>,
) -> anyhow::Result<AnalysisDepset> {
    Ok(AnalysisDepset::new(
        order,
        direct.into_iter().map(AnalysisValue::artifact).collect(),
        transitive,
    )?)
}

/// Label.parseCanonical: single @ denotes an already-canonical repository;
/// // denotes MAIN, never the caller's repo or its apparent-name mapping.
fn toolchain_label(value: &str) -> anyhow::Result<slug_identity_v2::CanonicalLabel> {
    let canonical = if value.starts_with("@@") {
        value.to_owned()
    } else if value.starts_with('@') {
        format!("@{value}")
    } else if value.starts_with("//") {
        format!("@@{value}")
    } else {
        anyhow::bail!("CppCompile toolchain_type must be an absolute canonical label");
    };
    slug_identity_v2::CanonicalLabel::parse(&canonical).map_err(anyhow::Error::msg)
}

fn selected_group(
    groups: &ConfiguredExecGroupCollection,
    toolchain_type: &str,
) -> anyhow::Result<Option<CompactString>> {
    if !groups.use_auto_exec_groups() {
        return Ok(None);
    }
    let group = ConfiguredExecGroup::automatic(toolchain_label(toolchain_type)?);
    Ok(
        if groups.use_auto_exec_groups() && groups.row(&group).is_some() {
            exec_group_name(&group).map(CompactString::new)
        } else {
            None
        },
    )
}

impl<'v, V: ValueLike<'v>> CcCompileActionContext for AnalysisContextGen<V> {
    fn prepare_cc_compile<'a>(
        &self,
        request: &CcCompileRequest<'a>,
    ) -> anyhow::Result<PreparedCcCompile<'a>> {
        self.token
            .require_active("create_cc_compile_action", "rule context")?;
        anyhow::ensure!(
            request.configuration == self.output_root.configuration(),
            "CppCompile configuration must equal the active ctx output root configuration"
        );
        self.action_sink.prepare_cc_compile(request)
    }
}

impl SynchronousAnalysisActionSink {
    pub(super) fn prepare_native_cc_compile<'v>(
        &self,
        request: &CcCompileRequest<'v>,
    ) -> anyhow::Result<PreparedCcCompile<'v>> {
        use slug_configuration_v2::native::host::ActionEnvironmentHostOs;
        anyhow::ensure!(
            !matches!(
                request.configuration.execution_host_os()?,
                ActionEnvironmentHostOs::Windows | ActionEnvironmentHostOs::Unknown
            ),
            "CppCompile requires an observed supported POSIX Host"
        );
        let features = slug_loading_v2::cc_feature_configuration_value(request.features)
            .ok_or_else(|| anyhow::anyhow!("CppCompile requires native FeatureConfiguration"))?;
        let source = regular(request.source, "source")?;
        let derived_action =
            CppCompileSpec::default_action_name(&source).map_err(anyhow::Error::msg)?;
        let action_name = request.action_name.unwrap_or(derived_action);
        anyhow::ensure!(
            matches!(action_name, "c-compile" | "c++-compile"),
            "CppCompile action_name is unsupported"
        );
        anyhow::ensure!(
            features.action_is_enabled(action_name),
            "Expected action_config for '{action_name}' to be configured"
        );
        for unsupported in [
            "use_header_modules",
            "header_modules",
            "cpp_modules",
            "parse_showincludes",
            "compiler_param_file",
        ] {
            anyhow::ensure!(
                !features.is_enabled(unsupported),
                "CppCompile feature {unsupported} is unsupported"
            );
        }
        let scan = should_scan(
            request.should_scan_includes,
            request.configuration.cpp_compile_include_scanning()?,
            features.is_requested("cc_include_scanning"),
        );
        anyhow::ensure!(!scan, "CppCompile include scanning is unsupported");
        anyhow::ensure!(
            request
                .configuration
                .cpp_compile_execution_modifiers_empty()?,
            "CppCompile modify_execution_info is unsupported"
        );
        anyhow::ensure!(
            slug_loading_v2::provider::starlark_user_provider_has_schema(request.toolchain)
                == Some(true),
            "CppCompile requires a schema-backed CcToolchainInfo"
        );
        let toolchain = fields(
            request.toolchain,
            "CcToolchainInfo",
            "cc/private/rules_impl:cc_toolchain_info.bzl",
        )?;
        let context = fields(
            request.compilation_context,
            "CcCompilationContextInfo",
            "cc/private:cc_info.bzl",
        )?;
        let toolchain_cpp = slug_loading_v2::analysis_fragments::cpp_fragment_value(field(
            &toolchain,
            "_cpp_configuration",
        )?)
        .ok_or_else(|| anyhow::anyhow!("CppCompile toolchain requires a genuine Cpp fragment"))?;
        let mut lowerer = AnalysisValueLowerer::default();
        anyhow::ensure!(
            field(&context, "_module_map")?.is_none(),
            "CppCompile module maps are unsupported"
        );
        for name in [
            "_direct_module_maps",
            "_transitive_modules",
            "_transitive_pic_modules",
            "_module_files",
            "_pic_module_files",
            "_modules_info_files",
            "_pic_modules_info_files",
        ] {
            anyhow::ensure!(
                depset(field(&context, name)?, name, &mut lowerer)?.is_empty(),
                "CppCompile {name} is unsupported"
            );
        }
        let header = lowerer
            .lower(field(&context, "_header_info")?, "CppCompile HeaderInfo")
            .map_err(anyhow::Error::msg)?;
        let AnalysisValueKind::CcHeaderInfo(header) = header.kind() else {
            anyhow::bail!("CppCompile requires native HeaderInfo");
        };
        let h = header.model();
        anyhow::ensure!(
            h.header_module.is_none()
                && h.pic_header_module.is_none()
                && h.separate_module.is_none()
                && h.separate_pic_module.is_none(),
            "CppCompile header modules are unsupported"
        );
        let (tool_inputs_value, tool_inputs) = compiler_inputs(
            &toolchain,
            request
                .configuration
                .cpp_compile_use_specific_tool_files()?,
            &mut lowerer,
        )?;
        let additional = sequence(request.additional_inputs, "additional_compilation_inputs")?;
        let additional_set = optional_depset(
            request.additional_inputs_set,
            "additional_compilation_inputs_set",
            &mut lowerer,
        )?;
        let mandatory = union(
            DepsetOrder::Default,
            additional.clone(),
            vec![additional_set, tool_inputs],
        )?;
        let mut builtin = sequence(
            Some(field(&toolchain, "_builtin_include_files")?),
            "builtin include files",
        )?;
        builtin.extend(sequence(
            request.build_info_headers.filter(|value| !value.is_none()),
            "build_info_header_files",
        )?);
        // Native builtinIncludeFiles is an ImmutableList, not a nested set.
        // Preserve its duplicates here; the mandatory-input union deduplicates.
        let builtin_inputs = ArtifactInputs::new(
            builtin
                .iter()
                .cloned()
                .map(ArtifactInputSource::Direct)
                .collect::<Vec<_>>(),
        );
        let non_code = depset(
            field(&context, "_non_code_inputs")?,
            "non-code inputs",
            &mut lowerer,
        )?;
        let declared = depset(
            field(&context, "headers")?,
            "declared headers",
            &mut lowerer,
        )?;
        let mut direct = builtin;
        direct.push(source.clone());
        let grep = field(&toolchain, "_grep_includes")?;
        if !grep.is_none() {
            direct.push(regular(grep, "grep_includes")?);
        }
        let mandatory_spawn_inputs = inputs(union(
            DepsetOrder::Postorder,
            direct.clone(),
            vec![mandatory, non_code],
        )?)?;
        let cache_key_inputs = inputs(optional_depset(
            request.cache_key_inputs,
            "cache_key_inputs",
            &mut lowerer,
        )?)?;
        let output = self.owned_output(request.output, "create_cc_compile_action")?;
        let dotd = self.owned_output(request.dotd, "create_cc_compile_action")?;
        anyhow::ensure!(
            output.kind() == ActionOutputKind::File
                && dotd.kind() == ActionOutputKind::File
                && output != dotd
                && output.root() == dotd.root(),
            "CppCompile requires distinct regular object and dotd outputs in one root"
        );
        anyhow::ensure!(
            output.root().configuration() == request.configuration,
            "CppCompile outputs have foreign configuration"
        );
        let builtin_include_directories =
            sequence_values(field(&toolchain, "built_in_include_directories")?)
                .ok_or_else(|| {
                    anyhow::anyhow!("CppCompile builtin include directories require a sequence")
                })?
                .into_iter()
                .map(|value| {
                    let value = value.unpack_str().ok_or_else(|| {
                        anyhow::anyhow!("CppCompile builtin include directory requires string")
                    })?;
                    Ok(slug_build_api_v2::normalize_cc_path_posix(value)?)
                })
                .collect::<anyhow::Result<Vec<_>>>()?
                .into_boxed_slice();
        let requirements = features.get_execution_requirements(action_name)?;
        let execution_requirements = CanonicalStringMap::from_pairs(
            self.execution_tags
                .iter()
                .map(|(k, v)| (CompactString::new(k), CompactString::new(v)))
                .chain(
                    requirements
                        .into_iter()
                        .map(|key| (key, CompactString::default())),
                ),
        );
        let exec_group = selected_group(&self.exec_groups, request.toolchain_type)?;
        let selected_tool = features.get_tool_for_action(action_name)?;
        let construction = CppCompileConstruction {
            source,
            outputs: vec![output, dotd].into_boxed_slice(),
            action_name: action_name.into(),
            feature_configuration: features,
            mandatory_spawn_inputs,
            cache_key_inputs,
            declared_headers: inputs(declared)?,
            builtin_inputs,
            additional_prunable_inputs: inputs(AnalysisDepset::empty(DepsetOrder::Default))?,
            environment: self.typed_configuration()?.1.clone(),
            execution_requirements,
            selected_tool,
            toolchain_cpp,
            builtin_include_directories,
            use_pic: request.use_pic,
            needs_include_validation: request.needs_include_validation,
        };
        CppCompileSpec::validate_construction(&construction).map_err(anyhow::Error::msg)?;
        let normal_action_count = self
            .actions
            .lock()
            .map_err(|_| anyhow::anyhow!("ctx.actions state lock is poisoned"))?
            .registry()
            .actions()
            .len();
        Ok(PreparedCcCompile {
            snapshot: PendingCppCompile {
                construction,
                normal_action_count,
                exec_group,
                additional_files: additional.into_boxed_slice(),
                mandatory_direct: direct.into_boxed_slice(),
            },
            input_depsets: [
                request
                    .additional_inputs_set
                    .unwrap_or_else(Value::new_none),
                tool_inputs_value,
                field(&context, "_non_code_inputs")?,
                field(&context, "headers")?,
                request.cache_key_inputs.unwrap_or_else(Value::new_none),
            ],
        })
    }
}

pub(super) fn finalize<'v>(
    module: &'v Module,
    lowerer: &mut AnalysisValueLowerer<'v>,
    actions: &mut CtxActions,
) -> anyhow::Result<()> {
    let pending = slug_loading_v2::cc_compile_action::pending(module);
    if pending.is_empty() {
        return Ok(());
    }
    let mut merged = Vec::with_capacity(actions.registry().actions().len() + pending.len());
    let normal = actions.registry().actions();
    let mut cursor = 0;
    for (index, (prepared, variables, context)) in pending.into_iter().enumerate() {
        let mut pending = prepared.snapshot;
        anyhow::ensure!(
            pending.normal_action_count >= cursor && pending.normal_action_count <= normal.len(),
            "CppCompile pending action order is invalid"
        );
        merged.extend_from_slice(&normal[cursor..pending.normal_action_count]);
        cursor = pending.normal_action_count;
        let variables = lowerer
            .lower(variables, &format!("$actions[{index}].variables"))
            .map_err(anyhow::Error::msg)?;
        let AnalysisValueKind::CcToolchainVariables(variables) = variables.kind() else {
            anyhow::bail!("CppCompile requires native Variables");
        };
        let compilation_context = lowerer
            .lower(context, &format!("$actions[{index}].compilation_context"))
            .map_err(anyhow::Error::msg)?;
        let [additional, tools, non_code, declared, cache] = prepared.input_depsets;
        let mandatory = union(
            DepsetOrder::Default,
            pending.additional_files.to_vec(),
            vec![
                optional_depset(Some(additional), "additional inputs", lowerer)?,
                depset(tools, "tool inputs", lowerer)?,
            ],
        )?;
        pending.construction.mandatory_spawn_inputs = inputs(union(
            DepsetOrder::Postorder,
            pending.mandatory_direct.to_vec(),
            vec![mandatory, depset(non_code, "non-code inputs", lowerer)?],
        )?)?;
        pending.construction.declared_headers =
            inputs(depset(declared, "declared headers", lowerer)?)?;
        pending.construction.cache_key_inputs =
            inputs(optional_depset(Some(cache), "cache-only inputs", lowerer)?)?;
        let spec = CppCompileSpec::new(CppCompileData {
            construction: pending.construction,
            variables: variables.clone(),
            compilation_context,
        })
        .map_err(anyhow::Error::msg)?;
        let mut action = slug_build_api_v2::ActionSpec::cpp_compile(spec);
        if let Some(group) = pending.exec_group {
            action = action.with_exec_group(group.to_string());
        }
        merged.push(action);
    }
    merged.extend_from_slice(&normal[cursor..]);
    actions.replace_actions(merged)?;
    Ok(())
}
