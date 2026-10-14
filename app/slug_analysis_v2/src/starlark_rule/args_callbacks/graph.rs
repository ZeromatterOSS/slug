//! Phase-local descriptor assembly from actual Engine values, never source recipes.
use std::sync::Arc;

use compact_str::CompactString;
use slug_build_api_v2::CallableCode;
use slug_build_api_v2::CallableDefinitionSpan;
use slug_build_api_v2::CallableFunction;
use slug_build_api_v2::CallableGlobal;
use slug_build_api_v2::CallableGlobalAccess;
use slug_build_api_v2::CallableGraph;
use slug_build_api_v2::CallableGraphBuilder;
use slug_build_api_v2::CallableNode;
use slug_build_api_v2::CallableNodeId;
use slug_build_api_v2::CallableParameter;
use slug_build_api_v2::CallableParameterMode;
use slug_build_api_v2::CallableParameterRequirement;
use slug_build_api_v2::callable_graph::CALLABLE_GRAPH_DEPTH_LIMIT;
use slug_build_api_v2::callable_graph::CALLABLE_GRAPH_EDGE_LIMIT;
use slug_build_api_v2::callable_graph::CALLABLE_GRAPH_NODE_LIMIT;
use starlark::environment::Module;
use starlark::eval::DefCodeInspection;
use starlark::eval::DefDependency;
use starlark::eval::DefParameter;
use starlark::eval::InspectionBudget;
use starlark::eval::ModuleAccess;
use starlark::eval::ParameterRequirement;
use starlark::eval::ParameterStyle;
use starlark::eval::inspect_def;
use starlark::eval::inspect_native_bound_method;
use starlark::eval::inspect_native_function;
use starlark::values::Value;
use starlark::values::ValueIdentity;
use starlark::values::dict::DictRef;
use starlark::values::list::ListRef;
use starlark::values::structs::StructRef;
use starlark::values::tuple::TupleRef;
use starlark_map::unordered_map::UnorderedMap;

use super::native::NativeApis;
use super::native::validate_atom;
use super::source::SourceAuthorities;
use crate::analysis_value::AnalysisValueLowerer;

pub(super) struct ArgsCallableGraphBuilder<'v> {
    module: &'v Module,
    graph: CallableGraphBuilder,
    values: UnorderedMap<ValueIdentity<'v>, CallableNodeId>,
    sources: SourceAuthorities<'v>,
    natives: NativeApis,
    remaining: usize,
}
impl<'v> ArgsCallableGraphBuilder<'v> {
    pub(super) fn new(module: &'v Module) -> Self {
        Self {
            module,
            graph: CallableGraphBuilder::new(),
            values: UnorderedMap::new(),
            sources: SourceAuthorities::default(),
            natives: NativeApis::new(),
            remaining: CALLABLE_GRAPH_NODE_LIMIT,
        }
    }
    pub(super) fn add_callable(
        &mut self,
        value: Value<'v>,
        lowerer: &mut AnalysisValueLowerer<'v>,
    ) -> Result<CallableNodeId, String> {
        if inspect_def(value, Some(self.module)).is_none()
            && inspect_native_function(value).is_none()
            && inspect_native_bound_method(value).is_none()
            && slug_loading_v2::analysis_builtin_callable_api(value).is_none()
        {
            return Err("Args mapper callable kind has no admitted executable descriptor".into());
        }
        self.value(value, lowerer, 0)
    }
    /// Bound borrowed native input work before the shared lowerer allocates a
    /// snapshot. Function-containing inputs are an explicit initial capability
    /// boundary; actual native atom aliases retain the caller's lowerer memo.
    pub(super) fn lower_input(
        &mut self,
        value: Value<'v>,
        lowerer: &mut AnalysisValueLowerer<'v>,
    ) -> Result<slug_build_api_v2::AnalysisValue, String> {
        if !super::native::pure_candidate(
            self.module,
            &mut self.graph,
            &mut self.remaining,
            value,
            0,
        )? {
            return Err("Args input contains unsupported callable or cyclic values".into());
        }
        let native = lowerer.lower(value, "Args input")?;
        validate_atom(&native, &mut self.remaining, 0)?;
        Ok(native)
    }

    /// Audit all roots the actual mutable module freeze will retain. Extra scratch
    /// roots are separately sanitized by the publication owner before freezing.
    pub(super) fn audit_module(
        &mut self,
        lowerer: &mut AnalysisValueLowerer<'v>,
    ) -> Result<(), String> {
        if let Some(client_data) = self.module.client_data() {
            if slug_loading_v2::defining_module_source_manifest(client_data).is_none() {
                return Err("Args publication cannot retain unknown module client metadata".into());
            }
        }
        let module = self.module;
        module
            .visit_assigned_values(&mut |value| {
                self.value(value, lowerer, 0)
                    .map(|_| ())
                    .map_err(host_error)
            })
            .map_err(|e| e.to_string())
    }
    pub(super) fn finish(self) -> Result<CallableGraph, String> {
        self.graph.finish().map_err(|e| e.to_string())
    }
    fn step(&mut self, depth: usize) -> Result<(), String> {
        self.graph.observe_depth(depth).map_err(|e| e.to_string())?;
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or("Args callable inspection visit limit")?;
        Ok(())
    }
    fn text(&mut self, text: &str) -> Result<CompactString, String> {
        self.graph
            .charge_bytes(text.len())
            .map_err(|e| e.to_string())?;
        Ok(text.into())
    }
    fn set(&mut self, id: CallableNodeId, node: CallableNode) -> Result<(), String> {
        self.graph.set_node(id, node).map_err(|e| e.to_string())
    }
    fn value(
        &mut self,
        value: Value<'v>,
        lowerer: &mut AnalysisValueLowerer<'v>,
        depth: usize,
    ) -> Result<CallableNodeId, String> {
        self.step(depth)?;
        if let Some(id) = self.values.get(&value.identity()) {
            return Ok(*id);
        }
        let id = self.graph.reserve().map_err(|e| e.to_string())?;
        self.values.insert(value.identity(), id); // before descendants, preserving cycles
        if let Some(function) = inspect_def(value, Some(self.module)) {
            let parameters = self
                .parameters(function.parameters(), lowerer, depth + 1)
                .map_err(|error| {
                    format!(
                        "Args callable {} parameters: {error}",
                        function.code().name()
                    )
                })?;
            if function.return_annotation().is_some() {
                return Err("Args callable return annotations are unsupported".into());
            }
            let mut names = Vec::new();
            let mut captures = Vec::new();
            if function.captures().len() > self.remaining {
                return Err("Args capture count limit".into());
            }
            for capture in function.captures() {
                self.step(depth + 1)?;
                names.push(self.text(capture.child_name)?);
                let cell = if let Some(cell) = self.values.get(&capture.cell.identity()) {
                    *cell
                } else {
                    let cell = self.graph.reserve().map_err(|e| e.to_string())?;
                    self.values.insert(capture.cell.identity(), cell);
                    let contents = capture
                        .contents
                        .map(|value| {
                            self.value(value, lowerer, depth + 2).map_err(|error| {
                                format!(
                                    "Args callable {} capture {}: {error}",
                                    function.code().name(),
                                    capture.child_name
                                )
                            })
                        })
                        .transpose()?;
                    self.set(cell, CallableNode::Cell { contents })?;
                    cell
                };
                captures.push(cell);
            }
            let captures = self
                .graph
                .append_edges(&captures)
                .map_err(|e| e.to_string())?;
            let code = self.code(function.code(), lowerer, depth + 1)?;
            self.set(
                id,
                CallableNode::Function(CallableFunction {
                    code,
                    parameters: parameters.into(),
                    return_annotation: None,
                    capture_names: names.into(),
                    captures,
                }),
            )?;
        } else if let Some(node) = self.native_callable(value, lowerer, depth + 1)? {
            self.set(id, node)?;
        } else {
            // This bounded borrowed audit runs BEFORE lowerer creates native copies.
            let pure = super::native::pure_candidate(
                self.module,
                &mut self.graph,
                &mut self.remaining,
                value,
                depth + 1,
            )?;
            if pure {
                if let Ok(native) = lowerer.lower(value, "Args callback native capture") {
                    if validate_atom(&native, &mut self.remaining, depth + 1).is_ok() {
                        self.set(id, CallableNode::Native(native))?;
                        return Ok(id);
                    }
                }
            }
            let node = self.container(value, lowerer, depth + 1)?;
            self.set(id, node)?;
        }
        Ok(id)
    }
    fn parameters(
        &mut self,
        parameters: impl ExactSizeIterator<Item = DefParameter<'v>>,
        lowerer: &mut AnalysisValueLowerer<'v>,
        depth: usize,
    ) -> Result<Vec<CallableParameter>, String> {
        if parameters.len() > CALLABLE_GRAPH_NODE_LIMIT {
            return Err("Args parameter count limit".into());
        }
        self.graph
            .charge_bytes(
                parameters
                    .len()
                    .saturating_mul(std::mem::size_of::<CallableParameter>()),
            )
            .map_err(|e| e.to_string())?;
        let mut result = Vec::new();
        for parameter in parameters {
            self.step(depth)?;
            if parameter.annotation.is_some() {
                return Err("Args callable parameter annotations are unsupported".into());
            }
            let requirement = match parameter.requirement {
                ParameterRequirement::Required => CallableParameterRequirement::Required,
                ParameterRequirement::Optional => CallableParameterRequirement::Optional,
                ParameterRequirement::Defaulted(value) => CallableParameterRequirement::Defaulted(
                    self.value(value, lowerer, depth + 1).map_err(|error| {
                        format!("Args parameter {} default: {error}", parameter.name)
                    })?,
                ),
                ParameterRequirement::Variadic => CallableParameterRequirement::Variadic,
            };
            let mode = match parameter.style {
                ParameterStyle::PositionalOnly => CallableParameterMode::PositionalOnly,
                ParameterStyle::PositionalOrNamed => CallableParameterMode::PositionalOrNamed,
                ParameterStyle::NamedOnly => CallableParameterMode::NamedOnly,
                ParameterStyle::Args => CallableParameterMode::Args,
                ParameterStyle::Kwargs => CallableParameterMode::Kwargs,
            };
            result.push(CallableParameter {
                name: self.text(parameter.name)?,
                mode,
                requirement,
                annotation: None,
            });
        }
        Ok(result)
    }
    fn code(
        &mut self,
        code: DefCodeInspection<'v>,
        lowerer: &mut AnalysisValueLowerer<'v>,
        depth: usize,
    ) -> Result<CallableCode, String> {
        self.step(depth)?;
        let source = self.sources.authority(
            code.original_module_client_data(),
            code.source(),
            &mut self.graph,
            &mut self.remaining,
        )?;
        let name = self.text(code.name())?;
        let span = code.signature_span().span;
        let mut budget = InspectionBudget {
            remaining_nodes: self.remaining,
            maximum_depth: CALLABLE_GRAPH_DEPTH_LIMIT.saturating_sub(depth),
        };
        // Engine streams exhaustively. This bounded host scratch queue permits
        // recursive callable expansion after releasing the walker's budget borrow.
        let mut events = Vec::new();
        code.visit_dependencies(&mut budget, &mut |event| {
            if events.len() >= CALLABLE_GRAPH_EDGE_LIMIT {
                return Err(host_error("Args dependency event limit".into()));
            }
            events.push(event);
            Ok(())
        })
        .map_err(|e| e.to_string())?;
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
                    globals.push(CallableGlobal {
                        name,
                        access,
                        value,
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
                    let empty = self.graph.append_edges(&[]).map_err(|e| e.to_string())?;
                    let node = CallableNode::Code(CallableCode {
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
        let dependencies = self
            .graph
            .append_edges(&dependencies)
            .map_err(|e| e.to_string())?;
        Ok(CallableCode {
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
    fn membership(&mut self, count: usize) -> Result<(), String> {
        if count > CALLABLE_GRAPH_EDGE_LIMIT || count > self.remaining {
            return Err("Args borrowed container membership limit".into());
        }
        self.graph
            .charge_bytes(count.saturating_mul(std::mem::size_of::<Value<'v>>()))
            .map_err(|e| e.to_string())
    }
    fn container(
        &mut self,
        value: Value<'v>,
        lowerer: &mut AnalysisValueLowerer<'v>,
        depth: usize,
    ) -> Result<CallableNode, String> {
        self.step(depth)?;
        if let Some(list) = ListRef::from_value(value) {
            self.membership(list.content().len())?;
            let mut edges = Vec::new();
            for child in list.iter() {
                edges.push(self.value(child, lowerer, depth + 1)?);
            }
            return Ok(CallableNode::List(
                self.graph.append_edges(&edges).map_err(|e| e.to_string())?,
            ));
        }
        if let Some(tuple) = TupleRef::from_value(value) {
            self.membership(tuple.len())?;
            let mut edges = Vec::new();
            for child in tuple.iter() {
                edges.push(self.value(child, lowerer, depth + 1)?);
            }
            return Ok(CallableNode::Tuple(
                self.graph.append_edges(&edges).map_err(|e| e.to_string())?,
            ));
        }
        if let Some(dict) = DictRef::from_value(value) {
            self.membership(dict.iter().len().saturating_mul(2))?;
            let mut edges = Vec::new();
            for (k, v) in dict.iter() {
                edges.push(self.value(k, lowerer, depth + 1)?);
                edges.push(self.value(v, lowerer, depth + 1)?);
            }
            return Ok(CallableNode::Dict(
                self.graph.append_edges(&edges).map_err(|e| e.to_string())?,
            ));
        }
        if let Some(fields) = StructRef::from_value(value) {
            self.membership(fields.iter().len())?;
            let mut names = Vec::new();
            let mut edges = Vec::new();
            for (name, v) in fields.iter() {
                names.push(self.text(name.as_str())?);
                edges.push(self.value(v, lowerer, depth + 1)?);
            }
            return Ok(CallableNode::Struct {
                names: names.into(),
                values: self.graph.append_edges(&edges).map_err(|e| e.to_string())?,
            });
        }
        if let Some((identity, fields)) = provider_fields(value) {
            self.membership(fields.len())?;
            let mut names = Vec::new();
            let mut edges = Vec::new();
            for (name, v) in fields {
                names.push(self.text(&name)?);
                edges.push(self.value(v, lowerer, depth + 1)?);
            }
            return Ok(CallableNode::Provider {
                identity,
                names: names.into(),
                values: self.graph.append_edges(&edges).map_err(|e| e.to_string())?,
                has_schema: slug_loading_v2::provider::starlark_user_provider_has_schema(value)
                    .unwrap_or(false),
            });
        }
        Err(format!(
            "Args capture contains an unsupported value/capability of type `{}` ({})",
            value.get_type(),
            bounded_value_class(value)
        ))
    }
}
use diagnostic::bounded_value_class;
use diagnostic::host_error;
pub(super) use diagnostic::provider_fields;

#[cfg(test)]
mod tests;

mod diagnostic;
mod native_callable;
