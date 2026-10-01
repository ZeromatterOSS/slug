//! Preparation of the one source-owned Rust allocator helper. Code is phase
//! memory; only its complete loading authority enters retained analysis values.
use slug_build_api_v2::RustBzlModuleIdentity;
use slug_build_api_v2::RustBzlModuleSource;
use slug_build_api_v2::RustToolchainCallableSourceAuthority;
use slug_build_api_v2::RustToolchainHelperManifest;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;
use starlark::values::OwnedFrozenValue;
use starlark::values::ValueLike;
use starlark::values::function::StarlarkFunctionIdentity;

use super::*;

pub(crate) const CALLER_LABEL: &str = "@@rules_rust+//rust/private:toolchain.bzl";
pub(crate) const CALLER_SHA: [u8; 32] = [
    0xc4, 0xb6, 0x13, 0xce, 0xe9, 0x65, 0x40, 0xa9, 0x4f, 0xbd, 0xf4, 0xfb, 0xdc, 0xa7, 0xb8, 0xdc,
    0x4e, 0xf6, 0xd3, 0x08, 0x20, 0x24, 0xc4, 0xd3, 0x63, 0x6a, 0xfc, 0x2e, 0x9c, 0x4d, 0x46, 0x8e,
];
pub(crate) const HELPER_LABEL: &str = "@@rules_rust+//rust/private:rust_allocator_libraries.bzl";
pub(crate) const HELPER_SHA: [u8; 32] = [
    0xae, 0x4a, 0xcb, 0x50, 0xac, 0x6a, 0x1b, 0x92, 0x22, 0x54, 0xa0, 0x73, 0x46, 0xd9, 0x7b, 0x46,
    0x49, 0x81, 0x0d, 0x33, 0x83, 0x6f, 0x2b, 0xe4, 0x82, 0x4f, 0xd0, 0xb7, 0xa8, 0x1e, 0x53, 0x6e,
];
const EXPORT: &str = "make_libstd_and_allocator_ccinfo";

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RustToolchainHelperRequest {
    Producer(BzlModuleSourceProvenance),
    Retained(Arc<RustToolchainCallableSourceAuthority>),
}

#[derive(Debug, Clone, Copy)]
pub enum RustToolchainHelperMode {
    Legacy,
    Observed,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RustToolchainHelperError {
    Authentication(Arc<str>),
    Compute(Arc<str>),
    Route(HostCanonicalRepositoryLoadRouteError),
    RouteObservation(crate::HostCanonicalRepositoryLoadRouteObservationError),
    Loading {
        label: CanonicalLabel,
        message: Arc<str>,
    },
    Export(Arc<str>),
    Provenance(Arc<str>),
}

impl fmt::Display for RustToolchainHelperError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rust toolchain helper preparation: {self:?}")
    }
}
impl std::error::Error for RustToolchainHelperError {}

#[derive(Debug, Clone, Dupe, Allocative)]
pub struct PreparedRustToolchainHelperBinding(Arc<PreparedBinding>);

#[derive(Debug, Allocative)]
struct PreparedBinding {
    module: FrozenBzlModule,
    #[allocative(skip)]
    export: OwnedFrozenValue,
    authority: Arc<RustToolchainCallableSourceAuthority>,
    sources: Arc<[(CompactString, BzlModuleSourceProvenance)]>,
}

impl PreparedRustToolchainHelperBinding {
    pub fn authority(&self) -> &Arc<RustToolchainCallableSourceAuthority> {
        &self.0.authority
    }

    pub fn install(&self, heap: &FrozenHeap) -> FrozenValue {
        heap.add_reference(self.0.module.module.frozen_heap());
        heap.alloc(self.0.export.dupe())
    }

    pub fn source_identities(&self) -> &Arc<[(CompactString, BzlModuleSourceProvenance)]> {
        &self.0.sources
    }
}

pub type RustToolchainHelperPreparationOutcome = SourcePreparationOutcome<
    Result<
        (
            Result<PreparedRustToolchainHelperBinding, RustToolchainHelperError>,
            PathObservationEpoch,
        ),
        ObservedPathFrontierError,
    >,
>;

/// Uses the canonical repository's own selected mapping in both loader modes.
/// Public request provenance is checked against freshly loaded caller metadata.
pub async fn prepare_rust_toolchain_helper(
    ctx: &mut DiceComputations<'_>,
    workspace: NormalizedAbsolutePath,
    request: RustToolchainHelperRequest,
    mode: RustToolchainHelperMode,
) -> RustToolchainHelperPreparationOutcome {
    let expected = request_caller(&request);
    if let Err(error) = authenticate_source(&expected, CALLER_LABEL, CALLER_SHA) {
        return complete(Err(error), PathObservationEpoch::empty());
    }
    let repo = expected.identity.label.package().repo().clone();
    let (input, observations) = match mode {
        RustToolchainHelperMode::Legacy => {
            match ctx
                .compute(&HostCanonicalRepositoryLoadRouteKey::new(workspace, repo))
                .await
            {
                Err(error) => {
                    return complete(
                        Err(RustToolchainHelperError::Compute(
                            format!("{error:?}").into(),
                        )),
                        PathObservationEpoch::empty(),
                    );
                }
                Ok(SourcePreparationOutcome::Need(need)) => {
                    return SourcePreparationOutcome::Need(need);
                }
                Ok(SourcePreparationOutcome::Complete(result)) => match result.as_ref() {
                    Ok(route) => (route.input().clone(), PathObservationEpoch::empty()),
                    Err(error) => {
                        return complete(
                            Err(RustToolchainHelperError::Route(error.clone())),
                            PathObservationEpoch::empty(),
                        );
                    }
                },
            }
        }
        RustToolchainHelperMode::Observed => {
            match ctx
                .compute(&HostCanonicalRepositoryLoadRouteObservationKey::new(
                    workspace, repo,
                ))
                .await
            {
                Err(error) => {
                    return complete(
                        Err(RustToolchainHelperError::Compute(
                            format!("{error:?}").into(),
                        )),
                        PathObservationEpoch::empty(),
                    );
                }
                Ok(SourcePreparationOutcome::Need(need)) => {
                    return SourcePreparationOutcome::Need(need);
                }
                Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                    return match error.selected_frontier() {
                        HostSelectedObservationFrontier::Path(error) => {
                            SourcePreparationOutcome::Complete(Err(error))
                        }
                        HostSelectedObservationFrontier::Infrastructure(_) => complete(
                            Err(RustToolchainHelperError::RouteObservation(error)),
                            PathObservationEpoch::empty(),
                        ),
                    };
                }
                Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                    match observed.result().as_ref() {
                        Ok(route) => (route.input().clone(), observed.observations().dupe()),
                        Err(error) => {
                            return complete(
                                Err(RustToolchainHelperError::Route(error.clone())),
                                observed.observations().dupe(),
                            );
                        }
                    }
                }
            }
        }
    };
    let (caller, observations) =
        match load_module(ctx, input.clone(), CALLER_LABEL, mode, observations).await {
            SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                return SourcePreparationOutcome::Complete(Err(error));
            }
            SourcePreparationOutcome::Complete(Ok((Err(error), observations))) => {
                return complete(Err(error), observations);
            }
            SourcePreparationOutcome::Complete(Ok((Ok(module), observations))) => {
                (module, observations)
            }
        };
    if root_source(&caller.manifest) != Some(&expected) {
        return complete(Err(auth("caller provenance changed")), observations);
    }
    let (helper, observations) =
        match load_module(ctx, input, HELPER_LABEL, mode, observations).await {
            SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                return SourcePreparationOutcome::Complete(Err(error));
            }
            SourcePreparationOutcome::Complete(Ok((Err(error), observations))) => {
                return complete(Err(error), observations);
            }
            SourcePreparationOutcome::Complete(Ok((Ok(module), observations))) => {
                (module, observations)
            }
        };
    complete(
        finish_binding(&request, &caller.manifest, helper),
        observations,
    )
}

type ModuleOutcome = SourcePreparationOutcome<
    Result<
        (
            Result<FrozenBzlModule, RustToolchainHelperError>,
            PathObservationEpoch,
        ),
        ObservedPathFrontierError,
    >,
>;

async fn load_module(
    ctx: &mut DiceComputations<'_>,
    input: slug_bzlmod_v2::HostCanonicalRepositorySourceInput,
    label: &str,
    mode: RustToolchainHelperMode,
    observations: PathObservationEpoch,
) -> ModuleOutcome {
    let canonical = CanonicalLabel::parse(label).expect("fixed helper label");
    let label = RepositoryBzlLabel::new(
        canonical.package().package().clone(),
        RootPackageBzlTarget::parse(canonical.target().as_str()).expect("fixed bzl basename"),
    )
    .expect("fixed bzl label");
    let (result, observations) = match mode {
        RustToolchainHelperMode::Legacy => match ctx
            .compute(&ExternalBzlModuleEvalKey::new_canonical(input, label))
            .await
        {
            Err(error) => {
                return SourcePreparationOutcome::Complete(Ok((
                    Err(RustToolchainHelperError::Compute(
                        format!("{error:?}").into(),
                    )),
                    observations,
                )));
            }
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(result)) => (result, observations),
        },
        RustToolchainHelperMode::Observed => match ctx
            .compute(&ExternalBzlModuleObservationKey::new_canonical(
                input, label,
            ))
            .await
        {
            Err(error) => {
                return SourcePreparationOutcome::Complete(Ok((
                    Err(RustToolchainHelperError::Compute(
                        format!("{error:?}").into(),
                    )),
                    observations,
                )));
            }
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return SourcePreparationOutcome::Complete(Err(error));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                let observations =
                    match union_host_observations(&observations, observed.observations()) {
                        Ok(value) => value,
                        Err(error) => return SourcePreparationOutcome::Complete(Err(error)),
                    };
                (observed.result().dupe(), observations)
            }
        },
    };
    SourcePreparationOutcome::Complete(Ok((
        result
            .as_ref()
            .clone()
            .map_err(|error| RustToolchainHelperError::Loading {
                label: canonical,
                message: format!("{error:?}").into(),
            }),
        observations,
    )))
}

fn complete(
    result: Result<PreparedRustToolchainHelperBinding, RustToolchainHelperError>,
    observations: PathObservationEpoch,
) -> RustToolchainHelperPreparationOutcome {
    SourcePreparationOutcome::Complete(Ok((result, observations)))
}

fn auth(message: &str) -> RustToolchainHelperError {
    RustToolchainHelperError::Authentication(message.into())
}

fn request_caller(request: &RustToolchainHelperRequest) -> BzlModuleSourceProvenance {
    match request {
        RustToolchainHelperRequest::Producer(source) => source.clone(),
        RustToolchainHelperRequest::Retained(source) => loading_source(&source.caller),
    }
}

fn root_source(manifest: &BzlLoadManifest) -> Option<&BzlModuleSourceProvenance> {
    manifest
        .reachable
        .iter()
        .find(|source| source.identity == manifest.root)
}

fn authenticate_source(
    source: &BzlModuleSourceProvenance,
    label: &str,
    digest: [u8; 32],
) -> Result<(), RustToolchainHelperError> {
    if source.identity.label.to_string() != label || source.source_digest != digest {
        return Err(auth(
            "fixed canonical source or evaluated digest does not match",
        ));
    }
    Ok(())
}

fn retained_identity(identity: &BzlModuleIdentity) -> RustBzlModuleIdentity {
    RustBzlModuleIdentity {
        label: identity.label.clone(),
        workspace_path: identity.workspace_path.clone(),
        repository_mapping: identity.repository_mapping.dupe(),
    }
}

fn retained_source(source: &BzlModuleSourceProvenance) -> RustBzlModuleSource {
    RustBzlModuleSource {
        identity: retained_identity(&source.identity),
        digest: source.source_digest,
    }
}

fn loading_source(source: &RustBzlModuleSource) -> BzlModuleSourceProvenance {
    BzlModuleSourceProvenance::new(
        BzlModuleIdentity {
            label: source.identity.label.clone(),
            workspace_path: source.identity.workspace_path.clone(),
            repository_mapping: source.identity.repository_mapping.dupe(),
        },
        source.digest,
    )
}

fn finish_binding(
    request: &RustToolchainHelperRequest,
    caller_manifest: &BzlLoadManifest,
    helper: FrozenBzlModule,
) -> Result<PreparedRustToolchainHelperBinding, RustToolchainHelperError> {
    let caller =
        root_source(caller_manifest).ok_or_else(|| auth("caller root source is absent"))?;
    authenticate_source(caller, CALLER_LABEL, CALLER_SHA)?;
    if caller != &request_caller(request) {
        return Err(auth("caller provenance changed"));
    }
    let helper_source =
        root_source(&helper.manifest).ok_or_else(|| auth("helper root source is absent"))?;
    authenticate_source(helper_source, HELPER_LABEL, HELPER_SHA)?;
    if !caller_manifest
        .direct_children
        .contains(&helper.manifest.root)
        || !caller_manifest.reachable.contains(helper_source)
    {
        return Err(auth(
            "fixed helper is not the evaluated caller's direct import",
        ));
    }
    let authority = Arc::new(RustToolchainCallableSourceAuthority {
        caller: retained_source(caller),
        helper: RustToolchainHelperManifest {
            root: retained_identity(&helper.manifest.root),
            direct_children: helper
                .manifest
                .direct_children
                .iter()
                .map(retained_identity)
                .collect::<Vec<_>>()
                .into(),
            reachable: helper
                .manifest
                .reachable
                .iter()
                .map(retained_source)
                .collect::<Vec<_>>()
                .into(),
            fingerprint: helper.manifest.fingerprint,
        },
    });
    if let RustToolchainHelperRequest::Retained(expected) = request {
        if expected != &authority {
            return Err(auth("retained helper authority changed"));
        }
    }
    let export = helper
        .module
        .get(EXPORT)
        .map_err(|error| RustToolchainHelperError::Export(error.to_string().into()))?;
    if export.value().get_type() != "function"
        || export
            .value()
            .request_value::<&dyn StarlarkFunctionIdentity>()
            .is_none()
    {
        return Err(RustToolchainHelperError::Export(
            "fixed export is not a Starlark function".into(),
        ));
    }
    let mut sources = SmallMap::new();
    let mut labels = SmallMap::new();
    for source in std::iter::once(caller).chain(helper.manifest.reachable.iter()) {
        if let Some(previous) = labels.get(&source.identity.label) {
            if previous != source {
                return Err(RustToolchainHelperError::Provenance(
                    "conflicting canonical source identity".into(),
                ));
            }
        } else {
            labels.insert(source.identity.label.clone(), source.clone());
        }
        let filename = starlark_source_name(&source.identity.workspace_path).ok_or_else(|| {
            RustToolchainHelperError::Provenance("invalid Starlark source filename".into())
        })?;
        if let Some(previous) = sources.get(filename.as_str()) {
            if previous != source {
                return Err(RustToolchainHelperError::Provenance(
                    "conflicting Starlark source filename".into(),
                ));
            }
        } else {
            sources.insert(CompactString::new(filename), source.clone());
        }
    }
    Ok(PreparedRustToolchainHelperBinding(Arc::new(
        PreparedBinding {
            module: helper,
            export,
            authority,
            sources: sources.into_iter().collect::<Vec<_>>().into(),
        },
    )))
}

#[cfg(test)]
mod tests;

/// Guard/lifetime tests only: this cannot mint authority for the fixed producer.
#[cfg(test)]
pub(crate) fn synthetic_test_binding(source: &str) -> PreparedRustToolchainHelperBinding {
    let identity = |name: &str| BzlModuleIdentity {
        label: CanonicalLabel::parse(&format!("@@synthetic+//:{name}")).unwrap(),
        workspace_path: PathBuf::from(format!("/synthetic/{name}")),
        repository_mapping: Arc::from([]),
    };
    let root = identity("rust_toolchain_helper.bzl");
    let module = Module::new();
    Evaluator::new(&module)
        .eval_module(
            AstModule::parse(
                root.workspace_path.to_str().unwrap(),
                source.to_owned(),
                &Dialect::Bazel,
            )
            .unwrap(),
            &starlark::environment::Globals::standard(),
        )
        .unwrap();
    let module = module.freeze().unwrap();
    let manifest = BzlLoadManifest::new(
        root.clone(),
        Sha256::digest(source).into(),
        std::iter::empty::<FrozenBzlModule>(),
    );
    let export = module.get(EXPORT).unwrap();
    let authority = Arc::new(RustToolchainCallableSourceAuthority {
        caller: retained_source(&BzlModuleSourceProvenance::new(
            identity("producer.bzl"),
            Sha256::digest([]).into(),
        )),
        helper: RustToolchainHelperManifest {
            root: retained_identity(&manifest.root),
            direct_children: Arc::from([]),
            reachable: manifest
                .reachable
                .iter()
                .map(retained_source)
                .collect::<Vec<_>>()
                .into(),
            fingerprint: manifest.fingerprint,
        },
    });
    let sources = manifest_starlark_sources(&manifest);
    PreparedRustToolchainHelperBinding(Arc::new(PreparedBinding {
        module: FrozenBzlModule {
            module,
            path: root.workspace_path,
            loads: Vec::new(),
            manifest,
            bzl_load_visibility: BzlLoadVisibility::Public,
            retained_bzl_modules: Arc::from([]),
        },
        export,
        authority,
        sources,
    }))
}
