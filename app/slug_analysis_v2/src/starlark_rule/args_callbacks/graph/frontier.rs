//! Bounded typed native frontiers. Physical views and native semantic payloads
//! are both inspected; runtime caches are never evaluated during publication.
use slug_build_api_v2::published_value::*;

use super::*;

impl<'v> PublicationBuilder<'v> {
    pub(super) fn function_symbol(
        &mut self,
        value: Value<'v>,
        depth: usize,
    ) -> Result<PublishedFunctionSymbol, String> {
        let inspected =
            starlark::eval::inspect_function_symbol(value).ok_or("missing actual Def symbol")?;
        let owner = slug_loading_v2::function_owner::function_symbol_owner_ref(value)
            .map_err(|e| e.to_string())?
            .ok_or("missing modeled function owner")?;
        let entry_visits = self.graph.charged_visits();
        self.graph.audit_owner_shape_at_depth(owner, depth).map_err(|error| {
            format!("actual function owner publication quota: {error}; domain={:?}; owner={:?}; aggregate_visits={}; local={:?}; entry_visits={entry_visits}; accepted_delta={}; intrinsic={:?}; stable_key={:?}; input_validity={:?}", owner.domain(), owner.retained_shape(), self.graph.charged_visits(), self.graph.local_shape(), self.graph.charged_visits().saturating_sub(entry_visits), owner.intrinsic_shape(), owner.stable_key().retained_shape(), owner.input_validity().retained_shape())
        })?;
        if let starlark::eval::FunctionSymbolStateRef::Global { name } = inspected.state() {
            self.graph
                .charge_bytes(name.len().saturating_add(std::mem::size_of::<Arc<str>>()))
                .map_err(|e| e.to_string())?;
        }
        slug_loading_v2::function_owner::snapshot_function_symbol(value)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "missing modeled function owner".into())
    }

    pub(super) fn frontier(
        &mut self,
        value: Value<'v>,
        lowerer: &mut AnalysisValueLowerer<'v>,
        depth: usize,
    ) -> Result<Option<PublishedNode>, String> {
        if let Some((target, views)) =
            crate::analysis_value::published::configured_target_view(value)
        {
            self.graph
                .audit_owner_configured_target_at_depth(target, depth)
                .map_err(|e| e.to_string())?;
            self.membership(target.providers().len())?;
            if target.providers().len() != views.len() {
                return Err("configured target physical/native provider map mismatch".into());
            }
            let mut providers = Vec::with_capacity(views.len());
            for (identity, provider) in target.providers().iter() {
                self.step(depth)?;
                let view = views
                    .get(identity)
                    .ok_or("configured target is missing a physical provider view")?;
                let view = PublishedEdge::Local(self.value(view.to_value(), lowerer, depth + 1)?);
                providers.push(match provider {
                    slug_build_api_v2::ProviderValue::Occurrence(_) => {
                        PublishedTargetProvider::Occurrence(view)
                    }
                    provider => PublishedTargetProvider::Native {
                        provider: self
                            .graph
                            .provider_value_atom_borrowed(provider)
                            .map_err(|e| e.to_string())?,
                        view,
                    },
                });
            }
            return Ok(Some(PublishedNode::ConfiguredTarget {
                identity: target.identity().clone(),
                providers: providers.into(),
            }));
        }
        if let Some(header) = slug_loading_v2::cc_variables_header(value) {
            self.membership(header.local_count)?;
            let mut locals = Vec::with_capacity(header.local_count);
            slug_loading_v2::visit_cc_variables_locals(value, &mut |name, child| {
                locals.push((
                    self.text(name)?,
                    PublishedEdge::Local(self.value(child, lowerer, depth + 1)?),
                ));
                Ok::<_, String>(())
            })
            .ok_or("missing C++ variable locals")??;
            locals.sort_by(|(a, _), (b, _)| a.encode_utf16().cmp(b.encode_utf16()));
            let parent = header
                .parent
                .map(|value| {
                    self.value(value, lowerer, depth + 1)
                        .map(PublishedEdge::Local)
                })
                .transpose()?;
            let entries =
                slug_loading_v2::snapshot_cc_variables_seed_bounded(value, &mut |bytes, edges| {
                    self.graph.charge_bytes(bytes).map_err(|e| e.to_string())?;
                    self.membership(edges)
                })?
                .ok_or("missing C++ variable seed")?;
            let mut seed = Vec::with_capacity(entries.len());
            for (name, entry) in entries {
                self.step(depth)?;
                use slug_build_api_v2::cc_variables::CcVariableCacheEntry;
                let entry = match entry {
                    CcVariableCacheEntry::Value(value) => PublishedVariableSeed::Value(
                        PublishedEdge::Local(self.value(value, lowerer, depth + 1)?),
                    ),
                    CcVariableCacheEntry::Sequence(values) => {
                        let values = values
                            .iter()
                            .map(|value| self.value(*value, lowerer, depth + 1))
                            .collect::<Result<Vec<_>, _>>()?;
                        PublishedVariableSeed::Sequence(self.edges(&values)?)
                    }
                    CcVariableCacheEntry::Missing => PublishedVariableSeed::Missing,
                    CcVariableCacheEntry::Error(error) => PublishedVariableSeed::Error(error),
                };
                seed.push((name, entry));
            }
            seed.sort_by(|(a, _), (b, _)| a.encode_utf16().cmp(b.encode_utf16()));
            return Ok(Some(PublishedNode::CcVariables(PublishedCcVariables {
                kind: header.kind,
                occurrence: header.occurrence.clone(),
                parent,
                locals: locals.into(),
                seed: seed.into(),
            })));
        }
        Ok(None)
    }
}
