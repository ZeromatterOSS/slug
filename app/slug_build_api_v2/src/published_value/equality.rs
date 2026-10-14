use super::*;
use crate::analysis_value::PublicationEqState;

pub(super) fn equal(
    left: &PublishedValueRef,
    right: &PublishedValueRef,
    state: &mut PublicationEqState,
) -> bool {
    let mut work = vec![(left.clone(), right.clone())];
    while let Some((l, r)) = work.pop() {
        match state.enter_callable_node_pair(l.key(), r.key()) {
            Err(()) => return false,
            Ok(false) => continue,
            Ok(true) => {}
        }
        if !row(&l, &r, state, &mut work) {
            return false;
        }
    }
    true
}
fn edge(
    l: &PublishedValueRef,
    le: PublishedEdge,
    r: &PublishedValueRef,
    re: PublishedEdge,
    w: &mut Vec<(PublishedValueRef, PublishedValueRef)>,
) -> bool {
    match (l.arena().resolve(le), r.arena().resolve(re)) {
        (Ok(a), Ok(b)) => {
            w.push((a, b));
            true
        }
        _ => false,
    }
}
fn optional(
    l: &PublishedValueRef,
    le: Option<PublishedEdge>,
    r: &PublishedValueRef,
    re: Option<PublishedEdge>,
    w: &mut Vec<(PublishedValueRef, PublishedValueRef)>,
) -> bool {
    match (le, re) {
        (None, None) => true,
        (Some(a), Some(b)) => edge(l, a, r, b, w),
        _ => false,
    }
}
fn range(
    l: &PublishedValueRef,
    le: PublishedEdgeRange,
    r: &PublishedValueRef,
    re: PublishedEdgeRange,
    w: &mut Vec<(PublishedValueRef, PublishedValueRef)>,
) -> bool {
    match (l.arena().edges(le), r.arena().edges(re)) {
        (Ok(a), Ok(b)) if a.len() == b.len() => a.iter().zip(b).all(|(a, b)| edge(l, *a, r, *b, w)),
        _ => false,
    }
}
fn params(
    l: &PublishedValueRef,
    a: &[PublishedParameter],
    r: &PublishedValueRef,
    b: &[PublishedParameter],
    w: &mut Vec<(PublishedValueRef, PublishedValueRef)>,
) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            a.name == b.name
                && a.mode == b.mode
                && optional(l, a.annotation, r, b.annotation, w)
                && match (a.requirement, b.requirement) {
                    (
                        PublishedParameterRequirement::Defaulted(a),
                        PublishedParameterRequirement::Defaulted(b),
                    ) => edge(l, a, r, b, w),
                    (a, b) => a == b,
                }
        })
}
fn code(
    l: &PublishedValueRef,
    a: &PublishedCode,
    r: &PublishedValueRef,
    b: &PublishedCode,
    w: &mut Vec<(PublishedValueRef, PublishedValueRef)>,
) -> bool {
    a.source == b.source
        && a.name == b.name
        && a.definition == b.definition
        && range(l, a.dependencies, r, b.dependencies, w)
        && a.globals.len() == b.globals.len()
        && a.globals.iter().zip(b.globals.iter()).all(|(a, b)| {
            a.name == b.name && a.access == b.access && optional(l, a.value, r, b.value, w)
        })
}
fn row(
    l: &PublishedValueRef,
    r: &PublishedValueRef,
    state: &mut PublicationEqState,
    w: &mut Vec<(PublishedValueRef, PublishedValueRef)>,
) -> bool {
    match (l.node(), r.node()) {
        (PublishedNode::Native(a), PublishedNode::Native(b)) => {
            a.value().publication_eq_with(b.value(), state)
        }
        (PublishedNode::Function(a), PublishedNode::Function(b)) => {
            a.symbol == b.symbol
                && code(l, &a.code, r, &b.code, w)
                && params(l, &a.parameters, r, &b.parameters, w)
                && optional(l, a.return_annotation, r, b.return_annotation, w)
                && a.capture_names == b.capture_names
                && range(l, a.captures, r, b.captures, w)
        }
        (PublishedNode::Intrinsic(a), PublishedNode::Intrinsic(b)) => {
            a.api == b.api
                && params(l, &a.parameters, r, &b.parameters, w)
                && optional(l, a.return_annotation, r, b.return_annotation, w)
                && optional(l, a.receiver, r, b.receiver, w)
        }
        (PublishedNode::Code(a), PublishedNode::Code(b)) => code(l, a, r, b, w),
        (PublishedNode::Cell { contents: a }, PublishedNode::Cell { contents: b }) => {
            optional(l, *a, r, *b, w)
        }
        (PublishedNode::Set(a), PublishedNode::Set(b))
        | (PublishedNode::List(a), PublishedNode::List(b))
        | (PublishedNode::Tuple(a), PublishedNode::Tuple(b))
        | (PublishedNode::Dictionary(a), PublishedNode::Dictionary(b)) => range(l, *a, r, *b, w),
        (
            PublishedNode::Struct {
                names: a,
                values: av,
            },
            PublishedNode::Struct {
                names: b,
                values: bv,
            },
        ) => a == b && range(l, *av, r, *bv, w),
        (PublishedNode::Provider(a), PublishedNode::Provider(b)) => {
            a.identity == b.identity
                && a.has_schema == b.has_schema
                && a.names == b.names
                && range(l, a.values, r, b.values, w)
                && match (&a.files_to_run, &b.files_to_run) {
                    (None, None) => true,
                    (Some(a), Some(b)) => a.value().publication_eq_with(b.value(), state),
                    _ => false,
                }
        }
        (PublishedNode::ProviderConstructor(a), PublishedNode::ProviderConstructor(b)) => {
            a.id == b.id
                && projection::schema_eq(&a.schema, &b.schema)
                && optional(l, a.init, r, b.init, w)
        }
        (
            PublishedNode::RawProviderConstructor {
                provider: a,
                occurrence: ao,
            },
            PublishedNode::RawProviderConstructor {
                provider: b,
                occurrence: bo,
            },
        ) => state.enter_raw_provider_pair(ao.pointer(), bo.pointer()) && edge(l, *a, r, *b, w),
        (
            PublishedNode::ConfiguredTarget {
                identity: a,
                providers: av,
            },
            PublishedNode::ConfiguredTarget {
                identity: b,
                providers: bv,
            },
        ) => {
            a == b
                && av.len() == bv.len()
                && av.iter().zip(bv.iter()).all(|(a, b)| match (a, b) {
                    (
                        PublishedTargetProvider::Occurrence(a),
                        PublishedTargetProvider::Occurrence(b),
                    ) => edge(l, *a, r, *b, w),
                    (
                        PublishedTargetProvider::Native {
                            provider: a,
                            view: av,
                        },
                        PublishedTargetProvider::Native {
                            provider: b,
                            view: bv,
                        },
                    ) => a.value().publication_eq_with(b.value(), state) && edge(l, *av, r, *bv, w),
                    _ => false,
                })
        }
        (PublishedNode::Depset(a), PublishedNode::Depset(b)) => {
            state.enter_depset_occurrence_pair(
                a.occurrence.publication_pointer(),
                b.occurrence.publication_pointer(),
            ) && a.order == b.order
                && a.element_type == b.element_type
                && a.depth == b.depth
                && a.successors.len() == b.successors.len()
                && a.successors
                    .iter()
                    .zip(b.successors.iter())
                    .all(|(a, b)| match (a, b) {
                        (PublishedDepsetInput::Direct(a), PublishedDepsetInput::Direct(b))
                        | (
                            PublishedDepsetInput::Transitive(a),
                            PublishedDepsetInput::Transitive(b),
                        ) => edge(l, *a, r, *b, w),
                        _ => false,
                    })
        }
        (PublishedNode::CcVariables(a), PublishedNode::CcVariables(b)) => {
            state
                .enter_cc_variables_pair(a.occurrence.pointer(), b.occurrence.pointer())
                .is_ok()
                && a.kind == b.kind
                && optional(l, a.parent, r, b.parent, w)
                && a.locals.len() == b.locals.len()
                && a.locals
                    .iter()
                    .zip(b.locals.iter())
                    .all(|((an, av), (bn, bv))| an == bn && edge(l, *av, r, *bv, w))
                && a.seed.len() == b.seed.len()
                && a.seed.iter().zip(b.seed.iter()).all(|((an, a), (bn, b))| {
                    an == bn
                        && match (a, b) {
                            (PublishedVariableSeed::Value(a), PublishedVariableSeed::Value(b)) => {
                                edge(l, *a, r, *b, w)
                            }
                            (
                                PublishedVariableSeed::Sequence(a),
                                PublishedVariableSeed::Sequence(b),
                            ) => range(l, *a, r, *b, w),
                            (PublishedVariableSeed::Missing, PublishedVariableSeed::Missing) => {
                                true
                            }
                            (PublishedVariableSeed::Error(a), PublishedVariableSeed::Error(b)) => {
                                a == b
                            }
                            _ => false,
                        }
                })
        }
        _ => false,
    }
}
