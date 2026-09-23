//! Native InstrumentedFilesInfo.EMPTY only; coverage collection remains closed.
use super::*;

const FIELDS: [&str; 2] = ["instrumented_files", "metadata_files"];

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub(super) struct EmptyInstrumentedFiles {
    fields: [FrozenValue; 2],
}

impl EmptyInstrumentedFiles {
    pub(super) fn validate(occurrence: &ProviderOccurrence) -> Result<(), String> {
        if occurrence.fields().len() == FIELDS.len()
            && FIELDS.iter().all(|name| {
                occurrence.fields().get(*name).is_some_and(|value| {
                    matches!(value.kind(), AnalysisValueKind::Depset(files)
                        if files.is_empty() && files.order() == slug_build_api_v2::DepsetOrder::Default)
                })
            })
        {
            Ok(())
        } else {
            Err("only native empty InstrumentedFilesInfo is supported".into())
        }
    }

    pub(super) fn alloc(
        heap: &FrozenHeap,
        fields: Vec<(CompactString, FrozenValue)>,
    ) -> FrozenValue {
        let field = |name| fields.iter().find(|(key, _)| key == name).unwrap().1;
        heap.alloc(Self {
            fields: [field(FIELDS[0]), field(FIELDS[1])],
        })
    }

    pub(super) fn fields_from_value(value: Value<'_>) -> Option<Vec<(CompactString, Value<'_>)>> {
        let this = Self::from_value(value)?;
        Some(
            FIELDS
                .into_iter()
                .zip(this.fields)
                .map(|(name, value)| (name.into(), value.to_value()))
                .collect(),
        )
    }
}

impl fmt::Display for EmptyInstrumentedFiles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("struct(instrumented_files = depset([]), metadata_files = depset([]))")
    }
}

starlark::starlark_simple_value!(EmptyInstrumentedFiles);
#[starlark_value(type = "InstrumentedFilesInfo")]
impl<'v> StarlarkValue<'v> for EmptyInstrumentedFiles {
    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other).is_some())
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        // Hash bytes are Slug-native; all admitted instances are the native EMPTY.
        "InstrumentedFilesInfo.EMPTY".hash(hasher);
        Ok(())
    }

    fn get_attr(&self, name: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        FIELDS
            .iter()
            .position(|field| *field == name)
            .map(|index| self.fields[index].to_value())
    }

    fn dir_attr(&self) -> Vec<String> {
        FIELDS.into_iter().map(str::to_owned).collect()
    }
}

#[cfg(test)]
mod tests {
    use slug_build_api_v2::DepsetOrder;

    use super::*;

    fn empty() -> ProviderOccurrence {
        ProviderOccurrence::new(
            ProviderIdentity::builtin("InstrumentedFilesInfo"),
            FIELDS.map(|name| {
                (
                    name,
                    AnalysisValue::depset(AnalysisDepset::empty(DepsetOrder::Default)),
                )
            }),
        )
    }

    #[test]
    fn empty_native_facade_roundtrips_without_new_retained_identity() {
        let heap = FrozenHeap::new();
        let mut materializer = AnalysisValueMaterializer::new(&heap);
        let first = materializer.provider(&empty()).unwrap();
        let second = materializer.provider(&empty()).unwrap();
        assert_eq!(first.to_value().get_type(), "InstrumentedFilesInfo");
        assert_eq!(
            first.to_value().to_repr(),
            "struct(instrumented_files = depset([]), metadata_files = depset([]))"
        );
        assert!(first.to_value().equals(second.to_value()).unwrap());
        assert_eq!(
            first.to_value().get_hashed().unwrap().hash(),
            second.to_value().get_hashed().unwrap().hash()
        );
        let view = EmptyInstrumentedFiles::from_value(first.to_value()).unwrap();
        assert_eq!(view.dir_attr(), FIELDS);
        let lowered = AnalysisValueLowerer::default()
            .lower(first.to_value(), "empty")
            .unwrap();
        assert!(lowered.publication_eq(&AnalysisValue::provider(empty())));
        let nested = AnalysisValue::list(vec![lowered.clone(), lowered]);
        let frozen = materializer.value(&nested).unwrap();
        assert!(
            AnalysisValueLowerer::default()
                .lower(frozen.to_value(), "nested")
                .unwrap()
                .publication_eq(&nested)
        );
    }

    #[test]
    fn empty_native_facade_rejects_missing_extra_or_nonempty_fields() {
        let heap = FrozenHeap::new();
        let mut materializer = AnalysisValueMaterializer::new(&heap);
        for fields in [
            vec![],
            vec![("instrumented_files", AnalysisValue::none())],
            vec![
                (
                    "instrumented_files",
                    AnalysisValue::depset(AnalysisDepset::empty(DepsetOrder::Postorder)),
                ),
                (
                    "metadata_files",
                    AnalysisValue::depset(AnalysisDepset::empty(DepsetOrder::Default)),
                ),
            ],
            vec![
                (
                    "instrumented_files",
                    AnalysisValue::depset(AnalysisDepset::empty(DepsetOrder::Default)),
                ),
                (
                    "metadata_files",
                    AnalysisValue::depset(AnalysisDepset::empty(DepsetOrder::Default)),
                ),
                ("extra", AnalysisValue::none()),
            ],
            vec![
                (
                    "instrumented_files",
                    AnalysisValue::depset(
                        AnalysisDepset::new(
                            DepsetOrder::Default,
                            vec![AnalysisValue::artifact(AnalysisArtifact::Source(
                                slug_identity_v2::CanonicalLabel::parse("@@//:coverage").unwrap(),
                            ))],
                            vec![],
                        )
                        .unwrap(),
                    ),
                ),
                (
                    "metadata_files",
                    AnalysisValue::depset(AnalysisDepset::empty(DepsetOrder::Default)),
                ),
            ],
        ] {
            let occurrence =
                ProviderOccurrence::new(ProviderIdentity::builtin("InstrumentedFilesInfo"), fields);
            assert!(
                materializer
                    .provider(&occurrence)
                    .unwrap_err()
                    .contains("only native empty")
            );
        }
        let user = ProviderOccurrence::empty(ProviderIdentity::builtin("UnrelatedInfo"));
        assert_eq!(
            materializer.provider(&user).unwrap().to_value().get_type(),
            "provider"
        );
    }
}
