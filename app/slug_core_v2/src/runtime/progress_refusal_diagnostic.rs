//! Borrowed path-only retry evidence; owner headers convey no admission authority.
use std::fmt::Write;
use std::os::unix::ffi::OsStrExt;

use slug_bzlmod_v2::RepositoryMaterializationInstanceLookupError;
use slug_bzlmod_v2::RepositoryMaterializationKind;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;

use super::*;
use crate::runtime::materialization_diagnostic::BorrowedHex;

const ROWS: usize = 32;
const OWNER_EPOCH_ENTRIES: usize = 128;
const FIELD: usize = 1024;
const RAW: usize = 4096;
const ENCODED: usize = 10240;
const PREFIX: &str = "WP1100_PROGRESS_REFUSAL ";

pub(super) struct ProgressRefusalDiagnostic<'a> {
    root: &'a BuildCommandRootKey,
    workspace: &'a NormalizedAbsolutePath,
    needs: &'a SourcePreparationNeeds,
    epoch: &'a RepositoryMaterializationResultEpoch,
    lease: u64,
    attempt: usize,
}
impl<'a> ProgressRefusalDiagnostic<'a> {
    pub(super) fn for_refusal(
        root: &'a BuildCommandRootKey,
        command: &'a NativeDemandCommand<'_>,
        needs: &'a SourcePreparationNeeds,
        attempt: usize,
        error: &NativeDemandSessionError,
    ) -> Option<Self> {
        let scope = command.runtime.source_scope()?;
        matches!(
            error,
            NativeDemandSessionError::SourceService(
                source_service::SourceServiceError::WorkDeadline
            )
        )
        .then_some(Self {
            root,
            workspace: scope.workspace(),
            needs,
            epoch: &command.repository_results,
            lease: command.lease.0,
            attempt,
        })
    }
    fn families(&self) -> [(&'static str, bool, usize); 8] {
        let optional = |name, count: Option<usize>| (name, count.is_some(), count.unwrap_or(0));
        let map = |name, count| (name, count != 0, count);
        [
            optional(
                "paths",
                self.needs.path_observations().map(|n| n.demands().len()),
            ),
            optional(
                "scratch",
                self.needs.scratch_elf_metadata_requests().map(|n| n.len()),
            ),
            optional(
                "root_bootstrap",
                self.needs.root_module_bootstrap_request().map(|_| 1),
            ),
            optional(
                "environment",
                self.needs.repository_environment().map(|n| n.names().len()),
            ),
            map(
                "materializations",
                self.needs.repository_materializations().len(),
            ),
            map("downloads", self.needs.selected_extension_downloads().len()),
            map("executes", self.needs.repository_executes().len()),
            map("filesystem", self.needs.repository_filesystem_operations().len()),
        ]
    }
    fn operation(demand: &PathObservationDemand) -> Option<&'static str> {
        // Reject credited/Windows handles before operation() can Dupe them.
        if demand.image_path().is_some() || demand.windows_long_path_input().is_some() {
            return None;
        }
        Some(match demand.operation() {
            PathObservationOperation::Lstat => "lstat",
            PathObservationOperation::ReadLink => "read_link",
            PathObservationOperation::FileBytes => "file_bytes",
            PathObservationOperation::FileDigest => "file_digest",
            PathObservationOperation::DirectoryEntries => "directory_entries",
            _ => return None,
        })
    }
    fn association<W: Write>(&self, out: &mut Bounded<W>, fallback: bool) -> fmt::Result {
        if fallback {
            return write!(
                out,
                "[null,null,null,{},{},\"unavailable\"]",
                self.lease, self.attempt
            );
        }
        if self.root.targets.len() > ROWS {
            return Err(fmt::Error);
        }
        out.write_str("[")?;
        out.bytes(Some(self.root.workspace.as_path().as_os_str().as_bytes()))?;
        out.write_str(",")?;
        out.bytes(Some(self.workspace.as_path().as_os_str().as_bytes()))?;
        out.write_str(",[")?;
        for (i, target) in self.root.targets.iter().enumerate() {
            if i != 0 {
                out.write_str(",")?;
            }
            out.bytes(Some(target.as_bytes()))?;
        }
        write!(out, "],{},{},\"unavailable\"]", self.lease, self.attempt)
    }
    fn owner<W: Write>(
        &self,
        out: &mut Bounded<W>,
        instance: slug_workspace_v2::PathObservationInstanceId,
    ) -> fmt::Result {
        use RepositoryMaterializationInstanceLookupError as E;
        let view = match self.epoch.borrowed_instance_for_namespace(
            self.workspace,
            instance,
            OWNER_EPOCH_ENTRIES,
        ) {
            Ok(Some(view)) => view,
            other => {
                return write!(
                    out,
                    "[\"{}\",null]",
                    match other {
                        Ok(None) => "missing",
                        Err(E::WrongWorkspace) => "wrong_workspace",
                        Err(E::InvalidInstance) => "invalid_instance",
                        Err(E::TooManyEntries) => "bounds_refused",
                        Err(E::AmbiguousInstance) => "ambiguous",
                        Err(E::InvalidGenerationRoot) => "invalid_generation_root",
                        Ok(Some(_)) => unreachable!(),
                    }
                );
            }
        };
        let request = view.request();
        let (kind, local) = match &request.kind {
            RepositoryMaterializationKind::Local { logical_root } => {
                ("local", Some(logical_root.as_path().as_os_str().as_bytes()))
            }
            RepositoryMaterializationKind::Immutable => ("immutable", None),
            RepositoryMaterializationKind::GeneratedFileEffects(_) => {
                ("generated_file_effects", None)
            }
            RepositoryMaterializationKind::GeneratedFilesystemEffects(_) => {
                ("generated_archive_effects", None)
            }
        };
        write!(out, "[\"unique\",[\"identity_projection\",\"{kind}\",")?;
        let label = &request.repo_spec.rule_id.bzl_file;
        for bytes in [
            Some(request.id.workspace.as_path().as_os_str().as_bytes()),
            Some(request.id.canonical_repo.as_str().as_bytes()),
            Some(label.package().repo().as_str().as_bytes()),
            Some(label.package().package().as_str().as_bytes()),
            Some(label.target().as_str().as_bytes()),
            Some(request.repo_spec.rule_id.rule_name.as_bytes()),
            label.mapping_id().map(|id| id.as_str().as_bytes()),
            local,
            Some(view.generation_root().as_os_str().as_bytes()),
        ] {
            out.bytes(bytes)?;
            out.write_str(",")?;
        }
        out.write_str("\"unavailable\",\"unavailable\"]]")
    }
    fn record<W: Write>(&self, out: &mut Bounded<W>, capture: &str, fallback: bool) -> fmt::Result {
        write!(
            out,
            "{PREFIX}{{\"version\":1,\"capture\":\"{capture}\",\"refusal\":\"work_deadline\",\"association\":"
        )?;
        self.association(out, fallback)?;
        out.write_str(",\"families\":[")?;
        for (i, (name, present, count)) in self.families().iter().enumerate() {
            if i != 0 {
                out.write_str(",")?;
            }
            write!(out, "[\"{name}\",{present},{count}]")?;
        }
        out.write_str("],\"rows\":")?;
        if capture != "captured" {
            return out.write_str("null}\n");
        }
        out.write_str("[")?;
        for (i, demand) in self
            .needs
            .path_observations()
            .unwrap()
            .demands()
            .iter()
            .enumerate()
        {
            if i != 0 {
                out.write_str(",")?;
            }
            out.write_str("[")?;
            out.bytes(Some(demand.path().as_path().as_os_str().as_bytes()))?;
            write!(out, ",\"{}\",", Self::operation(demand).ok_or(fmt::Error)?)?;
            match demand.namespace() {
                PathObservationNamespace::Host => {
                    out.write_str("[\"host\",null],[\"host\",null]")?
                }
                PathObservationNamespace::Materialization(instance) => {
                    write!(out, "[\"materialization\",{}],", instance.value())?;
                    self.owner(out, instance)?;
                }
            }
            out.write_str("]")?;
        }
        out.write_str("]}\n")
    }
}
struct Sink;
impl Write for Sink {
    fn write_str(&mut self, _: &str) -> fmt::Result {
        Ok(())
    }
}
struct Bounded<W> {
    writer: W,
    raw: usize,
    encoded: usize,
}
impl<W: Write> Bounded<W> {
    fn new(writer: W) -> Self {
        Self {
            writer,
            raw: 0,
            encoded: 0,
        }
    }
    fn bytes(&mut self, bytes: Option<&[u8]>) -> fmt::Result {
        let Some(bytes) = bytes else {
            return self.write_str("null");
        };
        self.raw = self
            .raw
            .checked_add(bytes.len())
            .filter(|n| *n <= RAW && bytes.len() <= FIELD)
            .ok_or(fmt::Error)?;
        write!(self, "[{},\"", bytes.len())?;
        if !bytes.is_empty() {
            write!(self, "{}", BorrowedHex(Some(bytes)))?;
        }
        self.write_str("\"]")
    }
}
impl<W: Write> Write for Bounded<W> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.encoded = self
            .encoded
            .checked_add(text.len())
            .filter(|n| *n <= ENCODED)
            .ok_or(fmt::Error)?;
        self.writer.write_str(text)
    }
}
impl fmt::Display for ProgressRefusalDiagnostic<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fallback = self.association(&mut Bounded::new(Sink), false).is_err();
        let families = self.families();
        let mut capture = if !families[0].1
            || families[0].2 == 0
            || families[1..].iter().any(|(_, present, _)| *present)
        {
            "unsupported"
        } else if families[0].2 > ROWS {
            "bounds_refused"
        } else if self
            .needs
            .path_observations()
            .unwrap()
            .demands()
            .iter()
            .any(|d| Self::operation(d).is_none())
        {
            "unsupported"
        } else {
            "captured"
        };
        if fallback
            || self
                .record(&mut Bounded::new(Sink), capture, false)
                .is_err()
        {
            capture = "bounds_refused";
        }
        // The second pass borrows the same immutable owners. Transport may fail;
        // callers ignore that error and decoders reject the incomplete frame.
        self.record(&mut Bounded::new(formatter), capture, fallback)
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    use slug_bzlmod_v2::*;
    use slug_identity_v2::CanonicalRepoName;
    use slug_workspace_v2::ImageChargedPath;
    use slug_workspace_v2::ImageDescriptorBudget;
    use slug_workspace_v2::ImageObservationOperation;
    use slug_workspace_v2::ImageObservationRequest;
    use slug_workspace_v2::NeedPathObservations;
    use slug_workspace_v2::PathObservationInstanceId;

    use super::*;

    #[test]
    fn progress_refusal_path_projection_preserves_typed_identity_and_bounds() {
        let path = |p: &str| NormalizedAbsolutePath::new(p).unwrap();
        let workspace = path("/w");
        let mut root = BuildCommandRootKey {
            workspace: workspace.dupe(),
            targets: Arc::from([Arc::from("//pkg:probe")]),
            base_configuration: ConfigurationKey::target("proof").unwrap(),
            configuration_overlay: CommandConfigurationOverlay::default(),
        };
        let request = |name: &str| {
            Arc::new(RepositoryMaterializationRequest {
                id: RepositoryMaterializationRequestId {
                    workspace: workspace.dupe(),
                    canonical_repo: CanonicalRepoName::new(name).unwrap(),
                },
                repo_spec: RepoSpec {
                    rule_id: RepoRuleId {
                        bzl_file: CanonicalLabel::parse("@@r//p:f").unwrap(),
                        rule_name: "rule".into(),
                    },
                    attributes: Arc::default(),
                },
                kind: RepositoryMaterializationKind::Immutable,
            })
        };
        let a = request("a+");
        let weak = Arc::downgrade(&a);
        let epoch = |rows: Vec<(Arc<RepositoryMaterializationRequest>, &str, u64)>| {
            RepositoryMaterializationResultEpoch::new(
                workspace.dupe(),
                rows.into_iter().map(|(request, root, instance)| {
                    RepositoryMaterializationEpochEntry {
                        request,
                        result: RepositoryMaterializationResult::Success(
                            RepositoryMaterializationSuccess::Immutable {
                                source_identity: "proof".into(),
                                generation_root: root.into(),
                                observation_instance: PathObservationInstanceId::new(instance),
                            },
                        ),
                    }
                }),
            )
            .unwrap()
        };
        let current_a = epoch(vec![(a.clone(), "/A", 17)]);
        let current_b = epoch(vec![(a.clone(), "/B", 17)]);
        let count = Arc::strong_count(&a);
        let demand = |namespace, p: &str, op| PathObservationDemand::new(namespace, path(p), op);
        let need = |rows: Vec<PathObservationDemand>| {
            SourcePreparationNeeds::path(NeedPathObservations::try_from_iter(rows).unwrap())
        };
        let materialization =
            |n| PathObservationNamespace::Materialization(PathObservationInstanceId::new(n));
        let encode = |root: &BuildCommandRootKey,
                      needs: &SourcePreparationNeeds,
                      epoch: &RepositoryMaterializationResultEpoch| {
            let diagnostic = ProgressRefusalDiagnostic {
                root,
                workspace: &workspace,
                needs,
                epoch,
                lease: 9,
                attempt: 2,
            };
            let text = diagnostic.to_string();
            assert!(text.ends_with('\n') && text.len() <= ENCODED);
            let value: serde_json::Value =
                serde_json::from_str(text.strip_prefix(PREFIX).unwrap()).unwrap();
            assert_eq!(value.as_object().unwrap().len(), 6);
            assert_eq!(value["association"][3], 9);
            assert_eq!(value["association"][4], 2);
            (text, value)
        };
        let mut rows = Vec::new();
        for (op, tag) in [
            (PathObservationOperation::Lstat, "lstat"),
            (PathObservationOperation::ReadLink, "read_link"),
            (PathObservationOperation::FileBytes, "file_bytes"),
            (PathObservationOperation::FileDigest, "file_digest"),
            (
                PathObservationOperation::DirectoryEntries,
                "directory_entries",
            ),
        ] {
            rows.push(demand(PathObservationNamespace::Host, "/same", op.dupe()));
            let single = need(vec![demand(materialization(17), "/same", op)]);
            let (_, json) = encode(&root, &single, &current_a);
            assert_eq!(json["capture"], "captured");
            assert_eq!(json["rows"][0][1], tag);
            assert_eq!(
                json["rows"][0][2],
                serde_json::json!(["materialization", 17])
            );
            let header = &json["rows"][0][3][1];
            assert_eq!(header.as_array().unwrap().len(), 13);
            assert_eq!(header[0], "identity_projection");
            assert_eq!(header[11], "unavailable");
            assert_eq!(header[12], "unavailable");
        }
        let mut raw = b"/raw/".to_vec();
        raw.extend([0xe9, 0]);
        rows.push(PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(PathBuf::from(OsString::from_vec(raw.clone()))).unwrap(),
            PathObservationOperation::Lstat,
        ));
        let all = need(rows);
        let original = all.path_observations().unwrap().demands().as_ptr();
        let (text, json) = encode(&root, &all, &current_a);
        assert_eq!(json["rows"].as_array().unwrap().len(), 6);
        assert!(text.contains("2f7261772fe900"));
        assert_eq!(
            all.path_observations().unwrap().demands().as_ptr(),
            original
        );
        assert_eq!(Arc::strong_count(&a), count);
        let single = need(vec![demand(
            materialization(17),
            "/A/file",
            PathObservationOperation::FileBytes,
        )]);
        let (_, ja) = encode(&root, &single, &current_a);
        let (_, jb) = encode(&root, &single, &current_b);
        assert_ne!(ja["rows"][0][3][1][10], jb["rows"][0][3][1][10]); // Current B is not inferred from the stale A path.
        assert_eq!(
            current_a
                .borrowed_instance_for_namespace(
                    &workspace,
                    PathObservationInstanceId::new(17),
                    OWNER_EPOCH_ENTRIES
                )
                .unwrap()
                .unwrap()
                .request() as *const _,
            Arc::as_ptr(&a)
        );
        for (entries, duplicate) in [
            (32usize, false),
            (33, false),
            (128, false),
            (129, false),
            (128, true),
        ] {
            let mut original_root = Some(PathBuf::from("/A"));
            let root_pointer = original_root
                .as_ref()
                .unwrap()
                .as_os_str()
                .as_bytes()
                .as_ptr();
            let bounded = RepositoryMaterializationResultEpoch::new(
                workspace.dupe(),
                (0..entries).map(|index| {
                    let owner = if index == 0 {
                        a.clone()
                    } else {
                        let owner = request(&if duplicate && index == entries - 1 {
                            "zzduplicate+".to_owned()
                        } else {
                            format!("{}{index:03}+", if duplicate { "r" } else { "0r" })
                        });
                        if duplicate {
                            assert!(owner.id.canonical_repo > a.id.canonical_repo);
                            assert!(owner.id.canonical_repo.as_str() <= "zzduplicate+");
                        } else {
                            assert!(owner.id.canonical_repo < a.id.canonical_repo);
                        }
                        owner
                    };
                    RepositoryMaterializationEpochEntry {
                        request: owner,
                        result: RepositoryMaterializationResult::Success(
                            RepositoryMaterializationSuccess::Immutable {
                                source_identity: "proof".into(),
                                generation_root: if index == 0 {
                                    original_root.take().unwrap()
                                } else {
                                    PathBuf::from("/A")
                                },
                                observation_instance: PathObservationInstanceId::new(
                                    if index == 0 || (duplicate && index == entries - 1) {
                                        17
                                    } else {
                                        24 + index as u64
                                    },
                                ),
                            },
                        ),
                    }
                }),
            )
            .unwrap();
            let owners = Arc::strong_count(&a);
            let (_, projected) = encode(&root, &single, &bounded);
            assert_eq!(projected["capture"], "captured");
            let view = bounded.borrowed_instance_for_namespace(
                &workspace,
                PathObservationInstanceId::new(17),
                OWNER_EPOCH_ENTRIES,
            );
            if duplicate {
                assert_eq!(
                    projected["rows"][0][3],
                    serde_json::json!(["ambiguous", null])
                );
                assert_eq!(
                    view,
                    Err(RepositoryMaterializationInstanceLookupError::AmbiguousInstance)
                );
            } else if entries <= OWNER_EPOCH_ENTRIES {
                assert_eq!(projected["rows"][0][3], ja["rows"][0][3]);
                let view = view.unwrap().unwrap();
                assert!(std::ptr::eq(view.request(), a.as_ref()));
                assert_eq!(
                    view.generation_root().as_os_str().as_bytes().as_ptr(),
                    root_pointer
                );
                assert_eq!(Arc::strong_count(&a), owners);
            } else {
                assert_eq!(
                    projected["rows"][0][3],
                    serde_json::json!(["bounds_refused", null])
                );
                assert_eq!(
                    view,
                    Err(RepositoryMaterializationInstanceLookupError::TooManyEntries)
                );
            }
            assert_eq!(Arc::strong_count(&a), owners);
            drop(bounded);
            assert_eq!(Arc::strong_count(&a), count);
        }
        for (other, expected) in [
            (epoch(vec![]), "missing"),
            (
                epoch(vec![(a.clone(), "/A", 17), (request("b+"), "/B", 17)]),
                "ambiguous",
            ),
            (
                epoch(vec![(a.clone(), "relative", 17)]),
                "invalid_generation_root",
            ),
        ] {
            assert_eq!(encode(&root, &single, &other).1["rows"][0][3][0], expected);
        }
        let foreign = RepositoryMaterializationResultEpoch::new(path("/foreign"), []).unwrap();
        assert_eq!(
            encode(&root, &single, &foreign).1["rows"][0][3][0],
            "wrong_workspace"
        );
        let zero = need(vec![demand(
            materialization(0),
            "/A/file",
            PathObservationOperation::Lstat,
        )]);
        assert_eq!(
            encode(&root, &zero, &current_a).1["rows"][0][3][0],
            "invalid_instance"
        );
        let mut generated = (*request("generated+")).clone();
        generated.kind = RepositoryMaterializationKind::GeneratedFileEffects(
            GeneratedRepositoryFileEffectPlan::build([]).unwrap(),
        );
        let generated_epoch = epoch(vec![(Arc::new(generated), "/G", 17)]);
        assert_eq!(
            encode(&root, &single, &generated_epoch).1["rows"][0][3][1][1],
            "generated_file_effects"
        );
        let budget = ImageDescriptorBudget::new(1 << 20);
        let charged = ImageChargedPath::pin_existing(workspace.dupe(), &budget).unwrap();
        let image = PathObservationDemand::input_image(
            PathObservationNamespace::Host,
            charged.dupe(),
            ImageObservationRequest::new(charged, ImageObservationOperation::Lstat),
        )
        .unwrap();
        let credit = budget.used_bytes();
        for unsupported in [
            image,
            PathObservationDemand::windows_long_path(workspace.dupe(), Arc::from([0x2fu16, 0x77])),
            PathObservationDemand::windows_option_path_long_name(
                workspace.dupe(),
                Arc::from([0x2fu16, 0x77]),
            ),
        ] {
            let mixed = need(vec![
                demand(
                    PathObservationNamespace::Host,
                    "/ordinary",
                    PathObservationOperation::Lstat,
                ),
                unsupported,
            ]);
            let retained_credit = budget.used_bytes();
            let (_, j) = encode(&root, &mixed, &current_a);
            assert_eq!(j["capture"], "unsupported");
            assert!(j["rows"].is_null());
            assert_eq!(budget.used_bytes(), retained_credit);
        }
        assert!(budget.used_bytes() < credit); // The local image fixture has been dropped, no formatter retention.
        // Obtain the existing genuine selected-owner fixture, not a fabricated owner identity.
        let fixture = selected_tool_source::tests::Fixture::new(
            "def impl(ctx):\n    pass\next=module_extension(implementation=impl)\n",
        );
        let owner = fixture.key.owner().dupe();
        let resources = &fixture.runtime.scratch_resources;
        let mut builder = SelectedScratchPrefixBuilder::new(
            ImageChargedPath::pin_existing(workspace.dupe(), resources.descriptor_budget())
                .unwrap(),
            owner.dupe(),
            SelectedScratchPolicy::plain(),
            resources,
        )
        .unwrap();
        let member = builder.path("generator").unwrap();
        builder.file(&member, b"tiny", true).unwrap();
        let prefix = builder
            .finish(
                SelectedScratchProcessSpec {
                    arguments: &[SelectedScratchOperandSpec::Scratch(&member)],
                    cwd: "/__slug_module",
                    environment: &[],
                    timeout_seconds: 1,
                    quiet: true,
                },
                &RepositoryEnvironmentSnapshot::empty(),
            )
            .unwrap();
        let operand = SelectedToolOperand {
            role: SelectedToolRole::Generator,
            argument_index: 0,
        };
        let policy = SelectedToolSourcesPolicy::new(
            &[operand],
            slug_workspace_v2::ElfMetadataPolicy::default(),
        )
        .unwrap();
        let selection =
            SelectedToolSourceSelection::from_prefix(&prefix, policy, operand, resources).unwrap();
        let binding = slug_workspace_v2::ElfSourceDigestBinding::from_digest(
            slug_workspace_v2::FileContentDigest::new([0; 32], 4).unwrap(),
        );
        let scratch =
            SelectedScratchElfMetadataRequest::new(&selection, binding, resources).unwrap();
        for (family, other) in [
            (
                "scratch",
                SourcePreparationNeeds::scratch_elf_metadata(
                    SelectedScratchElfMetadataNeeds::singleton(&scratch, resources).unwrap(),
                ),
            ),
            (
                "root_bootstrap",
                SourcePreparationNeeds::root_module_bootstrap(RootModuleBootstrapRequest {
                    workspace: workspace.dupe(),
                }),
            ),
            (
                "environment",
                SourcePreparationNeeds::environment(
                    NeedRepositoryEnvironmentNames::new(
                        workspace.dupe(),
                        RepositoryEnvironmentNameFrontier::from_unsorted([
                            compact_str::CompactString::new("NAME"),
                        ]),
                    )
                    .unwrap(),
                ),
            ),
            (
                "materializations",
                SourcePreparationNeeds::repository((*a).clone()),
            ),
            (
                "downloads",
                SourcePreparationNeeds::selected_extension_download(
                    SelectedExtensionDownloadRequest::new(
                        workspace.dupe(),
                        owner.dupe(),
                        host_repository_relative_path("file".into()).unwrap(),
                        "file:///fixture",
                        [0; 32],
                        false,
                    )
                    .unwrap(),
                ),
            ),
            (
                "executes",
                SourcePreparationNeeds::repository_execute(RepositoryExecuteRequest::new(
                    RepositoryExecuteCallId::new(owner.dupe(), 0, 0),
                    "repo".into(),
                    Arc::from([RepositoryExecuteArg::String("literal".into())]),
                    RepositoryEnvironmentSnapshot::empty(),
                    GeneratedRepositoryFileEffectPlan::build([]).unwrap(),
                )),
            ),
        ] {
            let (_, standalone) = encode(&root, &other, &current_a);
            assert_eq!(standalone["capture"], "unsupported");
            assert!(standalone["rows"].is_null());
            assert_eq!(
                standalone["families"][0],
                serde_json::json!(["paths", false, 0])
            );
            let mixed = all.try_union(&other).unwrap();
            let (_, j) = encode(&root, &mixed, &current_a);
            assert_eq!(j["capture"], "unsupported");
            assert!(j["rows"].is_null());
            assert!(
                j["families"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row == &serde_json::json!([family, true, 1]))
            );
        }
        for bounded in [
            need(
                (0..33)
                    .map(|i| {
                        demand(
                            PathObservationNamespace::Host,
                            &format!("/p{i}"),
                            PathObservationOperation::Lstat,
                        )
                    })
                    .collect(),
            ),
            need(vec![demand(
                PathObservationNamespace::Host,
                &format!("/{}", "x".repeat(FIELD)),
                PathObservationOperation::Lstat,
            )]),
            need(
                (0..5)
                    .map(|i| {
                        demand(
                            PathObservationNamespace::Host,
                            &format!("/{i}{}", "x".repeat(900)),
                            PathObservationOperation::Lstat,
                        )
                    })
                    .collect(),
            ),
        ] {
            let (_, j) = encode(&root, &bounded, &current_a);
            assert_eq!(j["capture"], "bounds_refused");
            assert!(j["rows"].is_null());
        }
        let encoded = need(
            (0..32)
                .map(|i| {
                    demand(
                        materialization(17),
                        &format!("/{i}{}", "x".repeat(90)),
                        PathObservationOperation::Lstat,
                    )
                })
                .collect(),
        );
        let diagnostic = ProgressRefusalDiagnostic {
            root: &root,
            workspace: &workspace,
            needs: &encoded,
            epoch: &current_a,
            lease: 9,
            attempt: 2,
        };
        let mut preflight = Bounded::new(Sink);
        assert!(
            diagnostic
                .record(&mut preflight, "captured", false)
                .is_err()
        );
        assert!(preflight.raw <= RAW);
        let (_, j) = encode(&root, &encoded, &current_a);
        assert_eq!(j["capture"], "bounds_refused");
        assert!(j["rows"].is_null());
        root.targets = Arc::from([]);
        assert_eq!(encode(&root, &all, &current_a).1["capture"], "captured");
        root.targets = (0..33).map(|_| Arc::from("//pkg:probe")).collect();
        let (_, j) = encode(&root, &all, &current_a);
        assert_eq!(j["capture"], "bounds_refused");
        assert!(j["association"][0].is_null() && j["association"][2].is_null());
        let mut out = Bounded::new(String::new());
        out.bytes(Some(b"")).unwrap();
        assert_eq!(out.writer, "[0,\"\"]");
        let mut out = Bounded::new(Sink);
        for _ in 0..4 {
            out.bytes(Some(&[0; FIELD])).unwrap();
        }
        assert_eq!(out.raw, RAW);
        assert!(out.bytes(Some(b"x")).is_err());
        let mut out = Bounded::new(Sink);
        out.write_str(&"x".repeat(ENCODED)).unwrap();
        assert!(out.write_str("\n").is_err());
        struct Fail;
        impl Write for Fail {
            fn write_str(&mut self, _: &str) -> fmt::Result {
                Err(fmt::Error)
            }
        }
        let diagnostic = ProgressRefusalDiagnostic {
            root: &root,
            workspace: &workspace,
            needs: &all,
            epoch: &current_a,
            lease: 9,
            attempt: 2,
        };
        assert!(write!(&mut Fail, "{diagnostic}").is_err());
        drop(diagnostic);
        drop(current_a);
        drop(current_b);
        drop(a);
        assert!(weak.upgrade().is_none());
        println!(
            "WP1100_LAYOUT view={}:{} bounded_sink={}:{}",
            std::mem::size_of::<ProgressRefusalDiagnostic<'_>>(),
            std::mem::align_of::<ProgressRefusalDiagnostic<'_>>(),
            std::mem::size_of::<Bounded<Sink>>(),
            std::mem::align_of::<Bounded<Sink>>()
        );
    }
}
