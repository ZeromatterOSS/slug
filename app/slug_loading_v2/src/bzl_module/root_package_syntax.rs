//! Pure BUILD syntax below the normal parent's fresh source and observation frontier.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct RootPackageSyntaxKey {
    pub(super) package: RootPackageLoadKey,
    pub(super) mode: HostPackageLoadMode,
}
impl fmt::Display for RootPackageSyntaxKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "root-package-syntax:{:?}:{}", self.mode, self.package)
    }
}

#[derive(Allocative)]
pub(super) struct RootPackageSyntax {
    pub(super) bytes: Arc<[u8]>,
    pub(super) logical_path: NormalizedAbsolutePath,
    pub(super) relative_build: Arc<[u8]>,
    pub(super) result: Result<Arc<[String]>, RootPackageLoadError>,
}
impl fmt::Debug for RootPackageSyntax {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RootPackageSyntax")
            .field("logical_path", &self.logical_path)
            .field("source_bytes", &self.bytes.len())
            .field(
                "load_count",
                &self.result.as_ref().ok().map(|loads| loads.len()),
            )
            .finish()
    }
}
impl PartialEq for RootPackageSyntax {
    fn eq(&self, other: &Self) -> bool {
        (Arc::ptr_eq(&self.bytes, &other.bytes) || self.bytes == other.bytes)
            && self.logical_path == other.logical_path
            && self.relative_build == other.relative_build
            && self.result == other.result
    }
}
impl Eq for RootPackageSyntax {}

impl RootPackageSyntax {
    pub(super) fn from_source(source: &RootPackageSource, package: &PackagePath) -> Self {
        let result = (|| {
            // UTF-8 failure precedes filename conversion and parsing, as in the parent.
            let text = host_source_text(source).map_err(|error| {
                RootPackageLoadError::new(RootPackageLoadErrorInner::Input(error))
            })?;
            let name = host_source_name(source).map_err(|error| {
                RootPackageLoadError::new(RootPackageLoadErrorInner::Input(error))
            })?;
            let ast = AstModule::parse_with_string_encoding(
                &name,
                text.to_owned(),
                &Dialect::Bazel,
                StringEncoding::BazelInternal,
            )
            .map_err(|error| {
                RootPackageLoadError::new(RootPackageLoadErrorInner::Parse {
                    package: package.clone(),
                    message: Arc::from(error.to_string()),
                })
            })?;
            Ok(ast
                .loads()
                .into_iter()
                .map(|load| load.module_id.to_owned())
                .collect::<Vec<_>>()
                .into())
        })();
        Self {
            bytes: source.bytes().dupe(),
            logical_path: source.logical_path().dupe(),
            relative_build: source.relative_path().dupe(),
            result,
        }
    }
}

// Only complete source-bound metadata, including pure parse/input errors, is reusable.
pub(super) type RootPackageSyntaxOutcome =
    Result<Arc<RootPackageSyntax>, RootPackageLoadDriverOutcome>;
#[async_trait]
impl Key for RootPackageSyntaxKey {
    type Value = RootPackageSyntaxOutcome;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let child = match self.mode {
            HostPackageLoadMode::Legacy => host_dice_invariant(
                ctx.compute(&RootPackageSourceKey::for_build(
                    self.package.workspace.dupe(),
                    self.package.package.clone(),
                ))
                .await,
            )
            .map(|value| Ok((value.as_ref().clone(), PathObservationEpoch::empty()))),
            HostPackageLoadMode::Observed => host_dice_invariant(
                ctx.compute(&RootPackageSourceObservationKey::for_build(
                    self.package.workspace.dupe(),
                    self.package.package.clone(),
                ))
                .await,
            )
            .map(|value| {
                value.map(|observed| (observed.result().clone(), observed.observations().dupe()))
            }),
        };
        let (source, observations) = match child {
            SourcePreparationOutcome::Need(need) => {
                return Err(SourcePreparationOutcome::Need(need));
            }
            SourcePreparationOutcome::Complete(Err(error)) => {
                return Err(SourcePreparationOutcome::Complete(Err(error)));
            }
            SourcePreparationOutcome::Complete(Ok(value)) => value,
        };
        let source = source.map_err(|error| {
            root_package_driver_complete(
                Err(RootPackageLoadError::new(
                    RootPackageLoadErrorInner::Source(error),
                )),
                observations,
            )
        })?;
        let syntax = Arc::new(RootPackageSyntax::from_source(
            &source,
            &self.package.package,
        ));
        #[cfg(test)]
        if ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok()
        {
            ctx.store_evaluation_data(Arc::downgrade(&syntax))
                .expect("root syntax stores one Weak ownership witness");
        }
        Ok(syntax)
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        matches!((x, y), (Ok(x), Ok(y)) if Arc::ptr_eq(x, y) || x == y)
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_ok()
    }
}
