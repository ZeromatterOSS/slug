/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;
use dupe::OptionDupedExt;
use gazebo::prelude::SliceExt;
use indexmap::IndexSet;
use slug_artifact::actions::key::ActionIndex;
use slug_artifact::actions::key::ActionKey;
use slug_artifact::artifact::artifact_type::DeclaredArtifact;
use slug_artifact::artifact::artifact_type::OutputArtifact;
use slug_artifact::artifact::build_artifact::BuildArtifact;
use slug_core::category::Category;
use slug_core::deferred::key::DeferredHolderKey;
use slug_core::execution_types::execution::ExecutionPlatformResolution;
use slug_core::execution_types::executor_config::CommandExecutorConfig;
use slug_core::execution_types::executor_config::Executor;
use slug_core::execution_types::executor_config::RemoteEnabledExecutorOptions;
use slug_core::fs::buck_out_path::BuckOutPathKind;
use slug_core::fs::buck_out_path::BuildArtifactPath;
use slug_directory::directory;
use slug_directory::directory::builder::DirectoryBuilder;
use slug_directory::directory::builder::DirectoryInsertError;
use slug_directory::directory::directory::Directory;
use slug_directory::directory::directory_hasher::NoDigest;
use slug_directory::directory::directory_iterator::DirectoryIterator;
use slug_directory::directory::entry::DirectoryEntry;
use slug_error::BuckErrorContext;
use slug_error::internal_error;
use slug_execute::execute::request::OutputType;
use slug_fs::paths::forward_rel_path::ForwardRelativePath;
use slug_fs::paths::forward_rel_path::ForwardRelativePathBuf;
use starlark::codemap::FileSpan;
use starlark::collections::SmallMap;
use starlark::collections::SmallSet;
use starlark::values::Heap;
use starlark::values::Trace;

use crate::actions::ActionErrors;
use crate::actions::ActionToBeRegistered;
use crate::actions::RegisteredAction;
use crate::actions::UnregisteredAction;
use crate::analysis::registry::AnalysisValueFetcher;
use crate::deferred::calculation::ActionLookup;

/// The actions registry for a particular analysis of a rule, dynamic actions, anon target, BXL.
#[derive(Allocative, Trace)]
pub struct ActionsRegistry<'v> {
    owner: DeferredHolderKey,
    artifacts: SmallSet<DeclaredArtifact<'v>>,

    // For a dynamic_output, maps the ActionKeys for the outputs that have been bound
    // to this dynamic_output to the DeclaredArtifact created in the dynamic_output.
    declared_dynamic_outputs: SmallMap<ActionKey, DeclaredArtifact<'v>>,
    pending: Vec<ActionToBeRegistered>,
    pub execution_platform: ExecutionPlatformResolution,
    /// Plan 24 Phase 2: per-target `exec_properties = {…}` attribute,
    /// merged onto each action's `RePlatformFields` at registration time
    /// so the resulting RE Platform message has the right keys for
    /// per-target overrides (e.g. `container-image`). The platform's
    /// `re_properties` are layered first; this dict overrides them.
    target_exec_properties: Arc<std::collections::BTreeMap<String, String>>,
    /// Plan 24 Phase 4: names of exec groups declared by this rule via
    /// `rule(exec_groups={...})`. `actions.run(exec_group="<name>")`
    /// validates against this list — naming a group not in the list
    /// errors loudly with the valid names, matching Bazel's behavior.
    /// Empty for rules that didn't declare any exec groups.
    valid_exec_group_names: Arc<[String]>,
    /// Plan 24 Phase 8: per-named-exec-group resolved
    /// `ExecutionPlatformResolution`. When `actions.run(exec_group="<name>")`
    /// commits, the action's RE Platform message base comes from this
    /// map's entry for that name instead of `execution_platform` (the
    /// default group's). Default-group actions (no `exec_group=`) keep
    /// using `execution_platform`. Missing entries fall back to the
    /// default — this happens when the registered candidate list is
    /// empty (no `register_execution_platforms()` and no
    /// `--extra_execution_platforms`), in which case every group
    /// transparently shares the host fallback. The Phase 9 per-action
    /// kwarg merges on top of whichever base was selected here.
    group_platforms: Arc<HashMap<String, ExecutionPlatformResolution>>,
    claimed_output_paths: DirectoryBuilder<Option<FileSpan>, NoDigest>,
    /// Bazel-compat: map from path string → artifact so duplicate declare_file calls return same artifact.
    path_to_artifact: SmallMap<String, DeclaredArtifact<'v>>,
}

impl<'v> ActionsRegistry<'v> {
    pub fn new(owner: DeferredHolderKey, execution_platform: ExecutionPlatformResolution) -> Self {
        Self::new_with_attrs(
            owner,
            execution_platform,
            Arc::new(std::collections::BTreeMap::new()),
            Arc::from(Vec::<String>::new()),
            Arc::new(HashMap::new()),
        )
    }

    pub fn new_with_target_exec_properties(
        owner: DeferredHolderKey,
        execution_platform: ExecutionPlatformResolution,
        target_exec_properties: Arc<std::collections::BTreeMap<String, String>>,
    ) -> Self {
        Self::new_with_attrs(
            owner,
            execution_platform,
            target_exec_properties,
            Arc::from(Vec::<String>::new()),
            Arc::new(HashMap::new()),
        )
    }

    pub fn new_with_attrs(
        owner: DeferredHolderKey,
        execution_platform: ExecutionPlatformResolution,
        target_exec_properties: Arc<std::collections::BTreeMap<String, String>>,
        valid_exec_group_names: Arc<[String]>,
        group_platforms: Arc<HashMap<String, ExecutionPlatformResolution>>,
    ) -> Self {
        Self {
            owner,
            artifacts: Default::default(),
            declared_dynamic_outputs: SmallMap::new(),
            pending: Default::default(),
            execution_platform,
            target_exec_properties,
            valid_exec_group_names,
            group_platforms,
            claimed_output_paths: DirectoryBuilder::empty(),
            path_to_artifact: SmallMap::new(),
        }
    }

    /// Plan 24 Phase 4: returns the rule's declared exec_group names so
    /// `actions.run(exec_group=…)` can validate the user-supplied value
    /// against the list and surface a clear error with the valid names.
    pub fn valid_exec_group_names(&self) -> &[String] {
        &self.valid_exec_group_names
    }

    pub fn owner(&self) -> &DeferredHolderKey {
        &self.owner
    }

    pub fn declare_dynamic_output(
        &mut self,
        artifact: &BuildArtifact,
        heap: Heap<'v>,
    ) -> slug_error::Result<DeclaredArtifact<'v>> {
        if !self.pending.is_empty() {
            return Err(internal_error!(
                "output for dynamic_output/actions declared after actions: {}, {:?}",
                artifact,
                self.pending.map(|v| v.key())
            ));
        }

        // We don't want to claim path, because the output belongs to different (outer) context.

        // We also don't care to keep track of the hidden components count since this output will
        // never escape the dynamic lambda.
        // TODO(cjhopman): dynamic values mean this can escape. does this need to be updated for that?
        let new_artifact =
            DeclaredArtifact::new(artifact.get_path().dupe(), artifact.output_type(), 0, heap);

        assert!(
            self.declared_dynamic_outputs
                .insert(artifact.key().dupe(), new_artifact.dupe())
                .is_none()
        );

        Ok(new_artifact)
    }

    pub fn claim_output_path(
        &mut self,
        path: &ForwardRelativePath,
        declaration_location: Option<FileSpan>,
    ) -> slug_error::Result<()> {
        fn display_location_opt(location: Option<&FileSpan>) -> &dyn std::fmt::Display {
            location.map_or(&"<unknown>" as _, |l| l as _)
        }

        match self
            .claimed_output_paths
            .insert(path, DirectoryEntry::Leaf(declaration_location))
        {
            Ok(None) => Ok(()),
            Ok(Some(conflict)) => match conflict {
                DirectoryEntry::Leaf(location) => Err(ActionErrors::ConflictingOutputPath(
                    path.to_owned(),
                    display_location_opt(location.as_ref()).to_string(),
                )
                .into()),
                DirectoryEntry::Dir(conflict_dir) => {
                    let conflicting_paths = conflict_dir
                        .ordered_walk_leaves()
                        .with_paths()
                        .map(|(p, location)| {
                            format!(
                                "{} declared at {}",
                                path.join(p),
                                display_location_opt(location.as_ref()),
                            )
                        })
                        .collect::<Vec<_>>();
                    Err(
                        ActionErrors::ConflictingOutputPaths(path.to_owned(), conflicting_paths)
                            .into(),
                    )
                }
            },
            Err(DirectoryInsertError::EmptyPath) => Err(ActionErrors::EmptyOutputPath.into()),
            Err(DirectoryInsertError::CannotTraverseLeaf { path: conflict }) => {
                let location =
                    match directory::find::find(self.claimed_output_paths.as_ref(), &conflict) {
                        Ok(Some(DirectoryEntry::Leaf(l))) => l.as_ref(),
                        _ => None,
                    };

                let conflict = format!(
                    "{} declared at {}",
                    conflict,
                    display_location_opt(location),
                );

                Err(ActionErrors::ConflictingOutputPaths(path.to_owned(), vec![conflict]).into())
            }
        }
    }

    /// Declares a new output file that will be generated by some action.
    ///
    /// Bazel-compat: If an artifact was already declared at this path (e.g., via `ctx.outputs`),
    /// returns the existing artifact instead of erroring. This matches Bazel's behavior where
    /// `ctx.actions.declare_file(ctx.outputs.foo.basename)` re-uses the predeclared artifact.
    pub fn declare_artifact(
        &mut self,
        prefix: Option<ForwardRelativePathBuf>,
        path: ForwardRelativePathBuf,
        output_type: OutputType,
        declaration_location: Option<FileSpan>,
        path_resolution_method: BuckOutPathKind,
        heap: Heap<'v>,
    ) -> slug_error::Result<DeclaredArtifact<'v>> {
        let (path, hidden) = match prefix {
            None => (path, 0),
            Some(prefix) => (prefix.join(path), prefix.iter().count()),
        };
        // Bazel-compat: return existing artifact if this path was already declared.
        let path_str = path.as_str().to_owned();
        if let Some(existing) = self.path_to_artifact.get(&path_str) {
            return Ok(existing.dupe());
        }
        self.claim_output_path(&path, declaration_location)?;
        let out_path = BuildArtifactPath::with_dynamic_actions_action_key(
            self.owner.dupe(),
            path,
            path_resolution_method,
        );
        let declared = DeclaredArtifact::new(out_path, output_type, hidden, heap);
        if !self.artifacts.insert(declared.dupe()) {
            panic!("not expected duplicate artifact after output path was successfully claimed");
        }
        self.path_to_artifact.insert(path_str, declared.dupe());
        Ok(declared)
    }

    /// Registers the supplied action.
    ///
    /// `exec_group_name` is `Some("<name>")` when the action came from
    /// `actions.run(exec_group="<name>")` (Plan 24 Phase 8). The
    /// registry's finalize step uses it to rebase the action's RE
    /// Platform message on the named group's resolved platform instead
    /// of the default's. `None` for default-group actions.
    ///
    /// `action_exec_properties` carries the per-action `exec_properties`
    /// kwarg dict (Plan 24 Phase 9). It layers on top of the platform's
    /// `re_properties` and the target-level `exec_properties` attribute,
    /// with action-level winning. Empty for actions that didn't pass the
    /// kwarg (the common case).
    pub fn register<A: UnregisteredAction + 'static>(
        &mut self,
        self_key: &DeferredHolderKey,
        outputs: IndexSet<OutputArtifact>,
        action: A,
        exec_group_name: Option<String>,
        action_exec_properties: Arc<std::collections::BTreeMap<String, String>>,
    ) -> slug_error::Result<ActionKey> {
        // Bazel-compat idempotency: rules_cc's `_compute_public_headers` runs
        // twice for targets with `strip_include_prefix` set (once for
        // `hdrs`, once for `textual_hdrs`). Both runs call
        // `actions.declare_shareable_artifact(<same path>)` — the first
        // materializes an artifact, the second gets a dup returned from
        // `path_to_artifact` — and each subsequently calls
        // `actions.symlink(output=<that artifact>, ...)`. Under Bazel the
        // second `symlink` is a no-op. Without this guard, slug's
        // `OutputArtifact::bind` rejects the second call with
        // "Attempted to bind an artifact which was already bound".
        //
        // If every output in the set is already bound to an existing action
        // in this registry, reuse that action's key and skip re-registering.
        // If outputs are bound to *different* actions (pathological — the
        // caller is trying to overwrite a different action's result), fall
        // through to `bind()` and let it raise the usual error.
        if !outputs.is_empty() {
            let mut common_key: Option<ActionKey> = None;
            let mut all_bound_same = true;
            for output in &outputs {
                match output.existing_action_key() {
                    Some(k) => match &common_key {
                        None => common_key = Some(k),
                        Some(prev) if *prev == k => {}
                        _ => {
                            all_bound_same = false;
                            break;
                        }
                    },
                    None => {
                        all_bound_same = false;
                        break;
                    }
                }
            }
            if all_bound_same {
                if let Some(key) = common_key {
                    return Ok(key);
                }
            }
        }
        let key = ActionKey::new(
            self_key.dupe(),
            // If there are declared_dynamic_outputs, then the analysis that created this one has
            // already created ActionKeys for each of those declared outputs and so we offset the
            // index by that number.
            ActionIndex::new(
                (self.declared_dynamic_outputs.len() + self.pending.len()).try_into()?,
            ),
        );
        let mut bound_outputs = IndexSet::with_capacity(outputs.len());
        for output in outputs {
            let bound = output.bind(key.dupe())?.as_base_artifact().dupe();
            bound_outputs.insert(bound);
        }
        self.pending.push(ActionToBeRegistered::new(
            key.dupe(),
            bound_outputs,
            action,
            exec_group_name,
            action_exec_properties,
        ));

        Ok(key)
    }

    /// Consumes the registry so no more 'Action's can be registered. This returns
    /// an 'ActionAnalysisResult' that holds all the registered 'Action's
    pub fn finalize(
        self,
    ) -> slug_error::Result<
        impl FnOnce(&AnalysisValueFetcher) -> slug_error::Result<RecordedActions> + use<>,
    > {
        for artifact in self.artifacts {
            artifact.ensure_bound()?;
        }

        let num_action_keys = self.declared_dynamic_outputs.len() + self.pending.len();
        let mut actions = RecordedActions::new(num_action_keys);

        for (key, artifact) in self.declared_dynamic_outputs.into_iter() {
            actions.insert_dynamic_output(key, artifact.ensure_bound()?.action_key().dupe());
        }

        Ok(move |analysis_value_fetcher: &AnalysisValueFetcher| {
            // Slug has an invariant that pairs of categories and identifiers are unique throughout a build. That
            // invariant is enforced here, using observed_names to keep track of the categories and identifiers that we've seen.
            let mut observed_names: HashMap<Category, HashSet<String>> = HashMap::new();
            for a in self.pending.into_iter() {
                let key = a.key().dupe();
                // Plan 24 Phase 8: capture the action's exec_group name
                // before consuming `a` via `register` — the resolved
                // platform lookup below needs it to rebase the executor
                // config on the named group's platform.
                let exec_group_name: Option<String> = a.exec_group_name().map(str::to_owned);
                // Plan 24 Phase 9: capture per-action exec_properties
                // for the same reason — the merge below runs after `a`
                // has been consumed.
                let action_exec_properties = Arc::clone(a.action_exec_properties());
                let (starlark_data, error_handler) =
                    analysis_value_fetcher.get_action_data(&key)?;
                let action = a.register(starlark_data, error_handler)?;
                match (action.category(), action.identifier()) {
                    (category, Some(identifier)) => {
                        let existing_identifiers = observed_names
                            .entry(category.to_owned())
                            .or_insert_with(HashSet::<String>::new);
                        // If the category already has an unidentified action (empty string sentinel),
                        // mixing identified and unidentified actions is not allowed.
                        if existing_identifiers.contains("") {
                            return Err(ActionErrors::ActionCategoryDuplicateSingleton(
                                category.to_owned(),
                            )
                            .into());
                        }
                        // false -> identifier was already present in the set
                        if !existing_identifiers.insert(identifier.to_owned()) {
                            return Err(ActionErrors::ActionCategoryIdentifierNotUnique(
                                category.to_owned(),
                                identifier.to_owned(),
                            )
                            .into());
                        }
                    }
                    (category, None) => {
                        let existing_identifiers = observed_names
                            .entry(category.to_owned())
                            .or_insert_with(HashSet::<String>::new);
                        // If the category already has any actions (identified or unidentified),
                        // having an unidentified action is ambiguous.
                        if !existing_identifiers.is_empty() || existing_identifiers.contains("") {
                            return Err(ActionErrors::ActionCategoryDuplicateSingleton(
                                category.to_owned(),
                            )
                            .into());
                        }
                        // Use empty string as sentinel for "no identifier"
                        existing_identifiers.insert(String::new());
                    }
                }

                let executor_config = select_action_executor_config(
                    &self.execution_platform,
                    &self.group_platforms,
                    &self.target_exec_properties,
                    &action_exec_properties,
                    exec_group_name.as_deref(),
                )?;
                actions.insert(
                    key.dupe(),
                    Arc::new(RegisteredAction::new(key, action, executor_config)),
                );
            }

            Ok(actions)
        })
    }

    pub fn testing_artifacts(&self) -> &SmallSet<DeclaredArtifact<'v>> {
        &self.artifacts
    }

    pub fn testing_pending_action_keys(&self) -> Vec<ActionKey> {
        self.pending.map(|a| a.key().dupe())
    }

    pub(crate) fn execution_platform(&self) -> &ExecutionPlatformResolution {
        &self.execution_platform
    }

    pub(crate) fn actions_len(&self) -> usize {
        self.pending.len()
    }

    pub(crate) fn artifacts_len(&self) -> usize {
        self.artifacts.len()
    }
}

/// Plan 24 Phase 8 + 9 + 2: choose the action's final
/// `CommandExecutorConfig`.
///
/// Resolution rules — applied in this order:
///
/// 1. **Base.** Use the per-group platform's executor config when the
///    action was registered with `exec_group="<name>"` *and* the
///    rule's resolved `group_platforms` contains an entry for that
///    name (Phase 8). Otherwise fall back to `default_platform` —
///    this is what default-group actions and workspaces with no
///    registered platforms see.
/// 2. **Target overlay.** Layer the target's `exec_properties = {…}`
///    attribute on top (Phase 2). Target keys win over the
///    platform's contributing keys.
/// 3. **Action overlay.** Layer the per-action `exec_properties`
///    kwarg (Phase 9). Action keys win over the target's keys.
///
/// Empty overlays are no-ops. Non-`RemoteEnabled` executors pass
/// through unchanged (no RE Platform message to mutate).
pub(crate) fn select_action_executor_config(
    default_platform: &ExecutionPlatformResolution,
    group_platforms: &HashMap<String, ExecutionPlatformResolution>,
    target_exec_properties: &std::collections::BTreeMap<String, String>,
    action_exec_properties: &std::collections::BTreeMap<String, String>,
    exec_group_name: Option<&str>,
) -> slug_error::Result<Arc<CommandExecutorConfig>> {
    let base_resolution: &ExecutionPlatformResolution = exec_group_name
        .and_then(|name| group_platforms.get(name))
        .unwrap_or(default_platform);
    let mut executor_config = (*base_resolution.executor_config()?).dupe();
    if !target_exec_properties.is_empty() {
        executor_config = merge_exec_properties_overrides(&executor_config, target_exec_properties);
    }
    if !action_exec_properties.is_empty() {
        executor_config = merge_exec_properties_overrides(&executor_config, action_exec_properties);
    }
    Ok(executor_config)
}

/// Plan 24 Phases 2 + 9: rebuild a `CommandExecutorConfig` whose
/// `re_properties` are the receiver's overlaid with `overrides`
/// (overrides win on key collisions). Used for both the target-level
/// `exec_properties` attribute (Phase 2) and the per-action kwarg
/// (Phase 9). For non-`RemoteEnabled` executors the input is returned
/// unchanged — `re_properties` only exists on the remote-enabled
/// variant, and a pure-local action has no RE Platform message to
/// populate.
fn merge_exec_properties_overrides(
    config: &Arc<CommandExecutorConfig>,
    overrides: &std::collections::BTreeMap<String, String>,
) -> Arc<CommandExecutorConfig> {
    match &config.executor {
        Executor::RemoteEnabled(opts) => {
            let merged_re_properties = opts
                .re_properties
                .merged_with(overrides.iter().map(|(k, v)| (k.clone(), v.clone())));
            Arc::new(CommandExecutorConfig {
                executor: Executor::RemoteEnabled(RemoteEnabledExecutorOptions {
                    re_properties: merged_re_properties,
                    ..opts.clone()
                }),
                options: config.options.clone(),
            })
        }
        _ => config.dupe(),
    }
}

#[cfg(test)]
mod select_action_executor_config_tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use slug_core::configuration::data::ConfigurationData;
    use slug_core::execution_types::execution::ExecutionPlatform;
    use slug_core::execution_types::execution::ExecutionPlatformResolution;
    use slug_core::execution_types::executor_config::CacheUploadBehavior;
    use slug_core::execution_types::executor_config::CommandExecutorConfig;
    use slug_core::execution_types::executor_config::CommandGenerationOptions;
    use slug_core::execution_types::executor_config::Executor;
    use slug_core::execution_types::executor_config::LocalExecutorOptions;
    use slug_core::execution_types::executor_config::PathSeparatorKind;
    use slug_core::execution_types::executor_config::RePlatformFields;
    use slug_core::execution_types::executor_config::RemoteEnabledExecutor;
    use slug_core::execution_types::executor_config::RemoteEnabledExecutorOptions;
    use slug_core::execution_types::executor_config::RemoteExecutorUseCase;
    use slug_core::target::label::label::TargetLabel;
    use starlark_map::sorted_map::SortedMap;

    use super::*;

    fn re_executor_config_with_properties(
        properties: &[(&str, &str)],
    ) -> Arc<CommandExecutorConfig> {
        let map: SortedMap<String, String> = properties
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        Arc::new(CommandExecutorConfig {
            executor: Executor::RemoteEnabled(RemoteEnabledExecutorOptions {
                executor: RemoteEnabledExecutor::Local(LocalExecutorOptions::default()),
                re_properties: RePlatformFields {
                    properties: Arc::new(map),
                },
                re_use_case: RemoteExecutorUseCase::slug_default(),
                re_action_key: None,
                cache_upload_behavior: CacheUploadBehavior::Disabled,
                remote_cache_enabled: false,
                remote_dep_file_cache_enabled: false,
                dependencies: Vec::new(),
                custom_image: None,
                meta_internal_extra_params: Default::default(),
            }),
            options: CommandGenerationOptions {
                path_separator: PathSeparatorKind::Unix,
                output_paths_behavior: Default::default(),
                use_bazel_protocol_remote_persistent_workers: false,
            },
        })
    }

    fn platform_resolution_with_properties(
        label: &str,
        properties: &[(&str, &str)],
    ) -> ExecutionPlatformResolution {
        let target = TargetLabel::testing_parse(label);
        let cfg = ConfigurationData::testing_new();
        let executor_config = re_executor_config_with_properties(properties);
        let platform = ExecutionPlatform::platform(target, cfg, executor_config);
        ExecutionPlatformResolution::new(Some(platform), Vec::new())
    }

    fn re_properties(config: &CommandExecutorConfig) -> &SortedMap<String, String> {
        match &config.executor {
            Executor::RemoteEnabled(opts) => &opts.re_properties.properties,
            other => panic!("expected RemoteEnabled executor, got {other:?}"),
        }
    }

    /// Plan 24 Phase 8: an action registered with `exec_group="link"`
    /// must pick the *link group's* platform's `re_properties`, not
    /// the default group's. Regression check for the lookup at the
    /// top of `select_action_executor_config`.
    #[test]
    fn per_group_platform_routing_picks_named_group_platform() {
        let default = platform_resolution_with_properties(
            "root//:default_platform",
            &[("container-image", "docker://default:v0")],
        );
        let mut group_platforms = HashMap::new();
        group_platforms.insert(
            "link".to_owned(),
            platform_resolution_with_properties(
                "root//:link_platform",
                &[("container-image", "docker://link:v0")],
            ),
        );
        group_platforms.insert(
            "test".to_owned(),
            platform_resolution_with_properties(
                "root//:test_platform",
                &[("container-image", "docker://test:v0")],
            ),
        );
        let target_props = BTreeMap::new();
        let action_props = BTreeMap::new();

        let link_config = select_action_executor_config(
            &default,
            &group_platforms,
            &target_props,
            &action_props,
            Some("link"),
        )
        .unwrap();
        let test_config = select_action_executor_config(
            &default,
            &group_platforms,
            &target_props,
            &action_props,
            Some("test"),
        )
        .unwrap();
        let default_config = select_action_executor_config(
            &default,
            &group_platforms,
            &target_props,
            &action_props,
            None,
        )
        .unwrap();

        assert_eq!(
            re_properties(&link_config).get("container-image"),
            Some(&"docker://link:v0".to_owned())
        );
        assert_eq!(
            re_properties(&test_config).get("container-image"),
            Some(&"docker://test:v0".to_owned())
        );
        assert_eq!(
            re_properties(&default_config).get("container-image"),
            Some(&"docker://default:v0".to_owned())
        );
    }

    /// Plan 24 Phase 8 fallback: when the rule didn't declare any
    /// exec_groups (or `group_platforms` is empty for any other
    /// reason), an action that names a group falls back to the
    /// default platform — same as default-group actions. This is
    /// what guarantees workspaces with no registered platforms keep
    /// behaving exactly as before.
    #[test]
    fn unknown_exec_group_name_falls_back_to_default_platform() {
        let default = platform_resolution_with_properties(
            "root//:default_platform",
            &[("OSFamily", "Linux")],
        );
        let group_platforms = HashMap::new();

        let config = select_action_executor_config(
            &default,
            &group_platforms,
            &BTreeMap::new(),
            &BTreeMap::new(),
            Some("link"),
        )
        .unwrap();

        assert_eq!(
            re_properties(&config).get("OSFamily"),
            Some(&"Linux".to_owned())
        );
    }

    /// Plan 24 Phases 2 + 8 + 9 composed: with both per-group and
    /// per-action overrides in play, action wins on its keys, target
    /// wins on its keys, and the per-group platform contributes the
    /// rest. This is the assertion Phase 10 makes on the wire — done
    /// here at the executor config grain so a regression in
    /// `select_action_executor_config` fails CI without needing a
    /// live BES backend.
    #[test]
    fn three_layer_compose_action_target_per_group_platform() {
        let default = platform_resolution_with_properties(
            "root//:default_platform",
            &[("container-image", "docker://default:v0")],
        );
        let mut group_platforms = HashMap::new();
        group_platforms.insert(
            "link".to_owned(),
            platform_resolution_with_properties(
                "root//:link_platform",
                &[
                    ("container-image", "docker://link:v0"),
                    ("OSFamily", "Linux"),
                ],
            ),
        );

        let mut target_props = BTreeMap::new();
        target_props.insert("Arch".to_owned(), "x86_64".to_owned());
        target_props.insert(
            "container-image".to_owned(),
            "docker://target:v1".to_owned(),
        );

        let mut action_props = BTreeMap::new();
        action_props.insert(
            "container-image".to_owned(),
            "docker://action:v2".to_owned(),
        );
        action_props.insert("dockerNetwork".to_owned(), "bridge".to_owned());

        let config = select_action_executor_config(
            &default,
            &group_platforms,
            &target_props,
            &action_props,
            Some("link"),
        )
        .unwrap();
        let props = re_properties(&config);

        // platform-only key (came from link's platform) survives both layers
        assert_eq!(props.get("OSFamily"), Some(&"Linux".to_owned()));
        // target-only key contributed by middle layer survives action layer
        assert_eq!(props.get("Arch"), Some(&"x86_64".to_owned()));
        // contested key — action wins
        assert_eq!(
            props.get("container-image"),
            Some(&"docker://action:v2".to_owned())
        );
        // action-only key added by top layer
        assert_eq!(props.get("dockerNetwork"), Some(&"bridge".to_owned()));
        assert_eq!(props.len(), 4);
    }
}

#[derive(Debug, Allocative)]
pub struct RecordedActions {
    /// Vec of actions indexed by ActionKey::id.
    ///
    /// ActionLookup::Action indicates that this analysis created the action.
    ///
    /// It's possible for an Action to appear in this map multiple times. That can
    /// happen for a dynamic_outputs' "outputs" argument when the output is bound to
    /// an action created in that dynamic_output.
    ///
    /// ActionLookup::Deferred is only used for a dynamic_outputs "outputs" argument
    /// that has been re-bound to another dynamic_output.
    actions: Vec<ActionLookup>,
}

impl RecordedActions {
    pub fn new(capacity: usize) -> Self {
        Self {
            actions: Vec::with_capacity(capacity),
        }
    }

    pub fn insert(&mut self, key: ActionKey, action: Arc<RegisteredAction>) {
        self.insert_action_lookup(key, ActionLookup::Action(action));
    }

    fn insert_action_lookup(&mut self, key: ActionKey, action: ActionLookup) {
        assert!(self.actions.len() == key.action_index().0 as usize);
        self.actions.push(action);
    }

    /// Inserts a binding for a dynamic_outputs' "outputs" arg.
    pub fn insert_dynamic_output(&mut self, key: ActionKey, bound_to_key: ActionKey) {
        // TODO(cjhopman): This doesn't seem to work the way it's intended. We won't ever hit the Some case because we insert all
        // the dynamic_output "outputs" first before inserting the actual registered actions.
        match self.actions.get(bound_to_key.action_index().0 as usize) {
            Some(ActionLookup::Action(v)) => {
                // indicates that a dynamic_output "outputs" has been bound to an action it created
                self.insert_action_lookup(key, ActionLookup::Action(v.dupe()));
            }
            _ => {
                self.insert_action_lookup(key, ActionLookup::Deferred(bound_to_key));
            }
        }
    }

    pub fn lookup(&self, key: &ActionKey) -> slug_error::Result<ActionLookup> {
        self.actions
            .get(key.action_index().0 as usize)
            .duped()
            .with_internal_error(|| format!("action key missing in recorded actions {key}"))
    }

    /// Iterates over the actions created in this analysis.
    pub fn iter_actions(&self) -> impl Iterator<Item = &Arc<RegisteredAction>> + '_ {
        self.actions.iter().filter_map(|v| match v {
            ActionLookup::Action(a) => Some(a),
            ActionLookup::Deferred(_) => None,
        })
    }

    pub fn len(&self) -> usize {
        self.actions.len()
    }
}
