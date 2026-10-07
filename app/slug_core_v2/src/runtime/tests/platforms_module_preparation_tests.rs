use std::mem::Discriminant;
use std::os::unix::ffi::OsStrExt;
use std::sync::Weak;

use sha2::Digest;
use slug_bzlmod_v2::HostRootRepositoryMappingObservationKey;
use slug_bzlmod_v2::ObservedHostRootRepositoryMapping;
use slug_bzlmod_v2::OverrideAttributeValue;
use slug_bzlmod_v2::RepositoryMaterializationKind;
use slug_bzlmod_v2::RootModuleCommandPolicyKey;
use slug_bzlmod_v2::RootModuleLoadingAnchorObservationKey;
use slug_bzlmod_v2::SelectedSourceRefusal;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_workspace_v2::NeedPathObservations;
use slug_workspace_v2::ObservedPathFrontierError;

use super::*;

const PLATFORMS_SHA: [u8; 32] = [
    0x1c, 0x0c, 0x09, 0xf5, 0xbd, 0xcf, 0x4b, 0x3f, 0x92, 0x47, 0x20, 0xd2, 0x47, 0x8a, 0x37, 0x11,
    0xcb, 0x39, 0xf4, 0x97, 0x70, 0x19, 0xca, 0x59, 0x88, 0x68, 0x5e, 0x5b, 0x7e, 0x18, 0xb3, 0xd2,
];
fn platforms_bytes(bytes: &[u8]) -> bool {
    bytes.len() == 416 && sha2::Sha256::digest(bytes).as_slice() == PLATFORMS_SHA
}

const ROOT_SHA: [u8; 32] = [
    0xa3, 0x57, 0x13, 0x30, 0x30, 0xb9, 0x43, 0x60, 0xb3, 0x70, 0x61, 0xf3, 0x80, 0x98, 0x4c, 0xaa,
    0x37, 0x4f, 0x73, 0x83, 0x84, 0x1f, 0xab, 0xdc, 0x8c, 0x5d, 0x26, 0xbc, 0x11, 0x4e, 0x3b, 0x10,
];
fn root_bytes(bytes: &[u8]) -> bool {
    bytes.len() == 426 && sha2::Sha256::digest(bytes).as_slice() == ROOT_SHA
}

const FEATURES_SHA: [u8; 32] = [
    0x20, 0x83, 0xef, 0x9c, 0x7a, 0x46, 0x9f, 0x52, 0x08, 0x90, 0x48, 0x3c, 0xcf, 0x8e, 0x01, 0x89,
    0xd6, 0xe7, 0x1e, 0x21, 0x17, 0xe7, 0x75, 0x2e, 0x15, 0xe6, 0x55, 0x44, 0x33, 0xd5, 0xae, 0x3e,
];
const RULES_SHA: [u8; 32] = [
    0x25, 0xe3, 0xb0, 0x77, 0x12, 0x86, 0x12, 0x75, 0x4c, 0x4a, 0xdd, 0x1b, 0x4c, 0x90, 0xd2, 0x0a,
    0x6b, 0xe0, 0x65, 0x66, 0xb6, 0x23, 0xde, 0xe6, 0xe3, 0x20, 0x38, 0xd0, 0xe8, 0xf9, 0x30, 0x62,
];
const SKYLIB_SHA: [u8; 32] = [
    0x69, 0xad, 0x69, 0x27, 0x09, 0x83, 0x16, 0x84, 0x8b, 0x34, 0xa9, 0x14, 0x2b, 0xcc, 0x97, 0x5e,
    0x01, 0x8b, 0xa2, 0x7f, 0x08, 0xc4, 0xff, 0x40, 0x3f, 0x50, 0xc1, 0xb6, 0xe6, 0x46, 0xca, 0x67,
];
// This exact local body declares 0.2.17; it is not the prior registry 0.2.4 source.
const CC_SHA: [u8; 32] = [
    0x18, 0x49, 0x60, 0x2c, 0x86, 0xcb, 0x60, 0xda, 0x86, 0x13, 0xd2, 0xde, 0x88, 0x7f, 0x95, 0x66,
    0xa6, 0xd3, 0x54, 0xa6, 0xdf, 0x6d, 0x70, 0x09, 0xf9, 0xd0, 0x4a, 0x14, 0x40, 0x2f, 0x9a, 0x84,
];
const LICENSE_SHA: [u8; 32] = [
    0xa7, 0xfd, 0xa6, 0x0e, 0xef, 0xdf, 0x3d, 0x8c, 0x82, 0x72, 0x62, 0xba, 0x49, 0x99, 0x57, 0xe4,
    0xdf, 0x06, 0xf6, 0x59, 0x33, 0x0b, 0xbe, 0x6c, 0xdb, 0xdb, 0x97, 0x5b, 0x76, 0x8b, 0xb6, 0x5c,
];
const ADDITIONAL_IDENTITIES: [(&str, &str); 4] = [
    ("bazel_features", "bazel_features+"),
    ("bazel_skylib", "bazel_skylib+"),
    ("rules_cc", "rules_cc+"),
    ("rules_license", "rules_license+"),
];
const ADDITIONAL_ENV: [&str; 4] = [
    "SLUG_NATIVE_BAZEL_FEATURES_ROOT",
    "SLUG_NATIVE_BAZEL_SKYLIB_ROOT",
    "SLUG_NATIVE_RULES_CC_ROOT",
    "SLUG_NATIVE_RULES_LICENSE_ROOT",
];
const ADDITIONAL_MARKERS: [(&str, &str); 4] = [
    ("WP945", "bazel_features_root"),
    ("WP946", "bazel_skylib_root"),
    ("WP947", "rules_cc_root"),
    ("WP948", "rules_license_root"),
];
fn additional_bytes(slot: usize, bytes: &[u8]) -> bool {
    let (size, hash) = match slot {
        0 => (346, FEATURES_SHA),
        1 => (389, SKYLIB_SHA),
        2 => (1181, CC_SHA),
        3 => (753, LICENSE_SHA),
        _ => unreachable!(),
    };
    bytes.len() == size && sha2::Sha256::digest(bytes).as_slice() == hash
}
#[derive(Clone, Copy)]
enum ModuleMode {
    Three,
    Four,
    Five,
    Six,
    Seven,
}
impl ModuleMode {
    fn additional_count(self) -> usize {
        match self {
            Self::Three => 0,
            Self::Four => 1,
            Self::Five => 2,
            Self::Six => 3,
            Self::Seven => 4,
        }
    }
}
struct AdditionalModule {
    root: NormalizedAbsolutePath,
    file: NormalizedAbsolutePath,
}
struct Bodies {
    roots: [NormalizedAbsolutePath; 3],
    files: [NormalizedAbsolutePath; 3],
    additional: [Option<AdditionalModule>; 4],
}
#[derive(Debug)]
struct Denied {
    family: &'static str,
    path: Option<NormalizedAbsolutePath>,
}
impl Bodies {
    fn new(roots: [NormalizedAbsolutePath; 3]) -> Self {
        let files = roots
            .each_ref()
            .map(|root| NormalizedAbsolutePath::new(root.as_path().join("MODULE.bazel")).unwrap());
        Self {
            roots,
            files,
            additional: [None, None, None, None],
        }
    }
    fn with_fourth(mut self, root: NormalizedAbsolutePath) -> Self {
        let file = NormalizedAbsolutePath::new(root.as_path().join("MODULE.bazel")).unwrap();
        self.additional[0] = Some(AdditionalModule { root, file });
        self
    }
    fn with_fifth(mut self, root: NormalizedAbsolutePath) -> Self {
        assert!(
            self.additional[0].is_some(),
            "fifth binding requires fourth"
        );
        let file = NormalizedAbsolutePath::new(root.as_path().join("MODULE.bazel")).unwrap();
        self.additional[1] = Some(AdditionalModule { root, file });
        self
    }
    fn with_sixth(mut self, root: NormalizedAbsolutePath) -> Self {
        assert!(
            self.additional[..2].iter().all(Option::is_some),
            "sixth binding requires fourth and fifth"
        );
        let file = NormalizedAbsolutePath::new(root.as_path().join("MODULE.bazel")).unwrap();
        self.additional[2] = Some(AdditionalModule { root, file });
        self
    }
    fn with_seventh(mut self, root: NormalizedAbsolutePath) -> Self {
        assert!(
            self.additional[..3].iter().all(Option::is_some),
            "seventh binding requires fourth, fifth and sixth"
        );
        let file = NormalizedAbsolutePath::new(root.as_path().join("MODULE.bazel")).unwrap();
        self.additional[3] = Some(AdditionalModule { root, file });
        self
    }
    fn roots(&self) -> impl Iterator<Item = &NormalizedAbsolutePath> {
        self.roots.iter().chain(
            self.additional
                .iter()
                .flatten()
                .map(|binding| &binding.root),
        )
    }
    fn files(&self) -> impl Iterator<Item = &NormalizedAbsolutePath> {
        self.files.iter().chain(
            self.additional
                .iter()
                .flatten()
                .map(|binding| &binding.file),
        )
    }
    fn bindings(
        &self,
    ) -> impl Iterator<Item = (&'static str, &'static str, &NormalizedAbsolutePath)> {
        [
            ("rules_rust", "rules_rust+", &self.roots[1]),
            ("platforms", "platforms+", &self.roots[2]),
        ]
        .into_iter()
        .chain(
            self.additional
                .iter()
                .enumerate()
                .filter_map(|(slot, binding)| {
                    binding.as_ref().map(|binding| {
                        (
                            ADDITIONAL_IDENTITIES[slot].0,
                            ADDITIONAL_IDENTITIES[slot].1,
                            &binding.root,
                        )
                    })
                }),
        )
    }
    fn admit(&self, needs: &SourcePreparationNeeds) -> Result<(), Denied> {
        let refusal = |family| Denied { family, path: None };
        if needs.scratch_elf_metadata_requests().is_some() {
            return Err(refusal("scratch"));
        }
        if needs
            .root_module_bootstrap_request()
            .is_some_and(|r| r.workspace != self.roots[0])
        {
            return Err(refusal("bootstrap"));
        }
        if needs.repository_environment().is_some() {
            return Err(refusal("environment"));
        }
        if !needs.selected_extension_downloads().is_empty() {
            return Err(refusal("download"));
        }
        if !needs.repository_executes().is_empty() {
            return Err(refusal("execute"));
        }
        for (_, request) in needs.repository_materializations().iter() {
            let RepositoryMaterializationKind::Local { logical_root } = &request.kind else {
                return Err(refusal("materialization"));
            };
            let expected = self.bindings().any(|(_, canonical, root)| {
                request.id.canonical_repo.as_str() == canonical && logical_root == root
            });
            let label = &request.repo_spec.rule_id.bzl_file;
            let attributes = &request.repo_spec.attributes;
            let path_matches = matches!(attributes.get("path"), Some(OverrideAttributeValue::String(path)) if Path::new(path.as_str()) == logical_root.as_path());
            if request.id.workspace != self.roots[0]
                || !expected
                || attributes.len() != 1
                || !path_matches
                || request.repo_spec.rule_id.rule_name != "local_repository"
                || label.mapping_id().is_some()
                || label.package().repo().as_str() != "bazel_tools"
                || label.package().package().as_str() != "tools/build_defs/repo"
                || label.target().as_str() != "local.bzl"
            {
                return Err(refusal("materialization"));
            }
        }
        if let Some(paths) = needs.path_observations() {
            for demand in paths.demands() {
                let deny = |family| Denied {
                    family,
                    path: Some(demand.path().dupe()),
                };
                if demand.namespace() != PathObservationNamespace::Host {
                    return Err(deny("namespace"));
                }
                let path = demand.path().as_path();
                let allowed = match demand.operation() {
                    PathObservationOperation::FileBytes | PathObservationOperation::FileDigest => {
                        self.files().any(|file| file.as_path() == path)
                    }
                    PathObservationOperation::Lstat => {
                        self.files().any(|file| file.as_path().starts_with(path))
                            || self.roots().any(|root| {
                                path.parent() == Some(root.as_path())
                                    && matches!(
                                        path.file_name().and_then(|n| n.to_str()),
                                        Some(".bazelignore" | "BUILD" | "BUILD.bazel")
                                    )
                            })
                    }
                    _ => return Err(deny("operation")),
                };
                if !allowed {
                    return Err(deny("path"));
                }
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct Milestones {
    calls: u64,
    preparations: u64,
    effective_empty: bool,
    source: Option<Weak<[u8]>>,
    additional_source: [Option<Weak<[u8]>>; 4],
    additional_bytes: [Option<(usize, bool)>; 4],
    selected_reported: bool,
    bytes: usize,
    hash_match: bool,
    anchor: Option<&'static str>,
    anchor_outer: Option<Discriminant<ObservedPathFrontierError>>,
    mapping: Option<&'static str>,
    refusal: Option<SelectedSourceRefusal>,
    denied: Option<Denied>,
}
#[derive(Clone)]
struct ModuleRoot {
    bodies: Arc<Bodies>,
    milestones: Arc<Mutex<Milestones>>,
}
#[derive(Clone)]
struct ModuleTerminal {
    mapping: ObservedHostRootRepositoryMapping,
    source: Arc<[u8]>,
    certificate: SourceCertificate,
}
fn refusal() -> NativeDemandSessionError {
    NativeDemandSessionError::Computation(anyhow::anyhow!("bounded module prerequisite refusal"))
}
impl ModuleRoot {
    fn need(
        &self,
        needs: SourcePreparationNeeds,
    ) -> Result<SourcePreparationOutcome<ModuleTerminal>, NativeDemandSessionError> {
        if let Err(denied) = self.bodies.admit(&needs) {
            self.milestones.lock().unwrap().denied = Some(denied);
            return Err(refusal());
        }
        Ok(SourcePreparationOutcome::Need(needs))
    }
}
#[async_trait]
impl NativeCommandRoot for ModuleRoot {
    type Terminal = ModuleTerminal;
    fn initializes_request_revision(&self) -> bool {
        true
    }
    fn allows_empty_terminal(&self) -> bool {
        true
    }
    fn observed_selection_association(&self) -> ObservedSelectionAssociation {
        ObservedSelectionAssociation::SelectedDependencySuperset
    }
    fn event_reconciliation_policy(&self, _: &Self::Terminal) -> EventReconciliationPolicy {
        EventReconciliationPolicy::SourceCertifiedCurrentClosure
    }
    fn observations<'a>(&self, terminal: &'a Self::Terminal) -> Option<&'a PathObservationEpoch> {
        Some(terminal.mapping.observations())
    }
    fn source_certificate<'a>(
        &self,
        terminal: &'a Self::Terminal,
    ) -> Option<&'a SourceCertificate> {
        Some(&terminal.certificate)
    }
    async fn compute(
        &self,
        tx: &mut dice::DiceTransaction,
    ) -> Result<SourcePreparationOutcome<ModuleTerminal>, NativeDemandSessionError> {
        self.milestones.lock().unwrap().calls += 1;
        let policy = tx
            .compute(&RootModuleCommandPolicyKey {
                workspace: self.bodies.roots[0].as_path().to_owned(),
            })
            .await
            .map_err(|_| refusal())?;
        let exact = self.bodies.bindings().all(|(name, _, root)| {
            policy
                .command_module_overrides()
                .any(|(actual, path)| actual == name && path == root.as_path())
        });
        let mode = tx
            .compute(&slug_bzlmod_v2::RootModuleLockfileModeKey {
                workspace: self.bodies.roots[0].as_path().to_owned(),
            })
            .await
            .map_err(|_| refusal())?;
        if !exact || mode != slug_bzlmod_v2::RootModuleLockfileMode::from(LockfileMode::Off) {
            return Err(refusal());
        }
        self.milestones.lock().unwrap().effective_empty = true;
        self.milestones.lock().unwrap().anchor = Some("need");
        let anchor = tx
            .compute(&RootModuleLoadingAnchorObservationKey::new(
                self.bodies.roots[0].dupe(),
            ))
            .await
            .map_err(|_| {
                self.milestones.lock().unwrap().anchor = Some("compute_refusal");
                refusal()
            })?;
        let anchor = match anchor {
            SourcePreparationOutcome::Need(needs) => return self.need(needs),
            SourcePreparationOutcome::Complete(Err(error)) => {
                let mut state = self.milestones.lock().unwrap();
                state.anchor = Some("outer_refusal");
                state.anchor_outer = Some(std::mem::discriminant(&error));
                return Err(refusal());
            }
            SourcePreparationOutcome::Complete(Ok(anchor)) => anchor,
        };
        if anchor.result().is_err() {
            self.milestones.lock().unwrap().anchor = Some("semantic_refusal");
            return Err(refusal());
        }
        let source = anchor
            .observations()
            .observations()
            .iter()
            .find_map(|(demand, value)| {
                if demand.namespace() == PathObservationNamespace::Host
                    && demand.path() == &self.bodies.files[0]
                    && demand.operation() == PathObservationOperation::FileBytes
                {
                    if let PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) =
                        value.as_ref()
                    {
                        if root_bytes(bytes) {
                            return Some(bytes.clone());
                        }
                    }
                }
                None
            });
        let Some(source) = source else {
            self.milestones.lock().unwrap().anchor = Some("coverage_refusal");
            return Err(refusal());
        };
        {
            let mut state = self.milestones.lock().unwrap();
            state.preparations += 1;
            state.source.get_or_insert_with(|| Arc::downgrade(&source));
            state.bytes = source.len();
            state.hash_match = root_bytes(&source);
            state.anchor = Some("complete");
            if !state.hash_match {
                return Err(refusal());
            }
            state.mapping = Some("need");
        }
        let observed = tx
            .compute(&HostRootRepositoryMappingObservationKey::new(
                self.bodies.roots[0].dupe(),
            ))
            .await
            .map_err(|_| refusal())?;
        let mapping = match observed {
            SourcePreparationOutcome::Need(needs) => return self.need(needs),
            SourcePreparationOutcome::Complete(Err(error)) => {
                let mut state = self.milestones.lock().unwrap();
                state.mapping = Some("outer_refusal");
                state.refusal = Some(error.selected_source_refusal());
                return Err(refusal());
            }
            SourcePreparationOutcome::Complete(Ok(mapping)) => mapping,
        };
        for (slot, binding) in self.bodies.additional.iter().enumerate() {
            if let Some(bytes) = binding
                .as_ref()
                .and_then(|binding| observed_bytes(mapping.observations(), &binding.file))
            {
                let mut state = self.milestones.lock().unwrap();
                state.additional_source[slot].get_or_insert_with(|| Arc::downgrade(bytes));
                state.additional_bytes[slot] = Some((bytes.len(), additional_bytes(slot, bytes)));
            }
        }
        if let Err(error) = mapping.result().as_ref() {
            let mut state = self.milestones.lock().unwrap();
            state.mapping = Some("semantic_refusal");
            state.refusal = Some(error.selected_source_refusal());
            if self.bodies.additional[0].is_some() {
                state.selected_reported = true;
                drop(state);
                report_selected(error.selected_module_registry_file());
            }
            return Err(refusal());
        }
        let covered = mapping.observations().observations().iter().any(|(demand, value)| {
            demand.namespace() == PathObservationNamespace::Host && demand.path() == &self.bodies.files[2]
                && demand.operation() == PathObservationOperation::FileBytes
                && matches!(value.as_ref(), PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) if platforms_bytes(bytes))
        });
        let root_covered = mapping.observations().observations().iter().any(|(demand, value)| {
            demand.namespace() == PathObservationNamespace::Host
                && demand.path() == &self.bodies.files[0]
                && demand.operation() == PathObservationOperation::FileBytes
                && matches!(value.as_ref(), PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) if root_bytes(bytes) && Arc::ptr_eq(bytes, &source))
        });
        let additional_covered =
            self.bodies
                .additional
                .iter()
                .enumerate()
                .all(|(slot, binding)| {
                    binding.as_ref().is_none_or(|binding| {
                        observed_bytes(mapping.observations(), &binding.file)
                            .is_some_and(|bytes| additional_bytes(slot, bytes))
                    })
                })
                && (self.bodies.additional[0].is_none()
                    || observed_bytes(mapping.observations(), &self.bodies.files[1]).is_some_and(
                        |bytes| {
                            bytes.len() == 4481
                                && sha2::Sha256::digest(bytes).as_slice() == RULES_SHA
                        },
                    ));
        if !covered || !root_covered || !additional_covered {
            self.milestones.lock().unwrap().mapping = Some("coverage_refusal");
            return Err(refusal());
        }
        let certificate =
            SourceCertificate::from_epoch(mapping.observations().dupe()).map_err(|_| {
                self.milestones.lock().unwrap().mapping = Some("certificate_refusal");
                refusal()
            })?;
        self.milestones.lock().unwrap().mapping = Some("complete");
        Ok(SourcePreparationOutcome::Complete(ModuleTerminal {
            mapping,
            source,
            certificate,
        }))
    }
}

fn observed_bytes<'a>(
    epoch: &'a PathObservationEpoch,
    path: &NormalizedAbsolutePath,
) -> Option<&'a Arc<[u8]>> {
    epoch.observations().iter().find_map(|(demand, value)| {
        if demand.namespace() == PathObservationNamespace::Host
            && demand.path() == path
            && demand.operation() == PathObservationOperation::FileBytes
        {
            if let PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) =
                value.as_ref()
            {
                return Some(bytes);
            }
        }
        None
    })
}
fn report_selected(selected: Option<(&str, &str, &slug_bzlmod_v2::RegistryFileUrl)>) {
    let Some((name, version, url)) = selected else {
        println!(
            "WP945_SELECTED capture=missing name_len=none name_hex=none version_len=none version_hex=none url_len=none url_hex=none"
        );
        return;
    };
    let values = [name.as_bytes(), version.as_bytes(), url.as_str().as_bytes()];
    let captured = values
        .iter()
        .all(|value| value.len() <= REGISTRY_IDENTITY_BYTES);
    println!(
        "WP945_SELECTED capture={} name_len={} name_hex={} version_len={} version_hex={} url_len={} url_hex={}",
        if captured {
            "captured"
        } else {
            "bounds_refused"
        },
        values[0].len(),
        RegistryHex(captured.then_some(values[0])),
        values[1].len(),
        RegistryHex(captured.then_some(values[1])),
        values[2].len(),
        RegistryHex(captured.then_some(values[2]))
    );
}

#[test]
fn complete_need_admission_refuses_mixed_and_wrong_operation_before_progress() {
    let normalized = |path: &str| NormalizedAbsolutePath::new(Path::new(path)).unwrap();
    let bodies = Bodies::new([
        normalized("/fixture"),
        normalized("/rules"),
        normalized("/platforms"),
    ]);
    let paths = |path: &str, operation| {
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Host,
            normalized(path),
            operation,
        )))
    };
    let body = paths(
        "/platforms/MODULE.bazel",
        PathObservationOperation::FileBytes,
    );
    assert!(bodies.admit(&body).is_ok());
    for path in [
        "/",
        "/platforms",
        "/platforms/MODULE.bazel",
        "/rules/BUILD.bazel",
        "/fixture/.bazelignore",
    ] {
        assert!(
            bodies
                .admit(&paths(path, PathObservationOperation::Lstat))
                .is_ok()
        );
    }
    assert!(
        bodies
            .admit(&paths(
                "/rules/MODULE.bazel",
                PathObservationOperation::FileDigest
            ))
            .is_ok()
    );
    let mut progress_calls = 0;
    for excluded in [
        paths("/platforms/REPO.bazel", PathObservationOperation::Lstat),
        paths(
            "/platforms/BUILD.bazel",
            PathObservationOperation::FileBytes,
        ),
        paths(
            "/platforms/MODULE.bazel",
            PathObservationOperation::ReadLink,
        ),
        paths("/platforms", PathObservationOperation::DirectoryEntries),
        paths(
            "/platforms/MODULE.bazel",
            PathObservationOperation::FileBytes,
        )
        .try_union(&SourcePreparationNeeds::root_module_bootstrap(
            slug_bzlmod_v2::RootModuleBootstrapRequest {
                workspace: normalized("/other"),
            },
        ))
        .unwrap(),
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Materialization(
                slug_workspace_v2::PathObservationInstanceId::new(1),
            ),
            normalized("/platforms/MODULE.bazel"),
            PathObservationOperation::FileBytes,
        ))),
    ] {
        let mixed = body.try_union(&excluded).unwrap();
        if bodies.admit(&mixed).is_ok() {
            progress_calls += 1;
        }
        assert!(bodies.admit(&mixed).is_err());
    }
    let mut local = (*local_native_request(&bodies.roots[0], "platforms+", "/platforms")).clone();
    assert!(
        bodies
            .admit(&SourcePreparationNeeds::repository(local.clone()))
            .is_ok()
    );
    local.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new("platforms").unwrap();
    assert!(
        bodies
            .admit(
                &body
                    .try_union(&SourcePreparationNeeds::repository(local.clone()))
                    .unwrap()
            )
            .is_err()
    );
    local.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new("platforms+").unwrap();
    local.kind = RepositoryMaterializationKind::Immutable;
    assert!(
        bodies
            .admit(&SourcePreparationNeeds::repository(local.clone()))
            .is_err()
    );
    local.kind = RepositoryMaterializationKind::Local {
        logical_root: normalized("/other"),
    };
    assert!(
        bodies
            .admit(&SourcePreparationNeeds::repository(local.clone()))
            .is_err()
    );
    local.kind = RepositoryMaterializationKind::Local {
        logical_root: normalized("/platforms"),
    };
    local.id.workspace = normalized("/other");
    assert!(
        bodies
            .admit(&SourcePreparationNeeds::repository(local.clone()))
            .is_err()
    );
    local.id.workspace = normalized("/fixture");
    local.repo_spec.rule_id.rule_name = "http_archive".into();
    assert!(
        bodies
            .admit(&SourcePreparationNeeds::repository(local))
            .is_err()
    );
    let budget = slug_workspace_v2::ImageDescriptorBudget::new(4096);
    let anchor =
        slug_workspace_v2::ImageChargedPath::pin_existing(normalized("/platforms"), &budget)
            .unwrap();
    let leaf = slug_workspace_v2::ImageChargedPath::pin_existing(
        normalized("/platforms/MODULE.bazel"),
        &budget,
    )
    .unwrap();
    let image = PathObservationDemand::input_image(
        PathObservationNamespace::Host,
        leaf,
        slug_workspace_v2::ImageObservationRequest::new(
            anchor,
            slug_workspace_v2::ImageObservationOperation::Lstat,
        ),
    )
    .unwrap();
    assert!(
        bodies
            .admit(
                &body
                    .try_union(&SourcePreparationNeeds::path(
                        NeedPathObservations::singleton(image)
                    ))
                    .unwrap()
            )
            .is_err()
    );
    assert_eq!(progress_calls, 0);
    assert!(
        bodies
            .admit(&SourcePreparationNeeds::root_module_bootstrap(
                slug_bzlmod_v2::RootModuleBootstrapRequest {
                    workspace: normalized("/fixture")
                }
            ))
            .is_ok()
    );
}

#[test]
fn fourth_module_need_admission_is_opt_in_and_exact() {
    let normalized = |path: &str| NormalizedAbsolutePath::new(Path::new(path)).unwrap();
    let bodies = Bodies::new([
        normalized("/fixture"),
        normalized("/rules"),
        normalized("/platforms"),
    ]);
    let need = |path: &str, operation| {
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Host,
            normalized(path),
            operation,
        )))
    };
    let body = need(
        "/features/MODULE.bazel",
        PathObservationOperation::FileBytes,
    );
    let local = (*local_native_request(&bodies.roots[0], "bazel_features+", "/features")).clone();
    assert!(bodies.admit(&body).is_err());
    assert!(
        bodies
            .admit(&SourcePreparationNeeds::repository(local.clone()))
            .is_err()
    );
    let bodies = bodies.with_fourth(normalized("/features"));
    assert!(bodies.admit(&body).is_ok());
    assert!(
        bodies
            .admit(&need(
                "/features/MODULE.bazel",
                PathObservationOperation::FileDigest
            ))
            .is_ok()
    );
    for path in [
        "/features",
        "/features/MODULE.bazel",
        "/features/BUILD",
        "/features/BUILD.bazel",
        "/features/.bazelignore",
    ] {
        assert!(
            bodies
                .admit(&need(path, PathObservationOperation::Lstat))
                .is_ok()
        );
    }
    assert!(
        bodies
            .admit(&SourcePreparationNeeds::repository(local.clone()))
            .is_ok()
    );
    let original = need("/fixture/MODULE.bazel", PathObservationOperation::FileBytes);
    let foreign =
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Materialization(
                slug_workspace_v2::PathObservationInstanceId::new(2),
            ),
            normalized("/features/MODULE.bazel"),
            PathObservationOperation::FileBytes,
        )));
    let mut progress_calls = 0;
    for excluded in [
        need(
            "/features_extra/MODULE.bazel",
            PathObservationOperation::FileBytes,
        ),
        need("/features/BUILD.bazel", PathObservationOperation::FileBytes),
        need("/features/REPO.bazel", PathObservationOperation::Lstat),
        need("/features/MODULE.bazel", PathObservationOperation::ReadLink),
        need("/features", PathObservationOperation::DirectoryEntries),
        foreign,
    ] {
        let mixed = original.try_union(&excluded).unwrap();
        if bodies.admit(&mixed).is_ok() {
            progress_calls += 1;
        }
        assert!(bodies.admit(&mixed).is_err());
    }
    for case in 0..7 {
        let mut wrong = local.clone();
        match case {
            0 => {
                wrong.id.canonical_repo =
                    slug_identity_v2::CanonicalRepoName::new("bazel_features").unwrap()
            }
            1 => wrong.id.workspace = normalized("/other"),
            2 => wrong.kind = RepositoryMaterializationKind::Immutable,
            3 => {
                wrong.kind = RepositoryMaterializationKind::Local {
                    logical_root: normalized("/other"),
                }
            }
            4 => wrong.repo_spec.rule_id.rule_name = "http_archive".into(),
            5 => {
                Arc::make_mut(&mut wrong.repo_spec.attributes).insert(
                    "path".into(),
                    OverrideAttributeValue::String("/other".into()),
                );
            }
            6 => {
                wrong.repo_spec.rule_id.bzl_file =
                    CanonicalLabel::parse("@@other//tools/build_defs/repo:local.bzl").unwrap()
            }
            _ => unreachable!(),
        }
        let mixed = original
            .try_union(&SourcePreparationNeeds::repository(wrong))
            .unwrap();
        if bodies.admit(&mixed).is_ok() {
            progress_calls += 1;
        }
        assert!(bodies.admit(&mixed).is_err());
    }
    assert_eq!(progress_calls, 0);
}

#[test]
fn fifth_module_need_admission_is_opt_in_and_exact() {
    additional_module_need_admission(ModuleMode::Five);
}

#[test]
fn sixth_module_need_admission_is_opt_in_and_exact() {
    additional_module_need_admission(ModuleMode::Six);
}

#[test]
fn seventh_module_need_admission_is_opt_in_and_exact() {
    additional_module_need_admission(ModuleMode::Seven);
}

fn additional_module_need_admission(mode: ModuleMode) {
    let slot = match mode {
        ModuleMode::Five => 1,
        ModuleMode::Six => 2,
        ModuleMode::Seven => 3,
        _ => unreachable!(),
    };
    let roots = ["/features", "/skylib", "/cc", "/license"];
    let paths = match mode {
        ModuleMode::Five => [
            "/",
            "/skylib",
            "/skylib/MODULE.bazel",
            "/skylib/BUILD",
            "/skylib/BUILD.bazel",
            "/skylib/.bazelignore",
        ],
        ModuleMode::Six => [
            "/",
            "/cc",
            "/cc/MODULE.bazel",
            "/cc/BUILD",
            "/cc/BUILD.bazel",
            "/cc/.bazelignore",
        ],
        ModuleMode::Seven => [
            "/",
            "/license",
            "/license/MODULE.bazel",
            "/license/BUILD",
            "/license/BUILD.bazel",
            "/license/.bazelignore",
        ],
        _ => unreachable!(),
    };
    let excluded_paths = match mode {
        ModuleMode::Five => [
            "/skylib_extra/MODULE.bazel",
            "/skylib/BUILD.bazel",
            "/skylib/REPO.bazel",
            "/skylib/MODULE.bazel",
        ],
        ModuleMode::Six => [
            "/cc_extra/MODULE.bazel",
            "/cc/BUILD.bazel",
            "/cc/REPO.bazel",
            "/cc/MODULE.bazel",
        ],
        ModuleMode::Seven => [
            "/license_extra/MODULE.bazel",
            "/license/BUILD.bazel",
            "/license/REPO.bazel",
            "/license/MODULE.bazel",
        ],
        _ => unreachable!(),
    };
    let (name, canonical) = ADDITIONAL_IDENTITIES[slot];
    let normalized = |path: &str| NormalizedAbsolutePath::new(Path::new(path)).unwrap();
    let base = || {
        Bodies::new([
            normalized("/fixture"),
            normalized("/rules"),
            normalized("/platforms"),
        ])
    };
    let need = |path: &str, operation| {
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Host,
            normalized(path),
            operation,
        )))
    };
    let configured = |count| {
        let mut bodies = base();
        if count >= 1 {
            bodies = bodies.with_fourth(normalized(roots[0]));
        }
        if count >= 2 {
            bodies = bodies.with_fifth(normalized(roots[1]));
        }
        if count >= 3 {
            bodies = bodies.with_sixth(normalized(roots[2]));
        }
        if count >= 4 {
            bodies = bodies.with_seventh(normalized(roots[3]));
        }
        bodies
    };
    let body = need(paths[2], PathObservationOperation::FileBytes);
    let local = (*local_native_request(&normalized("/fixture"), canonical, roots[slot])).clone();
    for count in 0..=slot {
        let bodies = configured(count);
        assert!(bodies.admit(&body).is_err());
        assert!(
            bodies
                .admit(&SourcePreparationNeeds::repository(local.clone()))
                .is_err()
        );
    }
    for count in 0..slot {
        assert!(
            std::panic::catch_unwind(|| match mode {
                ModuleMode::Five => configured(count).with_fifth(normalized(roots[slot])),
                ModuleMode::Six => configured(count).with_sixth(normalized(roots[slot])),
                ModuleMode::Seven => configured(count).with_seventh(normalized(roots[slot])),
                _ => unreachable!(),
            })
            .is_err()
        );
    }
    let bodies = configured(slot + 1);
    assert!(bodies.admit(&body).is_ok());
    assert!(
        bodies
            .admit(&need(paths[2], PathObservationOperation::FileDigest))
            .is_ok()
    );
    for path in paths {
        assert!(
            bodies
                .admit(&need(path, PathObservationOperation::Lstat))
                .is_ok()
        );
    }
    assert!(
        bodies
            .admit(&SourcePreparationNeeds::repository(local.clone()))
            .is_ok()
    );
    let original = need("/fixture/MODULE.bazel", PathObservationOperation::FileBytes);
    for excluded in [
        need(excluded_paths[0], PathObservationOperation::FileBytes),
        need(excluded_paths[1], PathObservationOperation::FileBytes),
        need(excluded_paths[2], PathObservationOperation::Lstat),
        need(excluded_paths[3], PathObservationOperation::ReadLink),
    ] {
        assert!(
            bodies
                .admit(&original.try_union(&excluded).unwrap())
                .is_err()
        );
    }
    let reject_local = |wrong| {
        assert!(
            bodies
                .admit(
                    &original
                        .try_union(&SourcePreparationNeeds::repository(wrong))
                        .unwrap()
                )
                .is_err()
        );
    };
    for case in [0, 1, 2, 3, 4, 5, 6, 9] {
        let mut wrong = local.clone();
        match case {
            0 => wrong.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new(name).unwrap(),
            1 => wrong.id.workspace = normalized("/other"),
            2 => wrong.kind = RepositoryMaterializationKind::Immutable,
            3 => {
                let path = "/other";
                wrong.kind = RepositoryMaterializationKind::Local {
                    logical_root: normalized(path),
                };
                Arc::make_mut(&mut wrong.repo_spec.attributes)
                    .insert("path".into(), OverrideAttributeValue::String(path.into()));
            }
            4 => wrong.repo_spec.rule_id.rule_name = "http_archive".into(),
            5 => {
                Arc::make_mut(&mut wrong.repo_spec.attributes).insert(
                    "path".into(),
                    OverrideAttributeValue::String("/other".into()),
                );
            }
            6 => {
                wrong.repo_spec.rule_id.bzl_file =
                    CanonicalLabel::parse("@@other//tools/build_defs/repo:local.bzl").unwrap()
            }
            9 => {
                Arc::make_mut(&mut wrong.repo_spec.attributes)
                    .insert("extra".into(), OverrideAttributeValue::String("x".into()));
            }
            _ => unreachable!(),
        }
        reject_local(wrong);
    }
    for prior in 0..slot {
        let mut wrong = local.clone();
        wrong.id.canonical_repo =
            slug_identity_v2::CanonicalRepoName::new(ADDITIONAL_IDENTITIES[prior].1).unwrap();
        reject_local(wrong);
        let mut wrong = local.clone();
        wrong.kind = RepositoryMaterializationKind::Local {
            logical_root: normalized(roots[prior]),
        };
        Arc::make_mut(&mut wrong.repo_spec.attributes).insert(
            "path".into(),
            OverrideAttributeValue::String(roots[prior].into()),
        );
        reject_local(wrong);
    }
}

#[test]
#[ignore = "frozen scoped native MODULE preparation observation"]
fn native_root_module_and_normal_mapping_prerequisite() {
    observe_native_modules(ModuleMode::Three);
}

#[test]
#[ignore = "frozen scoped four-root native MODULE readiness observation"]
fn native_four_root_local_module_and_mapping_prerequisite() {
    observe_native_modules(ModuleMode::Four);
}

#[test]
#[ignore = "frozen scoped five-root native MODULE readiness observation"]
fn native_five_root_local_module_and_mapping_prerequisite() {
    observe_native_modules(ModuleMode::Five);
}

#[test]
#[ignore = "frozen scoped six-root explicit local MODULE readiness observation"]
fn native_six_root_local_module_and_mapping_prerequisite() {
    observe_native_modules(ModuleMode::Six);
}

#[test]
#[ignore = "frozen scoped seven-root normal Local MODULE readiness observation"]
fn native_seven_root_local_module_and_mapping_prerequisite() {
    observe_native_modules(ModuleMode::Seven);
}

fn observe_native_modules(mode: ModuleMode) {
    phase("baseline");
    let workspace = NormalizedAbsolutePath::new(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/v2_oracle/fixtures/rules-rust-073-toolchain-owner/workspace"),
    )
    .unwrap();
    let root = |name| {
        let spelling = std::env::var(name).expect("frozen supervisor root");
        assert!(spelling.len() <= 1024 && !spelling.chars().any(char::is_whitespace));
        let normalized = NormalizedAbsolutePath::new(Path::new(&spelling)).unwrap();
        assert_eq!(normalized.as_path(), Path::new(&spelling));
        normalized
    };
    let mut bodies = Bodies::new([
        workspace.clone(),
        root("SLUG_NATIVE_RULES_RUST_ROOT"),
        root("SLUG_NATIVE_PLATFORMS_ROOT"),
    ]);
    for slot in 0..mode.additional_count() {
        let extra_root = root(ADDITIONAL_ENV[slot]);
        bodies = match slot {
            0 => bodies.with_fourth(extra_root),
            1 => bodies.with_fifth(extra_root),
            2 => bodies.with_sixth(extra_root),
            3 => bodies.with_seventh(extra_root),
            _ => unreachable!(),
        };
        let (prefix, field) = ADDITIONAL_MARKERS[slot];
        println!(
            "{prefix}_REQUEST {field}={}",
            bodies.additional[slot]
                .as_ref()
                .unwrap()
                .root
                .as_path()
                .display()
        );
    }
    let bodies = Arc::new(bodies);
    println!(
        "WP942_REQUEST workspace={} rules_root={} platforms_root={} registry=https://bcr.bazel.build/ lockfile=off local_overrides=true",
        workspace.as_path().display(),
        bodies.roots[1].as_path().display(),
        bodies.roots[2].as_path().display()
    );
    let start = Instant::now();
    let scope = SourceServiceScope::new_for_test(
        workspace.clone(),
        start + Duration::from_secs(5),
        start + Duration::from_secs(8),
        SourceServiceLimits {
            allow_network: false,
            registry_object_bytes: 1 << 20,
            registry_attempts: 16,
            registry_admitted_bytes: 16 << 20,
            source_attempts: 16,
            transferred_bytes: 16 << 20,
            decoded_bytes: 16 << 20,
            written_bytes: 16 << 20,
            live_buffer_bytes: 16 << 20,
            blocking_threads: Some(2),
        },
    )
    .unwrap();
    let scope_weak = Arc::downgrade(&scope);
    let (recorder, arm_error) = match scope.arm_registry_association_for_test() {
        Ok(r) => (Some(r), None),
        Err(e) => (None, Some(e)),
    };
    let recorder_weak = recorder.as_ref().map(Arc::downgrade);
    let (mut runtime, constructor_error) = if arm_error.is_none() {
        match WorkspaceRuntime::new_with_source_services(
            workspace.as_path(),
            ProcessHostOwner::native(),
            WorkspaceSourceServices::Diagnostic(scope.clone()),
        ) {
            Ok(r) => (Some(r), None),
            Err(e) => (None, Some(e)),
        }
    } else {
        (None, None)
    };
    let budgets = runtime.as_mut().map(|runtime| {
        let descriptor = slug_workspace_v2::ImageDescriptorBudget::new(16 << 20);
        runtime.image_resources =
            crate::runtime::input_image::WorkspaceImageResources::new(descriptor.clone(), 16 << 20);
        runtime.scratch_resources = slug_bzlmod_v2::SelectedScratchResources::new(
            slug_bzlmod_v2::SelectedScratchPayloadBudget::new(16 << 20),
            descriptor,
        );
        assert!(
            runtime
                .image_resources
                .descriptor_budget()
                .same_instance(runtime.scratch_resources.descriptor_budget())
        );
        (
            runtime.image_resources.clone(),
            runtime.scratch_resources.clone(),
        )
    });
    phase("after_runtime");
    resources("before_command", budgets.as_ref());
    let milestones = Arc::new(Mutex::new(Milestones::default()));
    let mut policy_error = false;
    let mut outcome = None;
    if let Some(runtime) = &runtime {
        let mut overrides: [Option<String>; 6] = Default::default();
        for (slot, (name, _, root)) in bodies.bindings().enumerate() {
            overrides[slot] = Some(format!("{name}={}", root.as_path().display()));
        }
        match BzlmodCommandPolicyKey::from_flags_with_module_overrides(
            None,
            false,
            workspace.as_path(),
            overrides.iter().filter_map(Option::as_deref),
        ) {
            Err(_) => policy_error = true,
            Ok(policy) => {
                let mut request = NativeDemandRequestInputBundle::normalized_initial();
                request.command_policy = policy;
                request.lockfile_mode = LockfileMode::Off;
                request.registry_urls = RegistryUrls::new(["https://bcr.bazel.build/"]);
                outcome = Some(runtime.drive_command(
                    request,
                    ModuleRoot {
                        bodies,
                        milestones: milestones.clone(),
                    },
                ));
            }
        }
    }
    phase("after_command");
    let stage = if arm_error.is_some() {
        "arm_refusal"
    } else if constructor_error.is_some() {
        "constructor_refusal"
    } else if policy_error {
        "policy_refusal"
    } else if matches!(outcome, Some(Ok(_))) {
        "mapping_complete"
    } else {
        "native_refusal"
    };
    println!("WP942_STAGE {stage}");
    match &outcome {
        Some(Ok(driven)) => {
            assert!(root_bytes(&driven.accepted.terminal().source));
            println!(
                "WP942_NATIVE kind=complete attempts={} roots={}",
                driven.attempts, driven.terminal_root_count
            );
        }
        Some(Err(error)) => println!(
            "WP942_NATIVE kind={} attempts=unavailable roots=unavailable",
            native_kind(error)
        ),
        None => println!("WP942_NATIVE kind=unavailable attempts=unavailable roots=unavailable"),
    }
    let (source_weak, additional_weak) = {
        let state = milestones.lock().unwrap();
        for (slot, (prefix, _)) in ADDITIONAL_MARKERS
            .into_iter()
            .take(mode.additional_count())
            .enumerate()
        {
            match state.additional_bytes[slot] {
                Some((bytes, hash_match)) => {
                    println!("{prefix}_SOURCE seen=true bytes={bytes} hash_match={hash_match}")
                }
                None => {
                    println!("{prefix}_SOURCE seen=false bytes=unavailable hash_match=unavailable")
                }
            }
        }
        if mode.additional_count() >= 1 && !state.selected_reported {
            report_selected(None);
        }
        println!(
            "WP942_ROOT_SOURCE seen={} calls={} policy_checked={} bytes={} hash_match={} anchor={} outer_variant={:?}",
            state.preparations != 0,
            state.calls,
            state.effective_empty,
            state.bytes,
            state.hash_match,
            state.anchor.unwrap_or("not_started"),
            state.anchor_outer
        );
        print!(
            "WP942_MAPPING state={} codes=",
            state.mapping.unwrap_or("not_started")
        );
        if let Some(refusal) = state.refusal {
            if refusal.codes().is_empty() {
                print!("none");
            }
            for (i, code) in refusal.codes().iter().enumerate() {
                if i != 0 {
                    print!(",");
                }
                print!("{code:?}");
            }
            println!(
                " termination={:?} native_detail={}",
                refusal.termination(),
                refusal.native_detail_observed()
            );
        } else {
            println!("none termination=none native_detail=false");
        }
        let bytes = state
            .denied
            .as_ref()
            .and_then(|d| d.path.as_ref())
            .map(|path| path.as_path().as_os_str().as_bytes());
        let captured = bytes.is_some_and(|bytes| bytes.len() <= REGISTRY_IDENTITY_BYTES);
        println!(
            "WP942_DENIED family={} path_len={} path_hex={} capture={}",
            state.denied.as_ref().map_or("none", |d| d.family),
            bytes.map_or_else(|| "none".to_owned(), |b| b.len().to_string()),
            RegistryHex(bytes.filter(|_| captured)),
            if captured {
                "captured"
            } else if bytes.is_some() {
                "bounds_refused"
            } else {
                "missing"
            }
        );
        (state.source.clone(), state.additional_source.clone())
    };
    let counters = scope.snapshot();
    println!(
        "WP938_SCOPE {:?} {} {} {} {} {} {} {} {}",
        scope.first_error_for_test(),
        counters.registry_attempts,
        counters.registry_admitted_bytes,
        counters.source_attempts,
        counters.transferred_bytes,
        counters.decoded_bytes,
        counters.written_bytes,
        counters.live_buffer_bytes,
        counters.peak_buffer_bytes
    );
    println!("WP940_ARM {arm_error:?}");
    if let Some(recorder) = &recorder {
        report_registry_association(recorder, &counters, scope.first_error_for_test());
    } else {
        println!("WP940_ASSOCIATION unavailable");
        println!("WP940_FIRST_ROW unavailable");
    }
    resources("after_command", budgets.as_ref());
    drop(outcome);
    drop(constructor_error);
    drop(milestones);
    phase("after_terminal_drop");
    drop(runtime);
    phase("after_runtime_drop");
    resources("after_runtime_drop", budgets.as_ref());
    drop(recorder);
    let deadline = Instant::now() + Duration::from_secs(2);
    while (budgets.as_ref().is_some_and(|(image, scratch)| {
        image.descriptor_budget().used_bytes() != 0
            || image.logical_bytes_used() != 0
            || scratch.payload_budget().used_bytes() != 0
    }) || scope.snapshot().live_buffer_bytes != 0
        || source_weak
            .as_ref()
            .is_some_and(|weak| weak.upgrade().is_some())
        || additional_weak
            .iter()
            .flatten()
            .any(|weak| weak.upgrade().is_some())
        || recorder_weak
            .as_ref()
            .is_some_and(|weak| weak.upgrade().is_some()))
        && Instant::now() < deadline
    {
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);
    assert!(
        source_weak
            .as_ref()
            .is_none_or(|weak| weak.upgrade().is_none())
    );
    assert!(
        recorder_weak
            .as_ref()
            .is_none_or(|weak| weak.upgrade().is_none())
    );
    assert!(
        additional_weak
            .iter()
            .flatten()
            .all(|weak| weak.upgrade().is_none())
    );
    if let Some((image, scratch)) = &budgets {
        assert_eq!(image.descriptor_budget().used_bytes(), 0);
        assert_eq!(image.logical_bytes_used(), 0);
        assert_eq!(scratch.payload_budget().used_bytes(), 0);
    }
    drop(scope);
    while scope_weak.upgrade().is_some() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(1));
    }
    assert!(scope_weak.upgrade().is_none());
    phase("after_scope_drop");
    println!(
        "WP942_RETIREMENT source_weak_seen={} source_weak_expired=true live_buffer_zero=true scope_weak_expired=true recorder_weak_expired=true",
        source_weak.is_some()
    );
    for (slot, (prefix, _)) in ADDITIONAL_MARKERS
        .into_iter()
        .take(mode.additional_count())
        .enumerate()
    {
        println!(
            "{prefix}_RETIREMENT source_weak_seen={} source_weak_expired=true",
            additional_weak[slot].is_some()
        );
    }
}
