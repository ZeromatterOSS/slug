//! The new capture is a mutable-heap value rooted only by the Args carrier.
use starlark::environment::GlobalsBuilder;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;

#[starlark_module]
fn globals(builder: &mut GlobalsBuilder) {
    fn make_args<'v>(heap: Heap<'v>) -> anyhow::Result<Value<'v>> {
        let fake_exe = heap.alloc_simple(AnalysisArtifactValue::new(AnalysisArtifact::Source(
            CanonicalLabel::parse("@@//:only_capture").unwrap(),
        )));
        Ok(heap.alloc(StarlarkArgsGen {
            state: Arc::new(Mutex::new(StarlarkArgsStateGen {
                calls: vec![EvaluatorArgCallGen::AddAll(EvaluatorVectorArgGen {
                    source: EvaluatorVectorSourceGen::Sequence(Vec::new()),
                    options: RetainedVectorOptions {
                        arg_name: None,
                        format_each: None,
                        before_each: None,
                        join_with: None,
                        format_joined: None,
                        omit_if_empty: true,
                        uniquify: false,
                        expand_directories: true,
                        terminate_with: None,
                    },
                    map_each: Some(EvaluatorVectorMapEachGen::CargoRunfiles {
                        fake_exe,
                        workspace_name: "_main".into(),
                    }),
                })],
                format: None,
                param_file: None,
            })),
        }))
    }
}

#[test]
fn cargo_args_traces_a_capture_without_an_input_or_caller_root() {
    let module = Module::new();
    let globals = GlobalsBuilder::standard().with(globals).build();
    let ast = AstModule::parse(
        "capture_gc.star",
        r#"
args = make_args()
def churn():
    for i in range(100):
        temporary = [str(i)] * 256
churn()
collected = True
"#
        .to_owned(),
        &Dialect::Standard,
    )
    .unwrap();
    let mut evaluator = Evaluator::new(&module);
    evaluator.eval_module(ast, &globals).unwrap();
    // The statement after churn is an ordinary module GC safe point. All
    // native constructor handles have gone; the capture is rooted only by Args.
    assert!(module.heap().allocated_bytes() < module.heap().peak_allocated_bytes());
    let snapshot = StarlarkArgs::snapshot(module.get("args").unwrap()).unwrap();
    let EvaluatorArgCallGen::AddAll(vector) = &snapshot.calls[0] else {
        panic!()
    };
    let Some(EvaluatorVectorMapEachGen::CargoRunfiles {
        fake_exe,
        workspace_name,
    }) = &vector.map_each
    else {
        panic!()
    };
    assert_eq!(workspace_name, "_main");
    assert_eq!(
        AnalysisArtifactValue::from_starlark(*fake_exe)
            .unwrap()
            .artifact(),
        &AnalysisArtifact::Source(CanonicalLabel::parse("@@//:only_capture").unwrap())
    );
}
