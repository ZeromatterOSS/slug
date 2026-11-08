use super::*;

fn bank() -> Arc<RepositoryRootResources> {
    RepositoryRootResources::new(
        RepositoryRootResourceLimits::new(8 * 1024 * 1024, 16, ARCHIVE_BYTES).unwrap(),
    )
    .unwrap()
}
#[test]
fn capture_exact_and_plus_one_caps_apply_independently_to_each_stream() {
    let bank = bank();
    let baseline = bank.snapshot().retained_bytes;
    {
        let mut captures = capture::Captures::new(&bank).unwrap();
        for stderr in [false, true] {
            for _ in 0..8 {
                captures
                    .drain_with(stderr, |bytes| {
                        bytes.fill(0xff);
                        Ok(bytes.len())
                    })
                    .unwrap();
            }
        }
        assert_eq!(captures.stdout.len(), 1024 * 1024);
        assert_eq!(captures.stderr.len(), 1024 * 1024);
        assert!(captures.stdout.iter().all(|v| *v == 0xff));
        for stderr in [false, true] {
            let e = captures
                .drain_with(stderr, |bytes| {
                    bytes[0] = 0;
                    Ok(1)
                })
                .unwrap_err();
            assert_eq!(e.kind(), io::ErrorKind::FileTooLarge);
        }
    }
    assert_eq!(bank.snapshot().retained_bytes, baseline);
}
#[test]
fn capture_handles_eintr_eagain_eof_and_bounded_turn_fairness() {
    let bank = bank();
    let mut captures = capture::Captures::new(&bank).unwrap();
    let mut calls = 0;
    captures
        .drain_with(false, |bytes| {
            calls += 1;
            match calls {
                1 => Err(io::Error::from_raw_os_error(libc::EINTR)),
                2 => {
                    bytes[..3].copy_from_slice(&[0, 0xff, b'x']);
                    Ok(3)
                }
                _ => Err(io::Error::from_raw_os_error(libc::EAGAIN)),
            }
        })
        .unwrap();
    assert_eq!(captures.stdout, [0, 0xff, b'x']);
    assert!(!captures.out_eof);
    captures.drain_with(false, |_| Ok(0)).unwrap();
    assert!(captures.out_eof);
    captures
        .drain_with(false, |_| panic!("EOF must be sticky"))
        .unwrap();
    let mut reads = 0;
    captures
        .drain_with(true, |bytes| {
            reads += 1;
            bytes[0] = 1;
            Ok(1)
        })
        .unwrap();
    assert_eq!(reads, 16);
    assert_eq!(captures.stderr.len(), 16);
    assert!(!captures.err_eof);
}
#[test]
fn capture_refuses_before_allocation_when_credit_is_insufficient() {
    let resources =
        RepositoryRootResources::new(RepositoryRootResourceLimits::new(1024, 1, 1).unwrap())
            .unwrap();
    let before = resources.snapshot().retained_bytes;
    assert!(matches!(
        capture::Captures::new(&resources),
        Err(PhysicalRootError::Resource)
    ));
    assert_eq!(resources.snapshot().retained_bytes, before);
}
#[test]
fn only_actual_normal_wait_status_with_echild_can_complete() {
    for code in [0, 1, 42, 127, 128, 255] {
        assert_eq!(
            normal_status([code << 8, 1, 0, 0, 0, 0, 0, 0]).unwrap(),
            code as i32
        );
    }
    for raw in [9, 15, 0x7f, 0x137f, 0xffff, 0x10000, u64::MAX] {
        assert!(normal_status([raw, 1, 0, 0, 0, 0, 0, 0]).is_err());
    }
    assert!(normal_status([0; 8]).is_err());
    assert!(normal_status([0, 1, 1, 0, 0, 0, 0, 0]).is_err());
}
#[test]
fn unavailable_closefrom_has_no_unguarded_spawn_fallback() {
    assert_eq!(
        spawn::closefrom_with(|_| std::ptr::null_mut())
            .err()
            .unwrap()
            .kind(),
        io::ErrorKind::Unsupported
    );
}

#[test]
fn borrowed_plan_forecast_checks_exact_cap_plus_one_and_overflow() {
    // Length arithmetic operates on borrowed input lengths; no huge CString or
    // encoded allocation is needed to refuse an oversized semantic request.
    let mut exact = PlanForecast::new();
    exact.item(wire::MAX_PLAN - 58 - 4).unwrap();
    assert_eq!(exact.bytes, wire::MAX_PLAN);
    assert!(matches!(exact.raw(1), Err(PhysicalRootError::Resource)));
    let mut plus_one = PlanForecast::new();
    assert!(matches!(
        plus_one.item(wire::MAX_PLAN - 58 - 3),
        Err(PhysicalRootError::Resource)
    ));
    let mut overflow = PlanForecast::new();
    assert!(matches!(
        overflow.item(usize::MAX),
        Err(PhysicalRootError::Resource)
    ));
    // Mirror scalar bytes are raw24, not a length-prefixed item.
    let mut mirror = PlanForecast::new();
    mirror.item(3).unwrap();
    mirror.raw(24).unwrap();
    mirror.item(7).unwrap();
    assert_eq!(mirror.bytes, 58 + 4 + 3 + 24 + 4 + 7);
    let mut view = PlanForecast::new();
    view.item(9).unwrap();
    view.raw(32).unwrap();
    assert_eq!(view.bytes, 58 + 4 + 9 + 32);
}

#[test]
fn private_monitor_status_requires_actual_terminal_record_not_ordinary_exit() {
    let completed = b"{\"child-pid\":123,\"pid-namespace\":456}\n{\"exit-code\":2}\n";
    assert!(lifecycle::monitor_reaped_status(completed, 2 << 8));
    assert!(!lifecycle::monitor_reaped_status(completed, 1 << 8));
    assert!(!lifecycle::monitor_reaped_status(completed, 9));
    for missing in [
        b"".as_slice(),
        b"{\"child-pid\":123}\n",
        b"{\"exit-code\":2}\n",
        b"{\"child-pid\":0}\n{\"exit-code\":2}\n",
        b"{\"child-pid\":123}\n{\"exit-code\":2,\"extra\":0}\n",
        b"{\"child-pid\":123}\n{\"exit-code\":2}\n{}\n",
    ] {
        assert!(!lifecycle::monitor_reaped_status(missing, 2 << 8));
    }
}

#[test]
fn monitor_record_parser_rejects_duplicates_escapes_and_unknown_fields() {
    for first in [
        "{\"child-pid\":123,\"child-pid\":123}",
        "{\"child-pid\":123,\"unknown-namespace\":456}",
        "{\"child-pid\":123,\"pid-namespace\":456,\"pid-namespace\":456}",
        "{\"child-pid\":1e2}",
        "{\"child-pid\":0123}",
        "{\"child-pid\":-123}",
        "{\"child-pid\":123,}",
        "{\"child\\u002dpid\":123}",
    ] {
        let status = format!("{first}\n{{\"exit-code\":2}}\n");
        assert!(!lifecycle::monitor_reaped_status(status.as_bytes(), 2 << 8));
    }
    assert!(!lifecycle::monitor_reaped_status(
        b"{\"child-pid\":123}\n{\"exit-code\":2}",
        2 << 8
    ));
    assert!(!lifecycle::monitor_reaped_status(
        b"{\"child-pid\":123}\n{\"exit-code\":2,\"exit-code\":2}\n",
        2 << 8
    ));
}

// The generic body is type-checked without invoking run or creating a child.
// This guards the future's Core/RPC handoff contract for every closed launch.
#[allow(dead_code)]
fn run_future_is_send_for_every_launch<L: ProcessLaunch>(launch: L, platform: ProcessPlatform) {
    fn require_send<T: Send>(_: T) {}
    require_send(run(launch, platform, &|| true));
}
