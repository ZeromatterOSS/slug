use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use super::*;
use crate::callable_graph::CallableGraphBuilder;
use crate::callable_graph::CallableIntrinsic;
use crate::callable_graph::CallableNode;
use crate::callable_graph::NativeCallableApiId;

#[derive(Debug)]
struct Driver {
    drops: Arc<AtomicUsize>,
    fail: bool,
}
impl Drop for Driver {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
impl RuntimeMapper for Driver {
    fn map_batch(
        &self,
        values: &[AnalysisValue],
        _: &dyn ArgsDirectoryContext,
    ) -> Result<Vec<CompactString>, ArgsExpansionError> {
        if self.fail {
            return Err(ArgsExpansionError::Callback("real failure".into()));
        }
        Ok(values
            .iter()
            .map(|v| match v.kind() {
                AnalysisValueKind::String(s) => s.into(),
                _ => "value".into(),
            })
            .collect())
    }
}
fn mapper(runtime: Arc<dyn RuntimeMapper>, receiver: Option<AnalysisValue>) -> RetainedArgsMapper {
    let mut b = CallableGraphBuilder::new();
    let root = b.reserve().unwrap();
    let receiver = receiver.map(|value| {
        let id = b.reserve().unwrap();
        b.set_node(id, CallableNode::Native(value)).unwrap();
        id
    });
    b.set_node(
        root,
        CallableNode::Intrinsic(CallableIntrinsic {
            api: NativeCallableApiId::new("native.identity", 1).unwrap(),
            parameters: Arc::from([]),
            return_annotation: None,
            receiver,
        }),
    )
    .unwrap();
    RetainedArgsMapper::new(b.finish().unwrap(), root, runtime).unwrap()
}
#[test]
fn detached_runtime_guard_survives_producer_drop_and_errors_are_fallible() {
    let drops = Arc::new(AtomicUsize::new(0));
    let runtime: Arc<dyn RuntimeMapper> = Arc::new(Driver {
        drops: drops.clone(),
        fail: false,
    });
    let retained = mapper(runtime.clone(), None);
    drop(runtime);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(
        retained
            .map_batch(&[AnalysisValue::string("ok")], &NoopArgsDirectoryContext)
            .unwrap(),
        [CompactString::new("ok")]
    );
    let failed = mapper(
        Arc::new(Driver {
            drops: drops.clone(),
            fail: true,
        }),
        None,
    );
    assert!(matches!(
        failed.map_batch(&[], &NoopArgsDirectoryContext),
        Err(ArgsExpansionError::Callback(_))
    ));
    // Executable addresses and behavior do not participate in semantic identity.
    assert_eq!(retained, failed);
    drop(retained);
    drop(failed);
    assert_eq!(drops.load(Ordering::SeqCst), 2);
}
#[test]
fn input_capture_native_container_aliases_share_one_publication_state() {
    let drops = Arc::new(AtomicUsize::new(0));
    let driver = || {
        Arc::new(Driver {
            drops: drops.clone(),
            fail: false,
        }) as Arc<dyn RuntimeMapper>
    };
    let left = AnalysisValue::list(vec![AnalysisValue::string("item")]);
    let right = AnalysisValue::list(vec![AnalysisValue::string("item")]);
    let right_split = AnalysisValue::list(vec![AnalysisValue::string("item")]);
    let lm = mapper(driver(), Some(left.clone()));
    let rm = mapper(driver(), Some(right.clone()));
    let split = mapper(driver(), Some(right_split));
    let li = RetainedArgsInputs::Sequence(Arc::from([left]));
    let ri = RetainedArgsInputs::Sequence(Arc::from([right]));
    let mut state = PublicationEqState::default();
    assert!(li.publication_eq_with(&ri, &mut state));
    assert!(lm.publication_eq_with(&rm, &mut state));
    let mut state = PublicationEqState::default();
    assert!(li.publication_eq_with(&ri, &mut state));
    assert!(!lm.publication_eq_with(&split, &mut state));
    assert_eq!(li.values().unwrap().len(), 1);
    let too_many = RetainedArgsInputs::Sequence(
        vec![AnalysisValue::string("x"); CALLABLE_GRAPH_NODE_LIMIT + 1].into(),
    );
    assert!(matches!(
        too_many.values(),
        Err(ArgsExpansionError::Limit(_))
    ));
}

#[test]
fn run_environment_native_atoms_remain_outside_args_metadata_admission() {
    let value = AnalysisValue::run_environment_info(crate::RunEnvironmentInfo::empty());
    let inputs = RetainedArgsInputs::Sequence(Arc::from([value.clone()]));
    assert!(matches!(
        inputs.values(),
        Err(ArgsExpansionError::Unsupported(_))
    ));
    let mut graph = CallableGraphBuilder::new();
    let node = graph.reserve().unwrap();
    assert!(graph.set_node(node, CallableNode::Native(value)).is_err());
}
