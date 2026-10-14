//! Outward native projections are built only after original roots and rows seal.
use slug_build_api_v2::published_value::*;
use slug_build_api_v2::*;
use starlark_map::unordered_map::UnorderedMap;

#[derive(Default)]
pub(crate) struct Projector {
    values: UnorderedMap<PublishedValueIdentity, AnalysisValue>,
    // Phase-only native typed rows; outward wrappers are made on demand.
    depsets: UnorderedMap<PublishedValueIdentity, AnalysisDepset>,
    budget: budget::ProjectionBudget,
}
impl Projector {
    pub(crate) fn value(&mut self, original: &PublishedValueRef) -> Result<AnalysisValue, String> {
        self.budget.admit_arena(original.arena())?;
        let key = original.identity();
        if let Some(value) = self.values.get(&key) {
            return Ok(value.clone());
        }
        // Seal has already refused projection-containment cycles. Functions and
        // constructors are symbolic leaves; their captures remain arena edges.
        let projected = match original.node() {
            PublishedNode::Function(value) => AnalysisValue::function_symbol(value.symbol.clone()),
            PublishedNode::ProviderConstructor(value) => {
                AnalysisValue::provider_constructor_symbol(PublishedProviderConstructorSymbol {
                    id: value.id.clone(),
                    schema: value.schema.clone(),
                })
            }
            PublishedNode::RawProviderConstructor {
                provider,
                occurrence,
            } => {
                let provider = original.arena().resolve(*provider).map_err(err)?;
                let PublishedNode::ProviderConstructor(provider) = provider.node() else {
                    return Err("invalid raw provider relationship".into());
                };
                AnalysisValue::raw_provider_constructor_symbol(
                    PublishedRawProviderConstructorSymbol {
                        provider: provider.id.clone(),
                        occurrence: occurrence.clone(),
                    },
                )
            }
            PublishedNode::Native(value) => value.value().clone(),
            PublishedNode::Set(_) => {
                return Err("outward set projection is unsupported".into());
            }
            PublishedNode::List(edges) => AnalysisValue::list(self.children(original, *edges)?),
            PublishedNode::Tuple(edges) => AnalysisValue::tuple(self.children(original, *edges)?),
            PublishedNode::Dictionary(edges) => {
                let values = self.children(original, *edges)?;
                AnalysisValue::dictionary(
                    values.chunks_exact(2).map(|p| (p[0].clone(), p[1].clone())),
                )
                .map_err(err)?
            }
            PublishedNode::Struct { names, values } => {
                AnalysisValue::strukt(names.iter().cloned().zip(self.children(original, *values)?))
            }
            PublishedNode::Provider(row) => {
                let fields = row
                    .names
                    .iter()
                    .cloned()
                    .zip(self.children(original, row.values)?)
                    .collect::<Vec<_>>();
                return self.provider(original, fields);
            }
            PublishedNode::Depset(_) => return self.depset(original),
            PublishedNode::CcVariables(row) => {
                use slug_build_api_v2::cc_variables::CcToolchainVariables;
                use slug_build_api_v2::cc_variables::CcVariableCacheEntry;
                use slug_build_api_v2::cc_variables::CcVariablesModel;
                let parent = row
                    .parent
                    .map(|edge| -> Result<CcToolchainVariables, String> {
                        let child = self.edge(original, edge)?;
                        let AnalysisValueKind::CcToolchainVariables(child) = child.kind() else {
                            return Err("invalid C++ variable parent projection".into());
                        };
                        Ok(child.clone())
                    })
                    .transpose()?;
                let locals = row
                    .locals
                    .iter()
                    .map(|(name, edge)| Ok((name.clone(), self.edge(original, *edge)?)))
                    .collect::<Result<Box<[_]>, String>>()?;
                let cache = row
                    .seed
                    .iter()
                    .map(|(name, seed)| {
                        Ok((
                            name.clone(),
                            match seed {
                                PublishedVariableSeed::Value(edge) => {
                                    CcVariableCacheEntry::Value(self.edge(original, *edge)?)
                                }
                                PublishedVariableSeed::Sequence(edges) => {
                                    CcVariableCacheEntry::Sequence(
                                        self.children(original, *edges)?.into_boxed_slice(),
                                    )
                                }
                                PublishedVariableSeed::Missing => CcVariableCacheEntry::Missing,
                                PublishedVariableSeed::Error(message) => {
                                    CcVariableCacheEntry::Error(message.clone())
                                }
                            },
                        ))
                    })
                    .collect::<Result<Box<[_]>, String>>()?;
                AnalysisValue::cc_toolchain_variables(CcToolchainVariables::new(CcVariablesModel {
                    kind: row.kind,
                    occurrence: row.occurrence.clone(),
                    parent,
                    locals,
                    cache,
                }))
            }
            PublishedNode::ConfiguredTarget {
                identity,
                providers,
            } => {
                let mut values = Vec::with_capacity(providers.len());
                for provider in &**providers {
                    values.push(match provider {
                        PublishedTargetProvider::Native { provider, .. } => {
                            provider.value().clone()
                        }
                        PublishedTargetProvider::Occurrence(edge) => {
                            let value = self.edge(original, *edge)?;
                            let AnalysisValueKind::Provider(value) = value.kind() else {
                                return Err("target occurrence projection is not a provider".into());
                            };
                            ProviderValue::Occurrence(value.clone())
                        }
                    });
                }
                AnalysisValue::configured_target(ConfiguredTargetValue::new(
                    identity.clone(),
                    ProviderCollection::new(values).map_err(err)?,
                ))
            }
            PublishedNode::Intrinsic(_) => {
                return Err(
                    "native API descriptors cannot be projected as Starlark function tokens".into(),
                );
            }
            PublishedNode::Code(_) | PublishedNode::Cell { .. } => {
                return Err("code/cell descriptors have no outward native projection".into());
            }
        };
        let result =
            AnalysisValue::with_original_origin(projected, original.clone()).map_err(err)?;
        self.values.insert(key, result.clone());
        Ok(result)
    }
    fn provider(
        &mut self,
        original: &PublishedValueRef,
        fields: Vec<(compact_str::CompactString, AnalysisValue)>,
    ) -> Result<AnalysisValue, String> {
        let occurrence = original.project_provider(fields).map_err(err)?;
        let value = AnalysisValue::provider(occurrence);
        self.values.insert(original.identity(), value.clone());
        Ok(value)
    }
    fn edge(
        &mut self,
        parent: &PublishedValueRef,
        edge: PublishedEdge,
    ) -> Result<AnalysisValue, String> {
        self.value(&parent.arena().resolve(edge).map_err(err)?)
    }
    fn children(
        &mut self,
        parent: &PublishedValueRef,
        range: PublishedEdgeRange,
    ) -> Result<Vec<AnalysisValue>, String> {
        parent
            .arena()
            .edges(range)
            .map_err(err)?
            .iter()
            .map(|edge| self.edge(parent, *edge))
            .collect()
    }
}
fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

mod depset;

mod budget;
