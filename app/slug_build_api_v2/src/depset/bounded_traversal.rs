use std::alloc::Layout;

use fxhash::hash64;

use super::DepsetOrder;
use super::DepsetSuccessor;
use super::DepsetView;

/// A traversal stopped before returning any output.
#[derive(Debug, PartialEq, Eq)]
pub enum DepsetTraversalError<E> {
    /// The charge, leaf hash or leaf equality callback refused.
    Operation(E),
    /// The requested scratch layout exceeds the representable size.
    SizeOverflow,
    /// A fallible scratch reservation failed.
    AllocationFailed,
}

type Result<T, E> = std::result::Result<T, DepsetTraversalError<E>>;

/// Flatten a depset using the same ordered walk and equality as
/// [`super::traverse_depset`], admitting scratch allocation and work before use.
///
/// `charge` receives work units and cumulative requested allocation bytes.
/// A failed charge stops the walk. The leaf callbacks admit their own hash and
/// equality work. Use a view with constant-cost node keys and key hashing,
/// equality, cloning, dropping, and individual successor accesses.
pub fn traverse_depset_bounded<N, E>(
    root: &N,
    mut charge: impl FnMut(usize, usize) -> std::result::Result<(), E>,
    mut hash: impl FnMut(&N::Item) -> std::result::Result<u64, E>,
    mut equals: impl FnMut(&N::Item, &N::Item) -> std::result::Result<bool, E>,
) -> Result<Vec<N::Item>, E>
where
    N: DepsetView,
{
    let mut stack = Vec::new();
    let mut nodes = Slots::new();
    let mut leaves = Slots::new();
    let mut links = Vec::new();
    let mut out = Vec::new();
    pay(&mut charge, 1, 0)?;
    push(
        &mut stack,
        DepsetSuccessor::Transitive(root.clone()),
        &mut charge,
    )?;

    // Keep the ordinary walk's reverse pushes, first occurrences, and final reversal.
    while !stack.is_empty() {
        pay(&mut charge, 1, 0)?;
        match stack.pop().unwrap() {
            DepsetSuccessor::Transitive(node) => {
                pay(&mut charge, 1, 0)?;
                let key = node.node_key();
                let hash = hash64(&key);
                if let Probe::Vacant(index) =
                    nodes.probe(hash, |existing| existing == &key, &mut charge)?
                {
                    nodes.insert(index, hash, key, &mut charge)?;
                    // Admit the first edge access, then admit the next one on
                    // return from each callback. This includes the end of the row.
                    pay(&mut charge, 1, 0)?;
                    node.for_each_successor_reverse(|edge| {
                        push(&mut stack, edge, &mut charge)?;
                        pay(&mut charge, 1, 0)
                    })?;
                }
            }
            DepsetSuccessor::Direct(item) => {
                pay(&mut charge, 1, 0)?;
                let hash = hash(&item).map_err(DepsetTraversalError::Operation)?;
                let slot = leaves.probe(hash, |_| true, &mut charge)?;
                if let Probe::Found(index) = slot {
                    let chain: &LeafChain = leaves.get(index);
                    let mut next = Some(chain.first);
                    let mut duplicate = false;
                    while let Some(leaf) = next {
                        pay(&mut charge, 1, 0)?;
                        if equals(&out[leaf], &item).map_err(DepsetTraversalError::Operation)? {
                            duplicate = true;
                            break;
                        }
                        next = links[leaf];
                    }
                    if duplicate {
                        continue;
                    }
                }

                let index = out.len();
                push(&mut out, item, &mut charge)?;
                push(&mut links, None, &mut charge)?;
                match slot {
                    Probe::Found(slot) => {
                        let chain = leaves.get_mut(slot);
                        links[chain.last] = Some(index);
                        chain.last = index;
                    }
                    Probe::Vacant(slot) => leaves.insert(
                        slot,
                        hash,
                        LeafChain {
                            first: index,
                            last: index,
                        },
                        &mut charge,
                    )?,
                }
            }
        }
    }
    if root.order() == DepsetOrder::Topological {
        pay(&mut charge, out.len() / 2, 0)?;
        out.reverse();
    }
    Ok(out)
}

struct LeafChain {
    first: usize,
    last: usize,
}

fn pay<E>(
    charge: &mut impl FnMut(usize, usize) -> std::result::Result<(), E>,
    work: usize,
    bytes: usize,
) -> Result<(), E> {
    charge(work, bytes).map_err(DepsetTraversalError::Operation)
}

fn double<E>(capacity: usize) -> Result<usize, E> {
    capacity
        .checked_mul(2)
        .ok_or(DepsetTraversalError::SizeOverflow)
}

fn reserve<T, E>(
    values: &mut Vec<T>,
    capacity: usize,
    work: usize,
    charge: &mut impl FnMut(usize, usize) -> std::result::Result<(), E>,
) -> Result<(), E> {
    let bytes = Layout::array::<T>(capacity)
        .map_err(|_| DepsetTraversalError::SizeOverflow)?
        .size();
    pay(charge, work, bytes)?;
    values
        .try_reserve_exact(capacity - values.len())
        .map_err(|_| DepsetTraversalError::AllocationFailed)
}

fn push<T, E>(
    values: &mut Vec<T>,
    value: T,
    charge: &mut impl FnMut(usize, usize) -> std::result::Result<(), E>,
) -> Result<(), E> {
    pay(charge, 1, 0)?;
    if values.len() == values.capacity() {
        let len = values.len();
        reserve(values, double(len)?.max(1), len, charge)?;
    }
    values.push(value);
    Ok(())
}

enum Probe {
    Found(usize),
    Vacant(usize),
}

/// Request-local slots with no deletion and at most half occupancy. A lookup
/// admits each actual probe, including those caused by hash collisions.
struct Slots<T> {
    values: Vec<Option<(u64, T)>>,
    len: usize,
}

impl<T> Slots<T> {
    fn new() -> Self {
        Self {
            values: Vec::new(),
            len: 0,
        }
    }

    fn probe<E>(
        &self,
        hash: u64,
        mut matches: impl FnMut(&T) -> bool,
        charge: &mut impl FnMut(usize, usize) -> std::result::Result<(), E>,
    ) -> Result<Probe, E> {
        if self.values.is_empty() {
            return Ok(Probe::Vacant(0));
        }
        let mask = self.values.len() - 1;
        let mut index = ((hash ^ (hash >> 32)) as usize) & mask;
        loop {
            pay(charge, 1, 0)?;
            match &self.values[index] {
                None => return Ok(Probe::Vacant(index)),
                Some((stored, value)) if *stored == hash && matches(value) => {
                    return Ok(Probe::Found(index));
                }
                Some(_) => index = (index + 1) & mask,
            }
        }
    }

    fn get(&self, index: usize) -> &T {
        &self.values[index].as_ref().unwrap().1
    }

    fn get_mut(&mut self, index: usize) -> &mut T {
        &mut self.values[index].as_mut().unwrap().1
    }

    fn insert<E>(
        &mut self,
        mut index: usize,
        hash: u64,
        value: T,
        charge: &mut impl FnMut(usize, usize) -> std::result::Result<(), E>,
    ) -> Result<(), E> {
        pay(charge, 1, 0)?;
        if self.len >= self.values.len() / 2 {
            self.grow(charge)?;
            index = self.vacancy(hash, charge)?;
        }
        debug_assert!(self.values[index].is_none());
        self.values[index] = Some((hash, value));
        self.len += 1;
        Ok(())
    }

    fn vacancy<E>(
        &self,
        hash: u64,
        charge: &mut impl FnMut(usize, usize) -> std::result::Result<(), E>,
    ) -> Result<usize, E> {
        match self.probe(hash, |_| false, charge)? {
            Probe::Vacant(index) => Ok(index),
            Probe::Found(_) => unreachable!(),
        }
    }

    fn grow<E>(
        &mut self,
        charge: &mut impl FnMut(usize, usize) -> std::result::Result<(), E>,
    ) -> Result<(), E> {
        let previous = self.values.len();
        let capacity = double(previous)?.max(4);
        let mut replacement = Vec::new();
        pay(charge, previous, 0)?;
        reserve(&mut replacement, capacity, capacity, charge)?;
        replacement.resize_with(capacity, || None);
        let old = std::mem::replace(&mut self.values, replacement);
        for (hash, value) in old.into_iter().flatten() {
            let index = self.vacancy(hash, charge)?;
            self.values[index] = Some((hash, value));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::sync::Arc;

    use super::*;
    use crate::depset::Depset;
    use crate::depset::traverse_depset;

    #[derive(Debug, PartialEq, Eq)]
    enum Refusal {
        Work,
        Storage,
    }

    #[test]
    fn exact_work_and_storage_preserve_all_orders_and_shared_children() {
        for order in [
            DepsetOrder::Default,
            DepsetOrder::Postorder,
            DepsetOrder::Preorder,
            DepsetOrder::Topological,
        ] {
            let shared = Depset::from_direct(order, vec![1, 2]).unwrap();
            let left = Depset::new(order, vec![3, 1], vec![shared.clone()]).unwrap();
            let right = Depset::new(order, vec![4, 2], vec![shared]).unwrap();
            let root = Depset::new(order, vec![0, 3], vec![left, right]).unwrap();
            let mut required_work = 0;
            let mut required_bytes = 0;
            let hash = |_: &i32| Ok(0);
            let equals = |left: &i32, right: &i32| Ok(left == right);
            let observed = traverse_depset_bounded(
                &root,
                |work, bytes| {
                    required_work += work;
                    required_bytes += bytes;
                    Ok::<_, Refusal>(())
                },
                hash,
                equals,
            )
            .unwrap();
            assert_eq!(observed, root.to_list(), "{order}");
            assert_eq!(observed.len(), 5);
            let run = |mut work: usize, mut bytes: usize| {
                traverse_depset_bounded(
                    &root,
                    |used_work, used_bytes| {
                        work = work.checked_sub(used_work).ok_or(Refusal::Work)?;
                        bytes = bytes.checked_sub(used_bytes).ok_or(Refusal::Storage)?;
                        Ok(())
                    },
                    hash,
                    equals,
                )
            };
            assert_eq!(run(required_work, required_bytes), Ok(observed));
            assert_eq!(
                run(required_work - 1, required_bytes),
                Err(DepsetTraversalError::Operation(Refusal::Work))
            );
            assert_eq!(
                run(required_work, required_bytes - 1),
                Err(DepsetTraversalError::Operation(Refusal::Storage))
            );
        }
    }

    #[derive(Clone, PartialEq, Eq)]
    struct CollisionKey(u8);

    impl Hash for CollisionKey {
        fn hash<H: Hasher>(&self, state: &mut H) {
            0u8.hash(state);
        }
    }

    #[derive(Clone)]
    struct Node(u8, Arc<[DepsetSuccessor<u8, Self>]>);

    impl DepsetView for Node {
        type Item = u8;
        type NodeKey = CollisionKey;

        fn order(&self) -> DepsetOrder {
            DepsetOrder::Postorder
        }

        fn depth(&self) -> usize {
            0
        }

        fn node_key(&self) -> Self::NodeKey {
            CollisionKey(self.0)
        }

        fn singleton_item(&self) -> Option<Self::Item> {
            None
        }

        fn for_each_successor_reverse<E>(
            &self,
            mut visit: impl FnMut(DepsetSuccessor<Self::Item, Self>) -> std::result::Result<(), E>,
        ) -> std::result::Result<(), E> {
            for successor in self.1.iter().rev() {
                visit(successor.clone())?;
            }
            Ok(())
        }
    }

    #[test]
    fn node_collisions_and_leaf_chain_rehash_keep_first_seen_comparison_order() {
        use DepsetSuccessor::Direct;
        use DepsetSuccessor::Transitive;

        let shared = Node(1, Arc::from([Direct(70), Direct(71)]));
        let left = Node(2, Arc::from([Transitive(shared.clone()), Direct(2)]));
        let right = Node(3, Arc::from([Transitive(shared), Direct(3)]));
        let root = Node(
            0,
            Arc::from([
                Transitive(left),
                Transitive(right),
                Direct(4),
                Direct(5),
                Direct(72),
                Direct(71),
            ]),
        );
        // 70/71 populate one hash chain. The unrelated keys force slot growth
        // before 72/71 advance the rehashed chain.
        let hash =
            |value: &u8| Ok::<_, Infallible>(if *value >= 70 { 0 } else { u64::from(*value) });
        let mut ordinary = Vec::new();
        let expected = traverse_depset(&root, hash, |left, right| {
            ordinary.push((*left, *right));
            Ok(left == right)
        })
        .unwrap();
        let mut bounded = Vec::new();
        let observed = traverse_depset_bounded(
            &root,
            |_, _| Ok(()),
            hash,
            |left, right| {
                bounded.push((*left, *right));
                Ok(left == right)
            },
        )
        .unwrap();
        assert_eq!(observed, [70, 71, 2, 3, 4, 5, 72]);
        assert_eq!(observed, expected);
        assert_eq!(bounded, [(70, 71), (70, 72), (71, 72), (70, 71), (71, 71)]);
        assert_eq!(bounded, ordinary);
    }
}
