use super::*;
use crate::AnalysisValue;
use crate::AnalysisValueKind;
use crate::ProviderOccurrence;

pub(super) fn value_type(row: &PublishedNode) -> AnalysisValueType {
    match row {
        PublishedNode::Function(_) | PublishedNode::Intrinsic(_) => AnalysisValueType::Function,
        PublishedNode::ProviderConstructor(_) => AnalysisValueType::ProviderConstructor,
        PublishedNode::RawProviderConstructor { .. } => AnalysisValueType::RawProviderConstructor,
        PublishedNode::Set(_) => AnalysisValueType::Set,
        PublishedNode::List(_) => AnalysisValueType::List,
        PublishedNode::Tuple(_) => AnalysisValueType::Tuple,
        PublishedNode::Dictionary(_) => AnalysisValueType::Dictionary,
        PublishedNode::Struct { .. } => AnalysisValueType::Struct,
        PublishedNode::Provider(p) if p.identity.is_builtin("ToolchainInfo") => {
            AnalysisValueType::ToolchainInfo
        }
        PublishedNode::Provider(_) => AnalysisValueType::Struct,
        PublishedNode::ConfiguredTarget { .. } => AnalysisValueType::ConfiguredTarget,
        PublishedNode::Depset(_) => AnalysisValueType::Depset,
        PublishedNode::CcVariables(_) => AnalysisValueType::CcToolchainVariables,
        PublishedNode::Native(atom) => atom.value().value_type(),
        PublishedNode::Code(_) | PublishedNode::Cell { .. } => AnalysisValueType::Function,
    }
}
fn range<'a>(
    edges: &'a [PublishedEdge],
    r: PublishedEdgeRange,
) -> Result<&'a [PublishedEdge], PublishedValueError> {
    edges
        .get(
            r.start as usize
                ..(r.start as usize)
                    .checked_add(r.len as usize)
                    .ok_or(PublishedValueError::Invalid("range"))?,
        )
        .ok_or(PublishedValueError::Invalid("range"))
}
pub(super) fn visit_edges(
    row: &PublishedNode,
    edges: &[PublishedEdge],
    containment: bool,
    mut f: impl FnMut(PublishedEdge) -> Result<(), PublishedValueError>,
) -> Result<(), PublishedValueError> {
    let mut ranges = [None, None];
    match row {
        PublishedNode::Function(fun) if !containment => {
            ranges = [Some(fun.code.dependencies), Some(fun.captures)];
            for global in &*fun.code.globals {
                if let Some(e) = global.value {
                    f(e)?;
                }
            }
            parameters(&fun.parameters, &mut f)?;
            if let Some(e) = fun.return_annotation {
                f(e)?;
            }
        }
        PublishedNode::Intrinsic(i) if !containment => {
            parameters(&i.parameters, &mut f)?;
            if let Some(e) = i.return_annotation {
                f(e)?;
            }
            if let Some(e) = i.receiver {
                f(e)?;
            }
        }
        PublishedNode::Code(c) if !containment => {
            ranges[0] = Some(c.dependencies);
            for global in &*c.globals {
                if let Some(e) = global.value {
                    f(e)?;
                }
            }
        }
        PublishedNode::Cell { contents: Some(e) } if !containment => f(*e)?,
        PublishedNode::ProviderConstructor(c) if !containment => {
            if let Some(e) = c.init {
                f(e)?;
            }
        }
        PublishedNode::RawProviderConstructor { provider, .. } if !containment => f(*provider)?,
        PublishedNode::Set(r)
        | PublishedNode::List(r)
        | PublishedNode::Tuple(r)
        | PublishedNode::Dictionary(r) => ranges[0] = Some(*r),
        PublishedNode::Struct { values, .. } => ranges[0] = Some(*values),
        PublishedNode::Provider(p) => ranges[0] = Some(p.values),
        PublishedNode::ConfiguredTarget { providers, .. } => {
            for p in &**providers {
                f(match p {
                    PublishedTargetProvider::Native { view, .. }
                    | PublishedTargetProvider::Occurrence(view) => *view,
                })?;
            }
        }
        PublishedNode::Depset(d) => {
            for input in &*d.successors {
                f(match input {
                    PublishedDepsetInput::Direct(e) | PublishedDepsetInput::Transitive(e) => *e,
                })?;
            }
        }
        PublishedNode::CcVariables(v) => {
            if let Some(e) = v.parent {
                f(e)?;
            }
            for (_, e) in &*v.locals {
                f(*e)?;
            }
            for (_, seed) in &*v.seed {
                match seed {
                    PublishedVariableSeed::Value(e) => f(*e)?,
                    PublishedVariableSeed::Sequence(r) => {
                        for e in range(edges, *r)? {
                            f(*e)?;
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    for r in ranges.into_iter().flatten() {
        for e in range(edges, r)? {
            f(*e)?;
        }
    }
    Ok(())
}
fn parameters(
    params: &[PublishedParameter],
    f: &mut impl FnMut(PublishedEdge) -> Result<(), PublishedValueError>,
) -> Result<(), PublishedValueError> {
    for p in params {
        if let Some(e) = p.annotation {
            f(e)?;
        }
        if let PublishedParameterRequirement::Defaulted(e) = p.requirement {
            f(e)?;
        }
    }
    Ok(())
}
pub(super) fn inspect_row(
    row: &PublishedNode,
    edges: &[PublishedEdge],
    mut check: impl FnMut(PublishedEdge) -> Result<(), PublishedValueError>,
) -> Result<(usize, usize), PublishedValueError> {
    let mut count = 0usize;
    visit_edges(row, edges, false, |e| {
        count = count
            .checked_add(1)
            .ok_or(PublishedValueError::Limit("row edges"))?;
        if count > EDGE_LIMIT {
            return Err(PublishedValueError::Limit("row edges"));
        }
        check(e)
    })?;
    let names = |names: &[compact_str::CompactString],
                 r: PublishedEdgeRange|
     -> Result<usize, PublishedValueError> {
        if names.len() > NODE_LIMIT {
            return Err(PublishedValueError::Limit("names"));
        }
        if names.len() != range(edges, r)?.len() {
            return Err(PublishedValueError::Invalid("field count"));
        }
        let mut seen = fxhash::FxHashSet::default();
        let mut bytes = 0usize;
        for name in names {
            if !seen.insert(name.as_str()) {
                return Err(PublishedValueError::Invalid("duplicate field"));
            }
            bytes = bytes
                .checked_add(name.len() + std::mem::size_of::<compact_str::CompactString>())
                .ok_or(PublishedValueError::Limit("names"))?;
        }
        Ok(bytes)
    };
    let mut bytes = 0usize;
    match row {
        PublishedNode::Dictionary(r) if range(edges, *r)?.len() % 2 != 0 => {
            return Err(PublishedValueError::Invalid("dictionary pairs"));
        }
        PublishedNode::Struct { names: n, values } => {
            bytes = bytes.saturating_add(names(n, *values)?)
        }
        PublishedNode::Provider(p) => bytes = bytes.saturating_add(names(&p.names, p.values)?),
        PublishedNode::Function(f) => {
            bytes = bytes.saturating_add(names(&f.capture_names, f.captures)?);
            bytes = bytes.saturating_add(code_bytes(&f.code));
            bytes = bytes.saturating_add(parameter_bytes(&f.parameters));
        }
        PublishedNode::Code(c) => bytes = bytes.saturating_add(code_bytes(c)),
        PublishedNode::Intrinsic(i) => {
            bytes = bytes
                .saturating_add(i.api.canonical().len())
                .saturating_add(parameter_bytes(&i.parameters))
        }
        PublishedNode::ProviderConstructor(c) => {
            if matches!(c.schema, PublishedProviderSchema::Initialized(_)) != c.init.is_some() {
                return Err(PublishedValueError::Invalid("constructor initializer"));
            }
            let n = match &c.schema {
                PublishedProviderSchema::Schemaless => &[][..],
                PublishedProviderSchema::List(n)
                | PublishedProviderSchema::Documented(n)
                | PublishedProviderSchema::Initialized(n) => &**n,
            };
            let mut seen = fxhash::FxHashSet::default();
            for name in n {
                if !seen.insert(name) {
                    return Err(PublishedValueError::Invalid("constructor schema"));
                }
                bytes = bytes
                    .saturating_add(name.len())
                    .saturating_add(std::mem::size_of::<compact_str::CompactString>());
            }
        }
        PublishedNode::Depset(d) => {
            if d.depth > crate::depset::MAX_DEPTH {
                return Err(PublishedValueError::Limit("depset depth"));
            }
            bytes = bytes.saturating_add(
                d.successors
                    .len()
                    .saturating_mul(std::mem::size_of::<PublishedDepsetInput>()),
            );
        }
        PublishedNode::CcVariables(v) => {
            for (n, _) in &*v.locals {
                bytes = bytes
                    .saturating_add(n.len())
                    .saturating_add(std::mem::size_of::<(
                        compact_str::CompactString,
                        PublishedEdge,
                    )>());
            }
            for (n, s) in &*v.seed {
                bytes = bytes
                    .saturating_add(n.len())
                    .saturating_add(std::mem::size_of::<PublishedVariableSeed>());
                if let PublishedVariableSeed::Error(e) = s {
                    bytes = bytes.saturating_add(e.len());
                }
            }
        }
        _ => {}
    }
    if bytes > BYTE_LIMIT {
        return Err(PublishedValueError::Limit("row bytes"));
    }
    Ok((bytes, count))
}
fn code_bytes(c: &PublishedCode) -> usize {
    c.globals.iter().fold(c.name.len(), |n, g| {
        n.saturating_add(g.name.len())
            .saturating_add(std::mem::size_of::<PublishedGlobal>())
    })
}
fn parameter_bytes(p: &[PublishedParameter]) -> usize {
    p.iter().fold(0usize, |n, p| {
        n.saturating_add(p.name.len())
            .saturating_add(std::mem::size_of::<PublishedParameter>())
    })
}
pub(super) fn validate_complete(
    rows: &[PublishedNode],
    edges: &[PublishedEdge],
    imports: &[PublishedValueArena],
) -> Result<(usize, usize), PublishedValueError> {
    let resolve = |e: PublishedEdge| -> Result<&PublishedNode, PublishedValueError> {
        match e {
            PublishedEdge::Local(n) => rows
                .get(n.index())
                .ok_or(PublishedValueError::Invalid("node")),
            PublishedEdge::Imported { arena, node } => imports
                .get(arena.index())
                .ok_or(PublishedValueError::Invalid("import"))?
                .node(node),
        }
    };
    for row in rows {
        if let PublishedNode::RawProviderConstructor { provider, .. } = row {
            if !matches!(resolve(*provider)?, PublishedNode::ProviderConstructor(c) if matches!(c.schema,PublishedProviderSchema::Initialized(_)))
            {
                return Err(PublishedValueError::Invalid("raw provider target"));
            }
        }
    }
    depth::validate(rows, edges, imports)
}

pub(crate) fn validate_value(
    value: &AnalysisValue,
    origin: &PublishedValueRef,
) -> Result<(), PublishedValueError> {
    if origin.original_slot().is_none() || value.value_type() != origin.value_type() {
        return Err(PublishedValueError::Invalid("projection type/origin"));
    }
    if value.original_origin().is_some() {
        return Err(PublishedValueError::Invalid("nested origin"));
    }
    match (value.kind(), origin.node()) {
        (AnalysisValueKind::Function(a), PublishedNode::Function(b)) if a == &b.symbol => Ok(()),
        (AnalysisValueKind::ProviderConstructor(a), PublishedNode::ProviderConstructor(b))
            if a.id == b.id && schema_eq(&a.schema, &b.schema) =>
        {
            Ok(())
        }
        (
            AnalysisValueKind::RawProviderConstructor(a),
            PublishedNode::RawProviderConstructor {
                provider,
                occurrence,
            },
        ) if &a.occurrence == occurrence => match origin.arena().resolve(*provider)?.node() {
            PublishedNode::ProviderConstructor(b) if a.provider == b.id => Ok(()),
            _ => Err(PublishedValueError::Invalid("raw projection")),
        },
        (_, PublishedNode::Native(atom)) if native_projection_matches(value, atom.value()) => {
            Ok(())
        }
        (AnalysisValueKind::List(a), PublishedNode::List(r))
        | (AnalysisValueKind::Tuple(a), PublishedNode::Tuple(r)) => children(a, origin, *r),
        (AnalysisValueKind::Dictionary(a), PublishedNode::Dictionary(r)) => {
            let e = origin.arena().edges(*r)?;
            if e.len() != a.len() * 2 {
                return Err(PublishedValueError::Invalid("dict projection"));
            }
            for ((k, v), pair) in a.iter().zip(e.chunks_exact(2)) {
                child(k, origin, pair[0])?;
                child(v, origin, pair[1])?;
            }
            Ok(())
        }
        (AnalysisValueKind::Struct(a), PublishedNode::Struct { names, values }) => {
            let e = origin.arena().edges(*values)?;
            if a.len() != names.len() {
                return Err(PublishedValueError::Invalid("struct projection"));
            }
            for (name, edge) in names.iter().zip(e) {
                let value = a
                    .get(name.as_str())
                    .ok_or(PublishedValueError::Invalid("struct field"))?;
                child(value, origin, *edge)?;
            }
            Ok(())
        }
        (AnalysisValueKind::Provider(p), PublishedNode::Provider(_)) => {
            validate_provider(p, origin)
        }
        (AnalysisValueKind::Depset(d), PublishedNode::Depset(_)) => validate_depset(d, origin, 0),
        (AnalysisValueKind::CcToolchainVariables(v), PublishedNode::CcVariables(_)) => {
            validate_variables(v, origin, 0)
        }
        (
            AnalysisValueKind::ConfiguredTarget(t),
            PublishedNode::ConfiguredTarget {
                identity,
                providers,
            },
        ) => {
            if t.identity() != identity || t.providers().len() != providers.len() {
                return Err(PublishedValueError::Invalid("target projection"));
            }
            let mut state = crate::analysis_value::PublicationEqState::default();
            for ((_, actual), expected) in t.providers().iter().zip(providers.iter()) {
                match (actual, expected) {
                    (
                        crate::ProviderValue::Occurrence(p),
                        PublishedTargetProvider::Occurrence(e),
                    ) => {
                        let expected = origin.arena().resolve(*e)?;
                        if p.original_origin()
                            .is_none_or(|actual| !actual.same_original(&expected))
                        {
                            return Err(PublishedValueError::Invalid("target occurrence origin"));
                        }
                    }
                    (_, PublishedTargetProvider::Native { provider, .. })
                        if actual.publication_eq_with(provider.value(), &mut state) => {}
                    _ => return Err(PublishedValueError::Invalid("target typed provider")),
                }
            }
            Ok(())
        }
        _ => Err(PublishedValueError::Unsupported("native projection kind")),
    }
}
pub(super) fn child(
    value: &AnalysisValue,
    parent: &PublishedValueRef,
    edge: PublishedEdge,
) -> Result<(), PublishedValueError> {
    let expected = parent.arena().resolve(edge)?;
    if let Some(actual) = value.original_origin() {
        if actual.key() == expected.key() {
            return Ok(());
        }
    }
    if let PublishedNode::Native(atom) = expected.node() {
        if native_projection_matches(value, atom.value()) {
            return Ok(());
        }
    }
    Err(PublishedValueError::Invalid("projection child origin"))
}

// Language-equal versions can have different observable spellings. A native
// projection must preserve the complete model, including that spelling.
fn native_projection_matches(value: &AnalysisValue, expected: &AnalysisValue) -> bool {
    match (value.kind(), expected.kind()) {
        (AnalysisValueKind::DottedVersion(left), AnalysisValueKind::DottedVersion(right)) => {
            left == right
        }
        _ => value == expected,
    }
}
fn children(
    values: &[AnalysisValue],
    parent: &PublishedValueRef,
    range: PublishedEdgeRange,
) -> Result<(), PublishedValueError> {
    let edges = parent.arena().edges(range)?;
    if values.len() != edges.len() {
        return Err(PublishedValueError::Invalid("projection children"));
    }
    for (v, e) in values.iter().zip(edges) {
        child(v, parent, *e)?;
    }
    Ok(())
}
pub(crate) fn validate_provider(
    p: &ProviderOccurrence,
    origin: &PublishedValueRef,
) -> Result<(), PublishedValueError> {
    let PublishedNode::Provider(row) = origin.node() else {
        return Err(PublishedValueError::Invalid("provider origin"));
    };
    if p.identity() != &row.identity
        || p.has_schema() != row.has_schema
        || p.fields().len() != row.names.len()
        || origin.original_slot().is_none()
    {
        return Err(PublishedValueError::Invalid("provider projection"));
    }
    let edges = origin.arena().edges(row.values)?;
    for ((name, value), (expected, edge)) in p.fields().iter().zip(row.names.iter().zip(edges)) {
        if name != expected {
            return Err(PublishedValueError::Invalid("provider field"));
        }
        child(value, origin, *edge)?;
    }
    match (p.files_to_run(), &row.files_to_run) {
        (None, None) => Ok(()),
        (Some(a), Some(b)) if a.as_ref() == b.value() => Ok(()),
        _ => Err(PublishedValueError::Invalid("hidden provider projection")),
    }
}
pub(super) fn schema_eq(a: &PublishedProviderSchema, b: &PublishedProviderSchema) -> bool {
    match (a, b) {
        (PublishedProviderSchema::Schemaless, PublishedProviderSchema::Schemaless) => true,
        (PublishedProviderSchema::List(a), PublishedProviderSchema::List(b))
        | (PublishedProviderSchema::Documented(a), PublishedProviderSchema::Documented(b))
        | (PublishedProviderSchema::Initialized(a), PublishedProviderSchema::Initialized(b)) => {
            a == b
        }
        _ => false,
    }
}

#[path = "projection_children.rs"]
mod projection_children;
use projection_children::validate_depset;
use projection_children::validate_variables;

#[path = "projection_depth.rs"]
mod depth;
pub(super) use depth::draft_scratch_bytes as depth_scratch_bytes;
