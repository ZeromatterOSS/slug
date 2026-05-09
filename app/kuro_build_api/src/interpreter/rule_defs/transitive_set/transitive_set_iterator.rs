/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::HashSet;
use std::collections::VecDeque;
use std::marker::PhantomData;

use kuro_error::BuckErrorContext;
use kuro_util::hash::BuckHasherBuilder;
use starlark::values::Value;
use starlark::values::ValueIdentity;
use starlark::values::ValueLike;

use crate::interpreter::rule_defs::nested_set::NestedSetOrder;
use crate::interpreter::rule_defs::nested_set::NestedSetPreorderDedupe;
use crate::interpreter::rule_defs::nested_set::nested_set_node_iter;
use crate::interpreter::rule_defs::nested_set::preorder_nested_set_node_iter;
use crate::interpreter::rule_defs::transitive_set::TransitiveSetGen;
use crate::interpreter::rule_defs::transitive_set::TransitiveSetLike;
use crate::interpreter::rule_defs::transitive_set::transitive_set::NodeGen;

pub trait TransitiveSetIteratorLike<'a, 'v, V>: Iterator<Item = &'a TransitiveSetGen<V>>
where
    V: 'v + Copy + ValueLike<'v>,
    TransitiveSetGen<V>: TransitiveSetLike<'v>,
    'v: 'a,
{
    fn values(self: Box<Self>) -> TransitiveSetValuesIteratorGen<'a, 'v, V>;
}

fn assert_transitive_set<'v, V>(child: Value<'v>) -> &'v TransitiveSetGen<V>
where
    V: 'v + Copy + ValueLike<'v>,
    TransitiveSetGen<V>: TransitiveSetLike<'v>,
{
    TransitiveSetLike::from_value(child)
        .with_buck_error_context(|| {
            format!(
                "Invalid set: expected {:?}, got: {:?}",
                std::any::type_name::<V>(),
                child
            )
        })
        .unwrap()
}

pub struct NestedOrderTransitiveSetIteratorGen<'a, 'v, V: ValueLike<'v>> {
    inner: Box<dyn Iterator<Item = &'a TransitiveSetGen<V>> + 'a>,
    _marker: PhantomData<&'v ()>,
}

impl<'a, 'v, V> NestedOrderTransitiveSetIteratorGen<'a, 'v, V>
where
    V: 'v + Copy + ValueLike<'v>,
    TransitiveSetGen<V>: TransitiveSetLike<'v>,
    'v: 'a,
{
    pub fn new(set: &'a TransitiveSetGen<V>, order: NestedSetOrder) -> Self {
        let identity = |set| set as *const TransitiveSetGen<V>;
        let children = transitive_set_children::<V>;
        Self {
            inner: match order {
                NestedSetOrder::Preorder => Box::new(preorder_nested_set_node_iter(
                    set,
                    NestedSetPreorderDedupe::OnChildEnqueue,
                    identity,
                    children,
                )),
                NestedSetOrder::Default
                | NestedSetOrder::Postorder
                | NestedSetOrder::Topological => {
                    nested_set_node_iter(set, order, identity, children)
                }
            },
            _marker: PhantomData,
        }
    }
}

impl<'a, 'v, V> TransitiveSetIteratorLike<'a, 'v, V>
    for NestedOrderTransitiveSetIteratorGen<'a, 'v, V>
where
    V: 'v + Copy + ValueLike<'v>,
    TransitiveSetGen<V>: TransitiveSetLike<'v>,
    'v: 'a,
{
    fn values(self: Box<Self>) -> TransitiveSetValuesIteratorGen<'a, 'v, V> {
        TransitiveSetValuesIteratorGen { inner: self }
    }
}

impl<'a, 'v, V> Iterator for NestedOrderTransitiveSetIteratorGen<'a, 'v, V>
where
    V: 'v + Copy + ValueLike<'v>,
    TransitiveSetGen<V>: TransitiveSetLike<'v>,
    'v: 'a,
{
    type Item = &'a TransitiveSetGen<V>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

fn transitive_set_children<'a, 'v, V>(set: &'a TransitiveSetGen<V>) -> Vec<&'a TransitiveSetGen<V>>
where
    V: 'v + Copy + ValueLike<'v>,
    TransitiveSetGen<V>: TransitiveSetLike<'v>,
    'v: 'a,
{
    set.children
        .iter()
        .map(|child| assert_transitive_set(child.to_value()))
        .collect()
}

/// Preorder breadth-first-search (BFS), visits parent node, then eagerly visits all children
/// left-to-right before traversing to any grandchildren.
pub struct BfsTransitiveSetIteratorGen<'a, 'v, V: ValueLike<'v>> {
    queue: VecDeque<&'a TransitiveSetGen<V>>,
    seen: HashSet<ValueIdentity<'v>, BuckHasherBuilder>,
}

impl<'a, 'v, V> BfsTransitiveSetIteratorGen<'a, 'v, V>
where
    V: 'v + Copy + ValueLike<'v>,
    TransitiveSetGen<V>: TransitiveSetLike<'v>,
    'v: 'a,
{
    pub fn new(set: &'a TransitiveSetGen<V>) -> Self {
        Self {
            queue: VecDeque::from(vec![set]),
            seen: Default::default(),
        }
    }

    fn enqueue_children(&mut self, children: &'a [V]) {
        for child in children.iter() {
            let child = child.to_value();

            if self.seen.insert(child.identity()) {
                self.queue.push_back(assert_transitive_set(child));
            }
        }
    }
}

impl<'a, 'v, V> TransitiveSetIteratorLike<'a, 'v, V> for BfsTransitiveSetIteratorGen<'a, 'v, V>
where
    V: 'v + Copy + ValueLike<'v>,
    TransitiveSetGen<V>: TransitiveSetLike<'v>,
    'v: 'a,
{
    fn values(self: Box<Self>) -> TransitiveSetValuesIteratorGen<'a, 'v, V> {
        TransitiveSetValuesIteratorGen { inner: self }
    }
}

impl<'a, 'v, V> Iterator for BfsTransitiveSetIteratorGen<'a, 'v, V>
where
    V: 'v + Copy + ValueLike<'v>,
    TransitiveSetGen<V>: TransitiveSetLike<'v>,
    'v: 'a,
{
    type Item = &'a TransitiveSetGen<V>;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.queue.pop_front()?;
        self.enqueue_children(&next.children);
        Some(next)
    }
}

/// Preorder depth-first-search (DFS).
///
/// This is similar to the pre-order traversal, except that children are guaranteed to be visited
/// left-to-right.
pub struct DfsTransitiveSetIteratorGen<'a, 'v, V: ValueLike<'v>> {
    stack: Vec<(&'a TransitiveSetGen<V>, Option<ValueIdentity<'v>>)>,
    seen: HashSet<ValueIdentity<'v>, BuckHasherBuilder>,
}

impl<'a, 'v, V> DfsTransitiveSetIteratorGen<'a, 'v, V>
where
    V: 'v + Copy + ValueLike<'v>,
    TransitiveSetGen<V>: TransitiveSetLike<'v>,
    'v: 'a,
{
    pub fn new(set: &'a TransitiveSetGen<V>) -> Self {
        Self {
            stack: vec![(set, None)],
            seen: Default::default(),
        }
    }
}

impl<'a, 'v, V> TransitiveSetIteratorLike<'a, 'v, V> for DfsTransitiveSetIteratorGen<'a, 'v, V>
where
    V: 'v + Copy + ValueLike<'v>,
    TransitiveSetGen<V>: TransitiveSetLike<'v>,
    'v: 'a,
{
    fn values(self: Box<Self>) -> TransitiveSetValuesIteratorGen<'a, 'v, V> {
        TransitiveSetValuesIteratorGen { inner: self }
    }
}

impl<'a, 'v, V> Iterator for DfsTransitiveSetIteratorGen<'a, 'v, V>
where
    V: 'v + Copy + ValueLike<'v>,
    TransitiveSetGen<V>: TransitiveSetLike<'v>,
    'v: 'a,
{
    type Item = &'a TransitiveSetGen<V>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (tset, identity) = self.stack.pop()?;
            if identity.is_none_or(|id| self.seen.insert(id)) {
                for child in tset.children.iter().rev() {
                    let child = child.to_value();
                    let child_identity = child.identity();
                    if !self.seen.contains(&child_identity) {
                        self.stack
                            .push((assert_transitive_set(child), Some(child_identity)));
                    }
                }
                return Some(tset);
            }
        }
    }
}

/// An iterator over values of a TransitiveSet. Notionally a FilterMap, but defined as its own type
/// since there are a few too many lifetimes involved to make a nice `impl Iterator<...>` work
/// here.
pub struct TransitiveSetValuesIteratorGen<'a, 'v, V: ValueLike<'v>> {
    inner: Box<dyn TransitiveSetIteratorLike<'a, 'v, V> + 'a>,
}

impl<'a, 'v, V> Iterator for TransitiveSetValuesIteratorGen<'a, 'v, V>
where
    V: 'v + Copy + ValueLike<'v>,
    TransitiveSetGen<V>: TransitiveSetLike<'v>,
    'v: 'a,
{
    type Item = &'a NodeGen<V>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let next = self.inner.next()?;
            if let Some(node) = next.node.as_ref() {
                return Some(node);
            }
        }
    }
}
