//! Final capture and freshness under the one command; outer slots pin successes
//! through restoration even if a later capture or post-capture check refuses.
use super::*;

pub(super) struct Completion<'a, 'request> {
    pub(super) mode: &'a PreparationMode,
    pub(super) key: &'a SelectedExtensionToolSourcesKey,
    pub(super) plan: &'a ObservedSelectedExtensionToolSourceRequests,
    pub(super) runtime_plan: Option<&'a slug_loading_v2::ObservedRuntimeSourceRequests>,
    pub(super) runtime_profile: Option<slug_loading_v2::ObservedRuntimeSourceProfile>,
    pub(super) namespace_facts: Option<slug_loading_v2::ObservedGuestNamespaceFacts>,
    pub(super) data_plan: Option<&'a slug_loading_v2::ObservedSelectedDataLabelRequests>,
    pub(super) catalog: &'a declarations::Catalog,
    pub(super) runtime_catalog: Option<&'a runtime_sources::catalogue::Catalogue>,
    pub(super) data_catalog: Option<&'a selected_label_data::catalogue::Catalogue>,
    pub(super) certificate: ObservedSelectedExtensionToolSources,
    pub(super) selected: NativeDemandTerminalSelection,
    pub(super) owner:
        Option<&'a super::super::super::source_preparation::OwnerAuthorization<'request>>,
    pub(super) deadline: Instant,
    #[cfg(test)]
    pub(super) hook: Option<&'a dyn Fn(ToolPhase<'_>) -> Result<(), NativeDemandSessionError>>,
}
impl Completion<'_, '_> {
    pub(super) fn complete(
        self,
        runtime: &WorkspaceRuntime,
        command: &NativeDemandCommand<'_>,
        stages: &mut setup::Stages,
    ) -> Result<PreparedLease, NativeDemandSessionError> {
        let Self {
            mode,
            key,
            plan,
            runtime_plan,
            runtime_profile,
            namespace_facts,
            data_plan,
            catalog,
            runtime_catalog,
            data_catalog,
            certificate,
            selected,
            owner,
            deadline,
            #[cfg(test)]
            hook,
        } = self;
        let reachable = selected.sidecars().demands();
        if let (PreparationMode::Namespace { key, .. }, Some(facts)) = (mode, &namespace_facts) {
            guest_namespace::validate(command, key, facts, runtime_profile.as_ref().unwrap())?;
        }

        validate_final(command, key, plan, &certificate, reachable, deadline)?;
        #[cfg(test)]
        if let Some(hook) = hook {
            hook(ToolPhase::BeforeCapture)?;
        }
        super::super::super::source_preparation::attempt::verify(
            command, owner, reachable, deadline,
        )?;
        verify_final(
            command,
            key,
            plan,
            &certificate,
            reachable,
            &catalog,
            deadline,
        )?;
        if let (Some(data_plan), Some(facts), Some(data_catalog)) =
            (&data_plan, &namespace_facts, &data_catalog)
        {
            let data = facts
                .data_sources()
                .ok_or_else(|| refusal("missing normal data sources"))?;
            selected_label_data::prepare::verify_final(
                command,
                data_catalog_key(mode),
                data_plan,
                data,
                reachable,
                &catalog,
                runtime_catalog.unwrap(),
                data_catalog,
                deadline,
            )?;
        }
        if let (
            PreparationMode::Runtime { key, .. }
            | PreparationMode::Namespace {
                runtime_key: key, ..
            },
            Some(profile),
        ) = (mode, &runtime_profile)
        {
            runtime_sources::prepare::verify_final(
                command,
                key,
                runtime_plan.unwrap(),
                profile,
                reachable,
                &catalog,
                runtime_catalog.unwrap(),
                deadline,
            )?;
        }

        let mut image_for_role = [None; 3];
        for (index, source) in certificate.sources().enumerate() {
            let Some(metadata) = source.label_metadata() else {
                continue;
            };
            let earlier = certificate
                .sources()
                .take(index)
                .enumerate()
                .find(|(_, earlier)| {
                    earlier
                        .label_metadata()
                        .is_some_and(|earlier| earlier.image() == metadata.image())
                });
            if let Some((earlier, _)) = earlier {
                image_for_role[index] = image_for_role[earlier];
                continue;
            }
            stages.images[index] = Some(
                catalog.capture(
                    command,
                    index,
                    metadata.image().dupe(),
                    stages.image_stages[index]
                        .as_mut()
                        .expect("prepared image stage"),
                    deadline,
                    #[cfg(test)]
                    Some(&|path| {
                        hook.map_or(Ok(()), |hook| hook(ToolPhase::CaptureActivity(path)))
                            .is_ok()
                    }),
                )?,
            );
            stages.image_stages[index].take();
            image_for_role[index] = Some(index);
        }
        let scratch = stages
            .scratch
            .as_mut()
            .expect("prepared scratch stage")
            .capture_retaining(
                command,
                key.checkpoint(),
                certificate.prefix().dupe(),
                reachable,
                deadline,
            )?;
        stages.captured_scratch = Some(scratch);
        let tools = stages
            .owner_credit
            .take()
            .expect("tool owner credit")
            .freeze_preserving(CertifiedToolInputOwner {
                certificate,
                scratch: stages.captured_scratch.as_ref().unwrap().dupe(),
                images: std::array::from_fn(|i| stages.images[i].as_ref().map(Dupe::dupe)),
                image_for_role,
                unused_stages: std::mem::replace(
                    &mut stages.image_stages,
                    std::array::from_fn(|_| None),
                ),
            });
        let tools = match tools {
            Ok(block) => CertifiedSelectedToolInputsLease(block),
            Err((error, partial, credit)) => {
                stages.failed_tools = Some(partial);
                stages.owner_credit = Some(credit);
                return Err(image_failure(error));
            }
        };
        stages.tools = Some(tools.dupe());
        let certificate = tools.certificate();
        if let Some(profile) = &runtime_profile {
            stages.runtime_storage.as_mut().unwrap().capture(
                command,
                profile,
                runtime_catalog.unwrap(),
                &tools,
            )?;
        }
        #[cfg(test)]
        if let Some(hook) = hook {
            hook(ToolPhase::AfterCapture)?;
        }
        super::super::super::source_preparation::attempt::verify(
            command, owner, reachable, deadline,
        )?;
        verify_final(
            command,
            key,
            plan,
            &certificate,
            reachable,
            &catalog,
            deadline,
        )?;
        if let (
            PreparationMode::Runtime { key, .. }
            | PreparationMode::Namespace {
                runtime_key: key, ..
            },
            Some(profile),
        ) = (mode, &runtime_profile)
        {
            runtime_sources::prepare::verify_final(
                command,
                key,
                runtime_plan.unwrap(),
                profile,
                reachable,
                &catalog,
                runtime_catalog.unwrap(),
                deadline,
            )?;
        }
        if let (PreparationMode::Namespace { key, .. }, Some(facts)) = (mode, &namespace_facts) {
            guest_namespace::validate(command, key, facts, runtime_profile.as_ref().unwrap())?;
        }
        let data_lease = if let (Some(storage), Some(facts), Some(data_catalog), Some(data_plan)) = (
            stages.data_storage.as_mut(),
            &namespace_facts,
            &data_catalog,
            &data_plan,
        ) {
            let data = facts
                .data_sources()
                .ok_or_else(|| refusal("missing normal data sources"))?;
            storage.capture(command, data, data_catalog)?;
            selected_label_data::prepare::verify_final(
                command,
                data_catalog_key(mode),
                data_plan,
                data,
                reachable,
                &catalog,
                runtime_catalog.unwrap(),
                data_catalog,
                deadline,
            )?;
            Some(stages.data_storage.as_mut().unwrap().finish(data.dupe())?)
        } else {
            None
        };
        stages.data = data_lease.as_ref().map(Dupe::dupe);
        let (_, _, terminal) = selected.sidecars.into_parts();
        terminal
            .reset_to_idle()
            .map_err(NativeDemandSessionError::Effect)?;
        return if let Some(profile) = runtime_profile {
            let sources = stages
                .runtime_storage
                .as_mut()
                .expect("runtime prepared storage")
                .finish(profile, tools)?;
            stages.runtime = Some(sources.dupe());
            if let Some(facts) = namespace_facts {
                guest_namespace::finish_retaining(
                    facts,
                    sources,
                    data_lease,
                    stages
                        .namespace_capture
                        .as_mut()
                        .expect("namespace owner credit"),
                    runtime.image_resources.descriptor_budget(),
                )
                .map(PreparedLease::Namespace)
            } else {
                Ok(PreparedLease::Runtime(sources))
            }
        } else {
            Ok(PreparedLease::Tools(tools))
        };
    }
}
