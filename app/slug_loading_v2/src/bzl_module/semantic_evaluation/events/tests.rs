use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;

fn evaluate_prints(direct: bool) -> Arc<EventBatch> {
    let capture = SemanticPrintCapture::new(3, 100, direct).unwrap();
    let module = Module::new();
    let mut evaluator = Evaluator::new(&module);
    evaluator.set_print_handler(&capture);
    evaluator
        .eval_module(
            AstModule::parse(
                "independent_prints.bzl",
                "print('first', 7)\nprint('second')\n".to_owned(),
                &Dialect::Standard,
            )
            .unwrap(),
            &crate::package::loading_globals(),
        )
        .unwrap();
    drop(evaluator);
    capture.finish().unwrap()
}

#[test]
fn immutable_print_batch_preserves_actual_locations_order_and_presentation_independence() {
    let captured = evaluate_prints(false);
    let presented = evaluate_prints(true);
    assert_eq!(captured, presented);
    assert_eq!(captured.events().len(), 2);
    for (event, expected, line) in captured
        .events()
        .iter()
        .zip(["first 7", "second"])
        .zip([1, 2])
        .map(|((event, text), line)| (event, text, line))
    {
        let EvaluationEvent::StarlarkPrint { location, text } = event else {
            panic!("actual print produced a different event kind");
        };
        assert_eq!(text, expected);
        assert_eq!(
            location.to_string(),
            format!("independent_prints.bzl:{line}:6")
        );
    }
    let retained = captured.clone();
    drop(captured);
    assert_eq!(retained.events().len(), 2);
    assert!(
        SemanticPrintCapture::new(0, 0, false)
            .unwrap()
            .finish()
            .unwrap()
            .events()
            .is_empty()
    );
}

#[test]
fn combined_record_and_byte_limits_poison_before_event_or_capacity_growth() {
    assert!(SemanticPrintCapture::new(RECORD_LIMIT + 1, 0, false).is_err());
    assert!(SemanticPrintCapture::new(0, usize::MAX, false).is_err());
    let capture = SemanticPrintCapture::new(RECORD_LIMIT - 1, 0, false).unwrap();
    capture.capture(Arc::from("a.bzl"), 1, 1, "one").unwrap();
    let previous_capacity = capture.state.borrow().events.capacity();
    assert!(capture.capture(Arc::from("a.bzl"), 2, 1, "two").is_err());
    assert_eq!(capture.state.borrow().events.len(), 1);
    assert_eq!(capture.state.borrow().events.capacity(), previous_capacity);
    assert!(capture.finish().is_err());

    // A small borrowed payload discriminates the remaining byte budget; the
    // fixture never constructs an oversized output or a 64-MiB test string.
    let capture = SemanticPrintCapture::new(0, BYTE_LIMIT - 1024, false).unwrap();
    capture.capture(Arc::from("a.bzl"), 1, 1, "one").unwrap();
    let previous_payload = capture.state.borrow().payload_bytes;
    let previous_capacity = capture.state.borrow().events.capacity();
    let text = "x".repeat(1024);
    assert!(capture.capture(Arc::from("a.bzl"), 2, 1, &text).is_err());
    assert_eq!(capture.state.borrow().events.len(), 1);
    assert_eq!(capture.state.borrow().payload_bytes, previous_payload);
    assert_eq!(capture.state.borrow().events.capacity(), previous_capacity);
    assert!(capture.capture(Arc::from("a.bzl"), 3, 1, "").is_err());
    assert!(capture.finish().is_err());
}

#[test]
fn deterministic_growth_and_transfer_forecasts_cover_exact_byte_boundary() {
    let file = "a.bzl";
    let text = "message";
    let payload = file.len() + 2 * size_of::<usize>() + text.len() * 2 + 1;
    let fixed =
        size_of::<SemanticPrintCapture>() + size_of::<EventBatch>() + 4 * size_of::<usize>();
    // First growth credits four slots; final transfer credits 4+2*1 slots.
    let event_cost = fixed + payload + 6 * size_of::<EvaluationEvent>();
    let capture = SemanticPrintCapture::new(0, BYTE_LIMIT - event_cost, false).unwrap();
    capture.capture(Arc::from(file), 1, 1, text).unwrap();
    assert_eq!(capture.finish().unwrap().events().len(), 1);
    let capture = SemanticPrintCapture::new(0, BYTE_LIMIT - event_cost + 1, false).unwrap();
    assert!(capture.capture(Arc::from(file), 1, 1, text).is_err());
    assert!(capture.state.borrow().events.is_empty());
    assert_eq!(capture.state.borrow().events.capacity(), 0);
    assert!(capture.finish().is_err());

    let capture = SemanticPrintCapture::new(0, 0, false).unwrap();
    for line in 1..=6 {
        capture.capture(Arc::from(file), line, 1, text).unwrap();
    }
    assert_eq!(capture.state.borrow().capacity_credit, 8);
    assert_eq!(capture.finish().unwrap().events().len(), 6);
}
