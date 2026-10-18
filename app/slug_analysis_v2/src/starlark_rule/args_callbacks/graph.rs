//! Phase-local descriptor assembly from actual Engine values, never source recipes.
use std::sync::Arc;

use compact_str::CompactString;
use slug_build_api_v2::callable_graph::CALLABLE_GRAPH_DEPTH_LIMIT;
use slug_build_api_v2::callable_graph::CALLABLE_GRAPH_EDGE_LIMIT;
use slug_build_api_v2::callable_graph::CALLABLE_GRAPH_NODE_LIMIT;
use slug_build_api_v2::published_value::PublishedFunction;
use slug_build_api_v2::published_value::PublishedNode;
use slug_build_api_v2::published_value::PublishedNodeId;
use slug_build_api_v2::published_value::PublishedValueArena;
use slug_build_api_v2::published_value::PublishedValueArenaBuilder;
use starlark::environment::Module;
use starlark::eval::inspect_def;
use starlark::eval::inspect_native_bound_method;
use starlark::eval::inspect_native_function;
use starlark::values::UnpackValue;
use starlark::values::Value;
use starlark::values::ValueIdentity;
use starlark::values::dict::DictRef;
use starlark::values::list::ListRef;
use starlark::values::set::SetRef;
use starlark::values::structs::StructRef;
use starlark::values::tuple::TupleRef;
use starlark_map::unordered_map::UnorderedMap;

use super::native::NativeApis;
use super::source::SourceAuthorities;
use crate::analysis_value::AnalysisValueLowerer;

pub(super) struct PublicationBuilder<'v> {
    module: &'v Module,
    graph: PublishedValueArenaBuilder,
    values: UnorderedMap<ValueIdentity<'v>, PublishedNodeId>,
    sources: SourceAuthorities<'v>,
    natives: NativeApis,
    remaining: usize,
    roots: Vec<Value<'v>>,
    value_credit: storage::PeakCredit,
    root_credit: storage::PeakCredit,
}
impl<'v> PublicationBuilder<'v> {
    pub(super) fn new(module: &'v Module) -> Self {
        Self {
            module,
            graph: PublishedValueArenaBuilder::new(),
            values: UnorderedMap::new(),
            sources: SourceAuthorities::default(),
            natives: NativeApis::new(),
            remaining: CALLABLE_GRAPH_NODE_LIMIT,
            roots: Vec::new(),
            value_credit: storage::PeakCredit::default(),
            root_credit: storage::PeakCredit::default(),
        }
    }
    pub(super) fn resource_storage(
        &mut self,
        visits: usize,
        bytes: usize,
        temporary: bool,
    ) -> Result<(), String> {
        self.graph
            .charge_storage(slug_build_api_v2::DepsetStorageCharge {
                nodes: 0,
                edges: 0,
                bytes: 0,
                visits,
            })
            .map_err(|e| e.to_string())?;
        if temporary {
            self.graph
                .producer_scratch()
                .charge_bytes(bytes)
                .map_err(|e| e.to_string())
        } else {
            self.graph.charge_bytes(bytes).map_err(|e| e.to_string())
        }
    }
    pub(super) fn add_callable(
        &mut self,
        value: Value<'v>,
        lowerer: &mut AnalysisValueLowerer<'v>,
    ) -> Result<PublishedNodeId, String> {
        if inspect_def(value, Some(self.module)).is_none()
            && inspect_native_function(value).is_none()
            && inspect_native_bound_method(value).is_none()
            && slug_loading_v2::analysis_builtin_callable_api(value).is_none()
        {
            return Err("Args mapper callable kind has no admitted executable descriptor".into());
        }
        self.value(value, lowerer, 0)
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
    pub(super) fn finish(self) -> (PublishedValueArenaBuilder, Vec<Value<'v>>) {
        (self.graph, self.roots)
    }
    fn edges(
        &mut self,
        nodes: &[PublishedNodeId],
    ) -> Result<slug_build_api_v2::published_value::PublishedEdgeRange, String> {
        let edges = nodes
            .iter()
            .copied()
            .map(slug_build_api_v2::published_value::PublishedEdge::Local)
            .collect::<Vec<_>>();
        self.graph.append_edges(&edges).map_err(|e| e.to_string())
    }
    pub(super) fn add_value(
        &mut self,
        value: Value<'v>,
        lowerer: &mut AnalysisValueLowerer<'v>,
    ) -> Result<PublishedNodeId, String> {
        self.value(value, lowerer, 0)
    }
    fn step(&mut self, depth: usize) -> Result<(), String> {
        self.graph.observe_depth(depth).map_err(|e| e.to_string())?;
        self.graph.charge_visit().map_err(|e| e.to_string())?;
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
    fn set(&mut self, id: PublishedNodeId, node: PublishedNode) -> Result<(), String> {
        self.graph.set_node(id, node).map_err(|e| e.to_string())
    }
    fn value(
        &mut self,
        value: Value<'v>,
        lowerer: &mut AnalysisValueLowerer<'v>,
        depth: usize,
    ) -> Result<PublishedNodeId, String> {
        // One paid encounter owns at most one new ordinary value row. A memo
        // hit drops the borrow without refund or storage allocation.
        let mut record = self.graph.begin_record(depth).map_err(|e| e.to_string())?;
        self.remaining = match self.remaining.checked_sub(1) {
            Some(remaining) => remaining,
            None => {
                record.abort();
                return Err("Args callable inspection visit limit".into());
            }
        };
        if let Some(id) = self.values.get(&value.identity()) {
            return Ok(*id);
        }
        // Preserve both existing aggregate pre-copy credits BEFORE the row and
        // slot arrays can grow. The token exposes only byte admission, not its
        // mutable owner or a reusable prepaid reservation.
        if let Err(error) = self.value_credit.map::<ValueIdentity<'v>, PublishedNodeId>(
            &mut record.producer_scratch(),
            self.values.len().saturating_add(1),
        ) {
            record.abort();
            return Err(error);
        }
        if let Err(error) = self
            .root_credit
            .vector::<Value<'v>>(&mut record, self.roots.len().saturating_add(1))
        {
            record.abort();
            return Err(error);
        }
        let id = record.reserve().map_err(|e| e.to_string())?;
        self.bind_reserved_value(value, id)?;
        if self.depset(value, id, lowerer, depth)? {
            return Ok(id);
        }
        if let Some(constructor) = slug_loading_v2::provider::provider_constructor_view(value) {
            self.provider_constructor(id, constructor, lowerer, depth + 1)?;
            return Ok(id);
        }
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
                    self.value_credit
                        .map::<ValueIdentity<'v>, PublishedNodeId>(
                            &mut self.graph.producer_scratch(),
                            self.values.len().saturating_add(1),
                        )?;
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
                    self.set(
                        cell,
                        PublishedNode::Cell {
                            contents: contents
                                .map(slug_build_api_v2::published_value::PublishedEdge::Local),
                        },
                    )?;
                    cell
                };
                captures.push(cell);
            }
            let captures = self.edges(&captures)?;
            let code = self.code(function.code(), lowerer, depth + 1)?;
            let symbol = self.function_symbol(value, depth + 1)?;
            self.set(
                id,
                PublishedNode::Function(PublishedFunction {
                    symbol,
                    code,
                    parameters: parameters.into(),
                    return_annotation: None,
                    capture_names: names.into(),
                    captures,
                }),
            )?;
        } else if let Some(node) = self.native_callable(value, lowerer, depth + 1)? {
            self.set(id, node)?;
        } else if let Some(node) = self.frontier(value, lowerer, depth + 1)? {
            self.set(id, node)?;
        } else if ListRef::from_value(value).is_some()
            || SetRef::unpack_value_opt(value).is_some()
            || TupleRef::from_value(value).is_some()
            || DictRef::from_value(value).is_some()
            || StructRef::from_value(value).is_some()
            || provider_header(value).is_some()
        {
            let node = self.container(value, lowerer, depth + 1)?;
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
                let native = lowerer.lower(value, "published native value")?;
                let atom = self.graph.native_atom(native).map_err(|e| e.to_string())?;
                self.set(id, PublishedNode::Native(atom))?;
                return Ok(id);
            }
            let node = self.container(value, lowerer, depth + 1)?;
            self.set(id, node)?;
        }
        Ok(id)
    }
    fn reserve_value(&mut self, value: Value<'v>) -> Result<PublishedNodeId, String> {
        self.value_credit
            .map::<ValueIdentity<'v>, PublishedNodeId>(
                &mut self.graph.producer_scratch(),
                self.values.len().saturating_add(1),
            )?;
        self.root_credit
            .vector::<Value<'v>>(&mut self.graph, self.roots.len().saturating_add(1))?;
        let id = self.graph.reserve().map_err(|e| e.to_string())?;
        self.bind_reserved_value(value, id)?;
        Ok(id)
    }
    /// Bind only after the selected caller has admitted map/root peaks and its
    /// ordinary or encounter-composed row. Typed depset frames keep reserve_value.
    fn bind_reserved_value(&mut self, value: Value<'v>, id: PublishedNodeId) -> Result<(), String> {
        let slot = slug_build_api_v2::published_value::PublishedRootSlot::new(self.roots.len())
            .map_err(|e| e.to_string())?;
        self.values.insert(value.identity(), id);
        self.roots.push(value);
        self.graph
            .set_original_slot(id, slot)
            .map_err(|e| e.to_string())?;
        Ok(())
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
    ) -> Result<PublishedNode, String> {
        self.step(depth)?;
        if let Some(set) = SetRef::unpack_value_opt(value) {
            self.membership(set.len())?;
            let mut edges = Vec::with_capacity(set.len());
            for child in set.iter() {
                edges.push(self.value(child, lowerer, depth + 1)?);
            }
            return Ok(PublishedNode::Set(self.edges(&edges)?));
        }
        if let Some(list) = ListRef::from_value(value) {
            self.membership(list.content().len())?;
            let mut edges = Vec::new();
            for child in list.iter() {
                edges.push(self.value(child, lowerer, depth + 1)?);
            }
            return Ok(PublishedNode::List(self.edges(&edges)?));
        }
        if let Some(tuple) = TupleRef::from_value(value) {
            self.membership(tuple.len())?;
            let mut edges = Vec::new();
            for child in tuple.iter() {
                edges.push(self.value(child, lowerer, depth + 1)?);
            }
            return Ok(PublishedNode::Tuple(self.edges(&edges)?));
        }
        if let Some(dict) = DictRef::from_value(value) {
            self.membership(dict.iter().len().saturating_mul(2))?;
            let mut edges = Vec::new();
            for (k, v) in dict.iter() {
                edges.push(self.value(k, lowerer, depth + 1)?);
                edges.push(self.value(v, lowerer, depth + 1)?);
            }
            return Ok(PublishedNode::Dictionary(self.edges(&edges)?));
        }
        if let Some(fields) = StructRef::from_value(value) {
            self.membership(fields.iter().len())?;
            let mut names = Vec::new();
            let mut edges = Vec::new();
            for (name, v) in fields.iter() {
                names.push(self.text(name.as_str())?);
                edges.push(self.value(v, lowerer, depth + 1)?);
            }
            return Ok(PublishedNode::Struct {
                names: names.into(),
                values: self.edges(&edges)?,
            });
        }
        if let Some((identity, count, has_schema, hidden)) = provider_header(value) {
            self.membership(count)?;
            let mut fields = Vec::with_capacity(count);
            visit_provider_fields(value, &mut |name, child| {
                fields.push((self.text(name)?, child));
                Ok(())
            })?;
            fields.sort_by(|(a, _), (b, _)| a.cmp(b));
            let mut names = Vec::with_capacity(count);
            let mut edges = Vec::with_capacity(count);
            for (name, child) in fields {
                names.push(name);
                edges.push(self.value(child, lowerer, depth + 1)?);
            }
            let files_to_run = hidden
                .map(|value| {
                    self.graph
                        .files_to_run_atom_borrowed(value)
                        .map_err(|e| e.to_string())
                })
                .transpose()?;
            return Ok(PublishedNode::Provider(
                slug_build_api_v2::published_value::PublishedProvider {
                    identity,
                    names: names.into(),
                    values: self.edges(&edges)?,
                    has_schema,
                    files_to_run,
                },
            ));
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
use diagnostic::provider_header;
use diagnostic::visit_provider_fields;

#[cfg(test)]
mod tests;

mod compiled;
mod diagnostic;
mod frontier;
mod native_callable;
mod provider_constructor;

mod depset;

mod storage;

#[cfg(test)]
#[path = "graph/record_tests.rs"]
mod record_tests;

#[cfg(test)]
#[path = "graph/resource_set_quota_tests.rs"]
mod resource_set_quota_tests;
