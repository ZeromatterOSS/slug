//! Exercise the current consumer signal through the real retained recipe path.
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::ArgsDirectoryContext;
use slug_build_api_v2::ArgsExpansionError;
use slug_build_api_v2::CallableGraphBuilder;
use slug_build_api_v2::CallableIntrinsic;
use slug_build_api_v2::CallableNode;
use slug_build_api_v2::NativeCallableApiId;
use slug_build_api_v2::RetainedArgCall;
use slug_build_api_v2::RetainedArgsInputs;
use slug_build_api_v2::RetainedArgsMapper;
use slug_build_api_v2::RetainedArgsRecipe;
use slug_build_api_v2::RetainedParamFileFormat;
use slug_build_api_v2::RetainedVectorArg;
use slug_build_api_v2::RetainedVectorOptions;
use slug_build_api_v2::RetainedVectorSource;

use super::*;

#[derive(Debug)]
struct Consumer {
    checks: AtomicUsize,
    cancelled: AtomicBool,
    cancel_at: usize,
}
impl Consumer {
    fn new(cancel_at: usize) -> Self {
        Self {
            checks: AtomicUsize::new(0),
            cancelled: AtomicBool::new(false),
            cancel_at,
        }
    }
}
impl ArgsDirectoryContext for Consumer {
    fn is_cancelled(&self) -> bool {
        if self.checks.fetch_add(1, Ordering::Relaxed) >= self.cancel_at {
            self.cancelled.store(true, Ordering::Release);
        }
        self.cancelled.load(Ordering::Acquire)
    }
    fn list(
        &self,
        artifact: &AnalysisArtifact,
    ) -> Result<Vec<AnalysisArtifact>, ArgsExpansionError> {
        Ok(vec![artifact.clone()])
    }
}

#[test]
fn args_recipe_current_consumer_cancels_original_callback_at_frozen_bytecode_checkpoint() {
    let module = Module::new();
    super::tests::evaluate(
        &module,
        "def mapper(x):\n    for i in range(20000):\n        x = str(i)\n    return x\n",
    );
    let runtime = StarlarkRuntimeMapper::new(module.freeze().unwrap().get("mapper").unwrap());
    // This descriptor fixture only supplies the native recipe's required graph
    // ownership; publication/source/equality admission has its own tests. The
    // executable is the actual original Def, with no cancellation test hook.
    let mut graph = CallableGraphBuilder::new();
    let root = graph.reserve().unwrap();
    graph
        .set_node(
            root,
            CallableNode::Intrinsic(CallableIntrinsic {
                api: NativeCallableApiId::new("test.runtime_cancellation", 1).unwrap(),
                parameters: Arc::from([]),
                return_annotation: None,
                receiver: None,
            }),
        )
        .unwrap();
    let mapper = RetainedArgsMapper::new(graph.finish().unwrap(), root, Arc::new(runtime)).unwrap();
    let recipe = RetainedArgsRecipe::new(
        vec![RetainedArgCall::AddAll(RetainedVectorArg::new(
            RetainedVectorSource::Mapped {
                inputs: RetainedArgsInputs::Sequence(Arc::from([AnalysisValue::string("input")])),
                mapper,
                registration_location: Some("registration.bzl:7".into()),
            },
            RetainedVectorOptions {
                arg_name: None,
                format_each: None,
                before_each: None,
                join_with: None,
                format_joined: None,
                omit_if_empty: true,
                uniquify: false,
                expand_directories: false,
                terminate_with: None,
            },
        ))],
        RetainedParamFileFormat::Shell,
    );

    let cancelled = Consumer::new(10);
    assert_eq!(
        recipe.render_with_context(&cancelled),
        Err(ArgsExpansionError::Cancelled)
    );
    assert!(cancelled.cancelled.load(Ordering::Acquire));
    assert!(cancelled.checks.load(Ordering::Relaxed) >= 11);
    // The same published mapper is usable by a new live consumer. It never
    // retains the old consumer's cancellation state in its executable payload.
    let live = Consumer::new(usize::MAX);
    assert_eq!(recipe.render_with_context(&live).unwrap(), ["19999"]);
    assert!(!live.cancelled.load(Ordering::Acquire));
    assert!(live.checks.load(Ordering::Relaxed) > 10);
}
