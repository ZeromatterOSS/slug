/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use dupe::Dupe;
use fxhash::FxHashMap;
use indexmap::indexset;
use slug_artifact::actions::key::ActionIndex;
use slug_artifact::artifact::artifact_type::Artifact;
use slug_artifact::artifact::artifact_type::testing::BuildArtifactTestingExt;
use slug_artifact::artifact::build_artifact::BuildArtifact;
use slug_artifact::artifact::source_artifact::SourceArtifact;
use slug_build_api::actions::registry::ActionsRegistry;
use slug_build_api::analysis::registry::AnalysisRegistry;
use slug_build_api::artifact_groups::ArtifactGroup;
use slug_build_api::interpreter::rule_defs::artifact::associated::AssociatedArtifacts;
use slug_build_api::interpreter::rule_defs::artifact::output_artifact_like::OutputArtifactArg;
use slug_build_api::interpreter::rule_defs::artifact::starlark_artifact::StarlarkArtifact;
use slug_build_api::interpreter::rule_defs::artifact::starlark_artifact_like::ValueAsInputArtifactLike;
use slug_build_api::interpreter::rule_defs::artifact::starlark_declared_artifact::StarlarkDeclaredArtifact;
use slug_build_api::interpreter::rule_defs::artifact::unpack_artifact::UnpackNonPromiseInputArtifact;
use slug_build_api::interpreter::rule_defs::cmd_args::DefaultCommandLineContext;
use slug_core::category::CategoryRef;
use slug_core::cells::paths::CellRelativePath;
use slug_core::configuration::data::ConfigurationData;
use slug_core::deferred::base_deferred_key::BaseDeferredKey;
use slug_core::deferred::key::DeferredHolderKey;
use slug_core::execution_types::execution::ExecutionPlatformResolution;
use slug_core::execution_types::executor_config::PathSeparatorKind;
use slug_core::fs::artifact_path_resolver::ArtifactFs;
use slug_core::fs::buck_out_path::BuckOutPathKind;
use slug_core::fs::buck_out_path::BuckOutPathResolver;
use slug_core::fs::project::ProjectRoot;
use slug_core::fs::project_rel_path::ProjectRelativePathBuf;
use slug_core::package::PackageLabel;
use slug_core::package::package_relative_path::PackageRelativePath;
use slug_core::package::source_path::SourcePath;
use slug_core::pattern::pattern::ParsedPattern;
use slug_core::pattern::pattern_type::TargetPatternExtra;
use slug_core::target::configured_target_label::ConfiguredTargetLabel;
use slug_core::target::label::label::TargetLabel;
use slug_execute::artifact::fs::ExecutorFs;
use slug_execute::execute::request::OutputType;
use slug_fs::paths::abs_norm_path::AbsNormPathBuf;
use slug_fs::paths::forward_rel_path::ForwardRelativePathBuf;
use slug_interpreter_for_build::interpreter::build_context::BuildContext;
use slug_interpreter_for_build::interpreter::testing::cells;
use slug_util::arc_str::ArcS;
use starlark::environment::GlobalsBuilder;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::Value;
use starlark::values::list_or_tuple::UnpackListOrTuple;

use crate::actions::testings::SimpleUnregisteredAction;

fn get_label(eval: &Evaluator, target: &str) -> slug_error::Result<ConfiguredTargetLabel> {
    let ctx = BuildContext::from_context(eval)?;
    match ParsedPattern::<TargetPatternExtra>::parse_precise(
        target,
        ctx.build_file_cell().name(),
        ctx.cell_resolver(),
        ctx.cell_info.cell_alias_resolver(),
    ) {
        Ok(ParsedPattern::Target(package, target_name, TargetPatternExtra)) => {
            Ok(TargetLabel::new(package, target_name.as_ref())
                .configure(ConfigurationData::testing_new()))
        }
        _ => panic!("expected a valid target"),
    }
}

#[starlark_module]
pub(crate) fn artifactory(builder: &mut GlobalsBuilder) {
    fn source_artifact(
        package: &str,
        path: &str,
        eval: &mut Evaluator,
    ) -> starlark::Result<StarlarkArtifact> {
        let ctx = BuildContext::from_context(eval)?;
        let package = PackageLabel::new(
            ctx.build_file_cell().name(),
            CellRelativePath::from_path(package).unwrap(),
        )?;
        let path = SourcePath::new(package, ArcS::from(PackageRelativePath::new(path)?));
        Ok(StarlarkArtifact::new(SourceArtifact::new(path).into()))
    }

    fn bound_artifact(
        target: &str,
        path: &str,
        eval: &mut Evaluator,
    ) -> starlark::Result<StarlarkArtifact> {
        let target_label = get_label(eval, target)?;
        let id = ActionIndex::new(0);
        let artifact = Artifact::from(BuildArtifact::testing_new(target_label, path, id));
        Ok(StarlarkArtifact::new(artifact))
    }

    fn declared_artifact<'v>(
        path: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkDeclaredArtifact<'v>> {
        let target_label = get_label(eval, "//foo:bar")?;
        let mut registry = ActionsRegistry::new(
            DeferredHolderKey::Base(BaseDeferredKey::TargetLabel(target_label)),
            ExecutionPlatformResolution::unspecified(),
        );
        let artifact = registry.declare_artifact(
            None,
            ForwardRelativePathBuf::try_from(path.to_owned()).unwrap(),
            OutputType::File,
            None,
            BuckOutPathKind::default(),
            eval.heap(),
        )?;
        Ok(StarlarkDeclaredArtifact::new(
            None,
            artifact,
            AssociatedArtifacts::new(),
        ))
    }

    fn declared_bound_artifact<'v>(
        target: &str,
        path: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkDeclaredArtifact<'v>> {
        let target_label = get_label(eval, target)?;
        let mut registry = ActionsRegistry::new(
            DeferredHolderKey::Base(BaseDeferredKey::TargetLabel(target_label.dupe())),
            ExecutionPlatformResolution::unspecified(),
        );
        let artifact = registry.declare_artifact(
            None,
            ForwardRelativePathBuf::try_from(path.to_owned()).unwrap(),
            OutputType::File,
            None,
            BuckOutPathKind::default(),
            eval.heap(),
        )?;
        let outputs = indexset![artifact.as_output()];
        registry.register(
            &DeferredHolderKey::Base(BaseDeferredKey::TargetLabel(target_label.dupe())),
            outputs,
            SimpleUnregisteredAction::new(
                indexset![],
                vec![],
                CategoryRef::new("fake_action").unwrap().to_owned(),
                None,
            ),
            None,
            std::sync::Arc::new(std::collections::BTreeMap::new()),
        )?;
        Ok(StarlarkDeclaredArtifact::new(
            None,
            artifact,
            AssociatedArtifacts::new(),
        ))
    }

    fn stringify_for_cli<'v>(artifact: ValueAsInputArtifactLike<'v>) -> starlark::Result<String> {
        let cell_info = cells(None).unwrap();
        let project_fs =
            ProjectRoot::new(AbsNormPathBuf::try_from(std::env::current_dir().unwrap()).unwrap())
                .unwrap();
        let fs = ArtifactFs::new(
            cell_info.1,
            BuckOutPathResolver::new(ProjectRelativePathBuf::unchecked_new(
                "buck-out/v2".to_owned(),
            )),
            project_fs,
        );
        let executor_fs = ExecutorFs::new(&fs, PathSeparatorKind::Unix);
        let mut cli = Vec::<String>::new();
        let mut ctx = DefaultCommandLineContext::new(&executor_fs);
        artifact
            .0
            .as_command_line_like()
            .add_to_command_line(&mut cli, &mut ctx, &FxHashMap::default())
            .unwrap();
        assert_eq!(1, cli.len());
        Ok(cli.first().unwrap().to_owned())
    }

    // Mainly tests get_or_declare_output function that can transfer associated artifacts
    // artifact parameter can be either string or artifact
    fn declared_bound_artifact_with_associated_artifacts<'v>(
        // TODO(nga): parameters should be either positional or named, not both.
        artifact: OutputArtifactArg<'v>,
        associated_artifacts: UnpackListOrTuple<UnpackNonPromiseInputArtifact<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let target_label = get_label(eval, "//foo:bar")?;
        let mut analysis_registry = AnalysisRegistry::new_from_owner(
            BaseDeferredKey::TargetLabel(target_label.dupe()),
            ExecutionPlatformResolution::unspecified(),
        )?;
        let mut actions_registry = ActionsRegistry::new(
            DeferredHolderKey::Base(BaseDeferredKey::TargetLabel(target_label.dupe())),
            ExecutionPlatformResolution::unspecified(),
        );

        let associated_artifacts = AssociatedArtifacts::from(
            associated_artifacts
                .items
                .iter()
                .map(|a| ArtifactGroup::Artifact(a.artifact().unwrap())),
        );
        let (declaration, output_artifact) =
            analysis_registry.get_or_declare_output(eval, artifact, OutputType::File, None)?;

        actions_registry.register(
            &DeferredHolderKey::Base(BaseDeferredKey::TargetLabel(target_label.dupe())),
            indexset![output_artifact],
            SimpleUnregisteredAction::new(
                indexset![],
                vec![],
                CategoryRef::new("fake_action").unwrap().to_owned(),
                None,
            ),
            None,
            std::sync::Arc::new(std::collections::BTreeMap::new()),
        )?;

        let value = declaration
            .into_declared_artifact(associated_artifacts)
            .to_value();
        Ok(value)
    }

    fn get_associated_artifacts_as_string<'v>(
        artifact: ValueAsInputArtifactLike<'v>,
    ) -> starlark::Result<String> {
        let associated_artifacts = artifact.0.get_associated_artifacts();
        let s: String = associated_artifacts
            .iter()
            .flat_map(|v| v.iter())
            .map(|a| a.to_string())
            .collect();
        Ok(s)
    }
}
