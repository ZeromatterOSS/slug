//! Actual Engine parameter and exhaustive compiled dependency projection.
use std::sync::Arc;

use slug_build_api_v2::CallableDefinitionSpan;
use slug_build_api_v2::CallableGlobalAccess;
use slug_build_api_v2::CallableParameterMode;
use slug_build_api_v2::published_value::PublishedCode;
use slug_build_api_v2::published_value::PublishedGlobal;
use slug_build_api_v2::published_value::PublishedParameter;
use slug_build_api_v2::published_value::PublishedParameterRequirement;
use starlark::eval::DefCodeInspection;
use starlark::eval::DefDependency;
use starlark::eval::DefParameter;
use starlark::eval::InspectionBudget;
use starlark::eval::ModuleAccess;
use starlark::eval::ParameterRequirement;
use starlark::eval::ParameterStyle;

use super::AnalysisValueLowerer;
use super::CALLABLE_GRAPH_DEPTH_LIMIT;
use super::CALLABLE_GRAPH_EDGE_LIMIT;
use super::CALLABLE_GRAPH_NODE_LIMIT;
use super::PublicationBuilder;
use super::PublishedNode;
use super::bounded_value_class;
use super::host_error;

impl<'v> PublicationBuilder<'v> {
    pub(super) fn parameters(
        &mut self,
        parameters: impl ExactSizeIterator<Item = DefParameter<'v>>,
        lowerer: &mut AnalysisValueLowerer<'v>,
        depth: usize,
    ) -> Result<Vec<PublishedParameter>, String> {
        if parameters.len() > CALLABLE_GRAPH_NODE_LIMIT {
            return Err("Args parameter count limit".into());
        }
        self.graph
            .charge_bytes(
                parameters
                    .len()
                    .saturating_mul(std::mem::size_of::<PublishedParameter>()),
            )
            .map_err(|e| e.to_string())?;
        let mut result = Vec::new();
        for parameter in parameters {
            self.step(depth)?;
            if parameter.annotation.is_some() {
                return Err("Args callable parameter annotations are unsupported".into());
            }
            let requirement = match parameter.requirement {
                ParameterRequirement::Required => PublishedParameterRequirement::Required,
                ParameterRequirement::Optional => PublishedParameterRequirement::Optional,
                ParameterRequirement::Defaulted(value) => PublishedParameterRequirement::Defaulted(
                    slug_build_api_v2::published_value::PublishedEdge::Local(
                        self.value(value, lowerer, depth + 1).map_err(|error| {
                            format!("Args parameter {} default: {error}", parameter.name)
                        })?,
                    ),
                ),
                ParameterRequirement::Variadic => PublishedParameterRequirement::Variadic,
            };
            let mode = match parameter.style {
                ParameterStyle::PositionalOnly => CallableParameterMode::PositionalOnly,
                ParameterStyle::PositionalOrNamed => CallableParameterMode::PositionalOrNamed,
                ParameterStyle::NamedOnly => CallableParameterMode::NamedOnly,
                ParameterStyle::Args => CallableParameterMode::Args,
                ParameterStyle::Kwargs => CallableParameterMode::Kwargs,
            };
            result.push(PublishedParameter {
                name: self.text(parameter.name)?,
                mode,
                requirement,
                annotation: None,
            });
        }
        Ok(result)
    }
    pub(super) fn code(
        &mut self,
        code: DefCodeInspection<'v>,
        lowerer: &mut AnalysisValueLowerer<'v>,
        depth: usize,
    ) -> Result<PublishedCode, String> {
        self.step(depth)?;
        let source = self.sources.authority(
            code.original_module_client_data(),
            code.source(),
            &mut self.graph,
            &mut self.remaining,
        )?;
        let name = self.text(code.name())?;
        let span = code.signature_span().span;
        let initial_host_remaining = self.remaining;
        let initial_native_available =
            CALLABLE_GRAPH_NODE_LIMIT.saturating_sub(self.graph.charged_visits());
        let remaining_nodes = initial_host_remaining.min(initial_native_available);
        let mut budget = InspectionBudget {
            remaining_nodes,
            maximum_depth: CALLABLE_GRAPH_DEPTH_LIMIT.saturating_sub(depth),
        };
        // Engine streams exhaustively. This bounded host scratch queue permits
        // recursive callable expansion after releasing the walker's budget borrow.
        let mut events = Vec::new();
        let mut event_credit = super::storage::PeakCredit::default();
        code.visit_dependencies(&mut budget, &mut |event| {
            if events.len() >= CALLABLE_GRAPH_EDGE_LIMIT {
                return Err(host_error("Args dependency event limit".into()));
            }
            // The borrowed event queue drops before this code is returned.
            // Pay its complete existing Vec peak policy before push/growth;
            // source/output metadata and original roots remain retained.
            event_credit
                .vector::<DefDependency<'v>>(
                    &mut self.graph.producer_scratch(),
                    events.len() + 1,
                )
                .map_err(host_error)?;
            events.push(event);
            Ok(())
        })
        .map_err(|error| {
            let name = code.name();
            let mut name_end = name.len().min(96);
            while !name.is_char_boundary(name_end) {
                name_end -= 1;
            }
            format!(
                "Args callable {:?} compiled inspection (name_truncated={}; initial_host_remaining={initial_host_remaining}; initial_native_available={initial_native_available}; allowance={remaining_nodes}; consumed={}; remaining={}; buffered_events={}): {error}",
                &name[..name_end],
                name_end != name.len(),
                remaining_nodes.saturating_sub(budget.remaining_nodes),
                budget.remaining_nodes,
                events.len()
            )
        })?;
        for _ in budget.remaining_nodes..remaining_nodes {
            self.graph.charge_visit().map_err(|e| e.to_string())?;
        }
        self.remaining = budget.remaining_nodes;
        let mut dependencies = Vec::new();
        let mut globals = Vec::new();
        for event in events {
            match event {
                DefDependency::ModuleSymbol {
                    name,
                    value,
                    access,
                } => {
                    let name = self.text(name)?;
                    let value = value
                        .map(|value| {
                            self.value(value, lowerer, depth + 1).map_err(|error| {
                                let provider = slug_loading_v2::provider::starlark_provider_identity(value);
                                format!(
                                    "Args callable {} global {name} (provider constructor {provider:?}): {error}",
                                    code.name()
                                )
                            })
                        })
                        .transpose()?;
                    let access = match access {
                        ModuleAccess::Read => CallableGlobalAccess::Read,
                        ModuleAccess::Write => CallableGlobalAccess::Write,
                        ModuleAccess::ReadWrite => CallableGlobalAccess::ReadWrite,
                    };
                    globals.push(PublishedGlobal {
                        name,
                        access,
                        value: value.map(slug_build_api_v2::published_value::PublishedEdge::Local),
                    });
                }
                DefDependency::Constant(value) => {
                    dependencies.push(self.value(value, lowerer, depth + 1).map_err(|error| {
                        format!(
                            "Args callable {} constant {}: {error}",
                            code.name(),
                            bounded_value_class(value)
                        )
                    })?)
                }
                DefDependency::NativeFunction(native) => {
                    dependencies.push(self.value(native.value(), lowerer, depth + 1).map_err(
                        |error| format!("Args callable {} native dependency: {error}", code.name()),
                    )?)
                }
                DefDependency::Type(_) => {
                    return Err(
                        "Args callable compiled annotation/type values are unsupported".into(),
                    );
                }
                DefDependency::Source(span) => self.sources.verify_source(
                    &source,
                    span.file,
                    &mut self.graph,
                    &mut self.remaining,
                )?,
                DefDependency::NestedCode(nested) => {
                    self.step(depth + 1)?;
                    let authority = self.sources.authority(
                        nested.original_module_client_data(),
                        nested.source(),
                        &mut self.graph,
                        &mut self.remaining,
                    )?;
                    let id = self.graph.reserve().map_err(|e| e.to_string())?;
                    let span = nested.signature_span().span;
                    let empty = self.edges(&[])?;
                    let node = PublishedNode::Code(PublishedCode {
                        source: authority,
                        name: self.text(nested.name())?,
                        definition: CallableDefinitionSpan {
                            start: span.begin().get(),
                            end: span.end().get(),
                        },
                        dependencies: empty,
                        globals: Arc::from([]),
                    });
                    self.set(id, node)?;
                    dependencies.push(id);
                    // The enclosing Engine walk already visits this nested body's dependencies.
                }
            }
        }
        let dependencies = self.edges(&dependencies)?;
        Ok(PublishedCode {
            source,
            name,
            definition: CallableDefinitionSpan {
                start: span.begin().get(),
                end: span.end().get(),
            },
            dependencies,
            globals: globals.into(),
        })
    }
}
