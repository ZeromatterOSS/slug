//! Typed native descriptor/dispatch proof only; genuine Starlark tests are separate.
#![allow(dead_code)]
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use slug_build_api_v2::actions::ResourceEstimate;
use slug_build_api_v2::actions::ResourceSetError;
use slug_build_api_v2::actions::ResourceSetEvaluationContext;
use slug_build_api_v2::actions::ResourceSetInputs;
use slug_build_api_v2::actions::ResourceSetRuntime;
use slug_build_api_v2::actions::RetainedSpawnResourceSet;
use slug_build_api_v2::published_value::PublishedIntrinsic;
use slug_build_api_v2::published_value::PublishedNode;
use slug_build_api_v2::published_value::PublishedOriginalBinding;
use slug_build_api_v2::published_value::PublishedRootSlot;
use slug_build_api_v2::published_value::PublishedValueArena;
use slug_build_api_v2::published_value::PublishedValueArenaBuilder;

#[derive(Debug, Default)]
pub struct Context {
    pub cancelled: AtomicBool,
    pub printed: Mutex<Vec<String>>,
}
impl ResourceSetEvaluationContext for Context {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
    fn print(&self, text: &str) -> Result<(), ResourceSetError> {
        self.printed.lock().unwrap().push(text.to_owned());
        Ok(())
    }
    fn mnemonic(&self) -> &str {
        "UnrelatedResource"
    }
}
#[derive(Debug)]
struct Original;
impl PublishedOriginalBinding for Original {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn root_count(&self) -> usize {
        1
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &[]
    }
}
#[derive(Debug)]
struct Count {
    calls: Arc<AtomicUsize>,
}
impl ResourceSetRuntime for Count {
    fn evaluate(
        &self,
        _: ResourceSetInputs,
        _: &dyn ResourceSetEvaluationContext,
    ) -> Result<ResourceEstimate, ResourceSetError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(ResourceSetError::Callback(
            "typed runtime would fail if invoked".into(),
        ))
    }
}
pub fn descriptor(api: &str) -> (RetainedSpawnResourceSet, Arc<AtomicUsize>) {
    let mut b = PublishedValueArenaBuilder::new();
    let root = b.reserve().unwrap();
    b.set_node(
        root,
        PublishedNode::Intrinsic(PublishedIntrinsic {
            api: slug_build_api_v2::NativeCallableApiId::new(api, 1).unwrap(),
            parameters: Arc::from([]),
            return_annotation: None,
            receiver: None,
        }),
    )
    .unwrap();
    b.set_original_slot(root, PublishedRootSlot::new(0).unwrap())
        .unwrap();
    let value = b.seal(Arc::new(Original)).unwrap().value(root).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let descriptor = RetainedSpawnResourceSet::from_published(
        value,
        Arc::new(Count {
            calls: calls.clone(),
        }),
    )
    .unwrap();
    (descriptor, calls)
}
