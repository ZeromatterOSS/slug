//! Normal-only syntax metadata: the guarded parent has already completed this
//! exact source in the same transaction. This key has no module detector identity.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct ExternalBzlSyntaxKey {
    pub(super) module: ExternalBzlModuleEvalKey,
    pub(super) mode: ExternalBzlModuleMode,
}
impl fmt::Display for ExternalBzlSyntaxKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "external-bzl-syntax:{:?}:{}", self.mode, self.module)
    }
}

#[derive(Allocative)]
pub(super) struct ExternalBzlSyntax {
    pub(super) bytes: Arc<[u8]>,
    pub(super) source_name: String,
    #[allocative(skip)]
    pub(super) encoding: StringEncoding,
    pub(super) result: Result<Arc<[String]>, ExternalBzlModuleError>,
}
impl fmt::Debug for ExternalBzlSyntax {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExternalBzlSyntax")
            .field("source_bytes", &self.bytes.len())
            .field("source_name", &self.source_name)
            .field("encoding", &self.encoding)
            .field(
                "load_count",
                &self.result.as_ref().ok().map(|loads| loads.len()),
            )
            .finish()
    }
}
impl PartialEq for ExternalBzlSyntax {
    fn eq(&self, other: &Self) -> bool {
        (Arc::ptr_eq(&self.bytes, &other.bytes) || self.bytes == other.bytes)
            && self.source_name == other.source_name
            && self.encoding == other.encoding
            && self.result == other.result
    }
}
impl Eq for ExternalBzlSyntax {}

pub(super) fn parse_external_bzl(
    source: &ExternalBzlSourceValue,
    label: &CanonicalLabel,
) -> Result<AstModule, ExternalBzlModuleError> {
    let text =
        std::str::from_utf8(&source.bytes).map_err(|_| ExternalBzlModuleError::Encoding {
            label: label.clone(),
        })?;
    AstModule::parse_with_string_encoding(
        &source.source_name,
        text.to_owned(),
        &Dialect::Bazel,
        StringEncoding::BazelInternal,
    )
    .map_err(|error| ExternalBzlModuleError::Parse {
        label: label.clone(),
        message: Arc::from(error.to_string()),
    })
}

impl ExternalBzlSyntax {
    pub(super) fn from_source(source: ExternalBzlSourceValue, label: &CanonicalLabel) -> Self {
        let result = parse_external_bzl(&source, label).map(|ast| {
            ast.loads()
                .into_iter()
                .map(|load| load.module_id.to_owned())
                .collect::<Vec<_>>()
                .into()
        });
        Self {
            bytes: source.bytes,
            source_name: source.source_name,
            encoding: StringEncoding::BazelInternal,
            result,
        }
    }
}

// Defensive non-pure failures are never cache-valid. Only successful source-bound
// syntax metadata (including deterministic parse/encoding errors) is reusable.
pub(super) type ExternalBzlSyntaxOutcome = Result<Arc<ExternalBzlSyntax>, ExternalBzlDriverOutcome>;
#[async_trait]
impl Key for ExternalBzlSyntaxKey {
    type Value = ExternalBzlSyntaxOutcome;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let child = match compute_external_bzl_source(ctx, &self.module, self.mode).await {
            SourcePreparationOutcome::Need(need) => {
                return Err(SourcePreparationOutcome::Need(need));
            }
            SourcePreparationOutcome::Complete(Err(error)) => {
                return Err(SourcePreparationOutcome::Complete(Err(error)));
            }
            SourcePreparationOutcome::Complete(Ok(child)) => child,
        };
        let label = self.module.canonical_label();
        let source = finish_external_bzl_source(child, label.clone())?;
        let syntax = Arc::new(ExternalBzlSyntax::from_source(source, &label));
        // Existing test event capture exposes only a Weak for ownership assertions;
        // no graph/body owner or production instrumentation is added.
        #[cfg(test)]
        if ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok()
        {
            ctx.store_evaluation_data(Arc::downgrade(&syntax))
                .expect("external syntax stores one ownership witness");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syntax_metadata_shares_bytes_and_has_exact_pure_identity() {
        let label = CanonicalLabel::parse("@@dep+//:root.bzl").unwrap();
        let bytes: Arc<[u8]> = Arc::from(b"load(':leaf.bzl', 'X')\nY = X + 1\n".as_slice());
        let source = |bytes: Arc<[u8]>, name: &str| ExternalBzlSourceValue {
            bytes,
            source_name: name.to_owned(),
            presentation_path: PathBuf::from(name),
            observations: PathObservationEpoch::empty(),
        };
        let syntax = ExternalBzlSyntax::from_source(source(bytes.dupe(), "root.bzl"), &label);
        assert!(Arc::ptr_eq(&syntax.bytes, &bytes));
        let equal =
            ExternalBzlSyntax::from_source(source(Arc::from(bytes.as_ref()), "root.bzl"), &label);
        assert!(!Arc::ptr_eq(&syntax.bytes, &equal.bytes));
        assert_eq!(syntax, equal);
        let changed = ExternalBzlSyntax::from_source(
            source(
                Arc::from(b"load(':leaf.bzl', 'X')\nY = X + 2\n".as_slice()),
                "root.bzl",
            ),
            &label,
        );
        assert_eq!(syntax.result, changed.result);
        assert_ne!(syntax, changed);
        let renamed = ExternalBzlSyntax::from_source(source(bytes.dupe(), "other.bzl"), &label);
        assert_ne!(syntax, renamed);
        let mut encoding = equal;
        encoding.encoding = StringEncoding::Unicode;
        assert_ne!(syntax, encoding);
        let invalid = ExternalBzlSyntax::from_source(
            source(Arc::from(b"\xff".as_slice()), "root.bzl"),
            &label,
        );
        assert!(matches!(
            invalid.result,
            Err(ExternalBzlModuleError::Encoding { .. })
        ));
        let mut diagnostic = encoding;
        diagnostic.result = Err(ExternalBzlModuleError::Parse {
            label: label.clone(),
            message: Arc::from("source-body-diagnostic-sentinel"),
        });
        assert!(!format!("{diagnostic:?}").contains("source-body-diagnostic-sentinel"));
        let value: ExternalBzlSyntaxOutcome = Ok(Arc::new(syntax));
        assert!(ExternalBzlSyntaxKey::validity(&value));
        assert!(ExternalBzlSyntaxKey::equality(&value, &value));
        assert!(!format!("{value:?}").contains("Y ="));
    }
}
