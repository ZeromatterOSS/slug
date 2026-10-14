//! Actual provider constructors retain initializer/raw edges without an invoke shim.
use slug_build_api_v2::published_value::PublishedEdge;
use slug_build_api_v2::published_value::PublishedProviderConstructor;
use slug_build_api_v2::published_value::PublishedProviderSchema;
use slug_loading_v2::provider::ProviderConstructorView;
use slug_loading_v2::provider::ProviderSchemaRef;

use super::*;

impl<'v> PublicationBuilder<'v> {
    pub(super) fn provider_constructor(
        &mut self,
        id: PublishedNodeId,
        view: ProviderConstructorView<'v>,
        lowerer: &mut AnalysisValueLowerer<'v>,
        depth: usize,
    ) -> Result<(), String> {
        self.step(depth)?;
        let row = match view {
            ProviderConstructorView::Raw {
                provider,
                occurrence,
            } => PublishedNode::RawProviderConstructor {
                occurrence: occurrence.clone(),
                provider: PublishedEdge::Local(self.value(provider, lowerer, depth + 1)?),
            },
            ProviderConstructorView::Constructor { id, schema, init } => {
                let id = id.ok_or("unexported provider constructor cannot be published")?;
                let names = |this: &mut Self,
                             names: &[CompactString]|
                 -> Result<Arc<[CompactString]>, String> {
                    this.membership(names.len())?;
                    this.graph
                        .charge_bytes(
                            names
                                .len()
                                .saturating_mul(std::mem::size_of::<CompactString>()),
                        )
                        .map_err(|e| e.to_string())?;
                    names
                        .iter()
                        .map(|name| this.text(name))
                        .collect::<Result<Vec<_>, _>>()
                        .map(Arc::from)
                };
                let schema = match schema {
                    ProviderSchemaRef::Schemaless => PublishedProviderSchema::Schemaless,
                    ProviderSchemaRef::List(fields) => {
                        PublishedProviderSchema::List(names(self, fields)?)
                    }
                    ProviderSchemaRef::Documented(fields) => {
                        PublishedProviderSchema::Documented(names(self, fields)?)
                    }
                    ProviderSchemaRef::Initialized(fields) => {
                        PublishedProviderSchema::Initialized(names(self, fields)?)
                    }
                };
                let init = init
                    .map(|value| {
                        self.value(value, lowerer, depth + 1)
                            .map(PublishedEdge::Local)
                    })
                    .transpose()?;
                PublishedNode::ProviderConstructor(PublishedProviderConstructor {
                    id: id.clone(),
                    schema,
                    init,
                })
            }
        };
        self.set(id, row)
    }
}
