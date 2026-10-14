use std::cell::Cell;
use std::fmt;

use allocative::Allocative;
use starlark_derive::NoSerialize;
use starlark_derive::Trace;
use starlark_derive::starlark_value;

use super::*;
use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::values::FrozenHeap;
use crate::values::OwnedHeap;
use crate::values::StarlarkValue;
use crate::values::Value;
use crate::values::ValueLike;
use crate::values::list::AllocList;
use crate::values::list::ListRef;
use crate::values::types::list::value::ListData;

#[derive(Debug, Trace, ProvidesStaticType, NoSerialize, Allocative)]
struct Link<'v> {
    edges: Vec<Value<'v>>,
}
impl fmt::Display for Link<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("link")
    }
}
#[starlark_value(type = "gc_link")]
impl<'v> StarlarkValue<'v> for Link<'v> {}

#[test]
fn deferred_complex_gc_preserves_deep_diamond_cycle_and_second_collection() {
    let owned = OwnedHeap::new();
    let heap = owned.as_ref();
    let foreign = FrozenHeap::new();
    let text = foreign.alloc("foreign");
    let child = heap.alloc_complex_no_freeze(Link {
        edges: vec![text.to_value()],
    });
    let split = heap.alloc_complex_no_freeze(Link {
        edges: vec![text.to_value()],
    });
    let cycle = heap.alloc_list(&[]);
    let mut root = child;
    for _ in 1..3500 {
        root = heap.alloc_complex_no_freeze(Link {
            edges: vec![root, child, cycle],
        });
    }
    ListData::from_value_mut(cycle).unwrap().push(root, heap);
    let mut roots = heap.alloc(AllocList([root, child, child, split]));
    let before = roots.identity();
    // All mutable edges belong to heap; every live root is rewritten here.
    unsafe { heap.garbage_collect(|tracer| tracer.trace(&mut roots)) }.unwrap();
    assert_ne!(before, roots.identity());
    for _ in 0..2 {
        let fields = ListRef::from_value(roots).unwrap();
        let values = fields.content();
        assert_eq!(values[1].identity(), values[2].identity());
        assert_ne!(values[1].identity(), values[3].identity());
        let root = values[0];
        let mut cursor = root;
        for _ in 1..3500 {
            let node = cursor.downcast_ref::<Link>().unwrap();
            assert_eq!(node.edges[1].identity(), values[1].identity());
            let back = ListRef::from_value(node.edges[2]).unwrap();
            assert_eq!(back.content()[0].identity(), root.identity());
            cursor = node.edges[0];
        }
        assert_eq!(cursor.identity(), values[1].identity());
        let leaf = cursor.downcast_ref::<Link>().unwrap();
        assert_eq!(leaf.edges[0].identity(), text.to_value().identity());
        // End all payload borrows before moving the graph a second time.
        drop(fields);
        unsafe { heap.garbage_collect(|tracer| tracer.trace(&mut roots)) }.unwrap();
    }
}

#[test]
fn gc_queue_refusal_precedes_root_rewrite_and_collection_callback() {
    let owned = OwnedHeap::new();
    let heap = owned.as_ref();
    let mut root = heap.alloc_complex_no_freeze(Link {
        edges: vec![heap.alloc("retained")],
    });
    let identity = root.identity();
    let bytes = heap.allocated_bytes();
    let called = Cell::new(false);
    let result = unsafe {
        heap.garbage_collect_with_queue_limit(0, |tracer| {
            called.set(true);
            tracer.trace(&mut root);
        })
    };
    assert!(result.is_err());
    assert!(!called.get());
    assert_eq!(heap.allocated_bytes(), bytes);
    assert_eq!(root.identity(), identity);
    assert_eq!(
        root.downcast_ref::<Link>().unwrap().edges[0].unpack_str(),
        Some("retained")
    );
    unsafe { heap.garbage_collect(|tracer| tracer.trace(&mut root)) }.unwrap();
    assert_ne!(root.identity(), identity);
}

#[test]
fn gc_queue_forecast_is_checked_and_has_no_minimum_growth_during_drain() {
    assert!(DeferredTraceQueue::with_limit(usize::MAX, usize::MAX).is_err());
    assert!(DeferredTraceQueue::with_limit(0, 0).is_err());
    let queue = DeferredTraceQueue::new(0).unwrap();
    assert_eq!(queue.tasks.borrow().capacity(), 0);
    assert_eq!(queue.inserted.get(), 0);
}
