"""Fixed bounded probe supervisor; saved evidence never dispatches historical mains."""
import contextlib
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import resource
import signal
import subprocess
import sys
import time

SUPERVISOR_PATH = Path(__file__).absolute()


def digest(path):
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        while block := f.read(65536):
            h.update(block)
    return h.hexdigest()


def members(group):
    result = []
    for path in Path('/proc').iterdir():
        if not path.name.isdecimal():
            continue
        try:
            fields = (path / 'stat').read_text().rsplit(')', 1)[1].split()
            if int(fields[2]) == group:
                result.append((int(path.name), int(fields[21]) * os.sysconf('SC_PAGE_SIZE') // 1024))
        except (OSError, ValueError, IndexError):
            continue
    return result


def stop(group, sig):
    try:
        os.killpg(group, sig)
    except ProcessLookupError:
        pass


@contextlib.contextmanager
def leases(rows, reports, live):
    if not rows:
        yield
        return
    # Accepted owner and tests are pinned explicitly in config; no historical
    # runner imports, gate replay, fixture reconstruction, or map rewriting.
    spec = importlib.util.spec_from_file_location('accepted_fd_owner', 'target/wp1117/fd_source_r7.py')
    owner = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(owner)
    scratch = bytearray(65536)

    def full_hash(fd, row):
        live()
        os.lseek(fd, 0, os.SEEK_SET)
        h = hashlib.sha256()
        total = 0
        while True:
            live()
            count = os.readv(fd, [scratch])
            total += count
            assert total <= row['bytes']
            if not count:
                break
            h.update(memoryview(scratch)[:count])
        assert total == row['bytes'] and h.hexdigest() == row['sha256']
        return dict(bytes=total, sha256=h.hexdigest())

    with contextlib.ExitStack() as stack:
        opened = []
        for row in rows:
            live()
            fd, lease = stack.enter_context(owner.source_fd(row['path'], row['bytes']))
            assert not os.get_inheritable(fd)
            report = dict(path=row['path'], lease=lease, before=full_hash(fd, row), after=None,
                          fd_inherited=False, collection_atomic=False)
            reports.append(report)
            opened.append((fd, row, report))
        try:
            yield
        finally:
            for fd, row, report in opened:
                report['after'] = full_hash(fd, row)


def validate_config(config_path):
    config_path = Path(config_path)
    root = config_path.parent
    config = json.loads(config_path.read_text())
    assert set(config) <= {'mode', 'stem', 'command', 'environment', 'pins', 'executable',
                           'executable_sha256', 'expected_count', 'sources', 'preparation_seconds',
                           'native_output_bytes', 'native_time_policy', 'fixture_output_bytes'}
    roles_path = root / 'roles.json'
    roles = json.loads(roles_path.read_text())
    assert roles['root'] == str(root)
    assert roles['supervisor_path'] == 'scripts/v2_probe_supervisor.py'
    assert Path(roles['supervisor_path']).absolute() == SUPERVISOR_PATH
    assert type(config['stem']) is str and config['stem'] not in ('', '.', '..')
    assert Path(config['stem']).name == config['stem']
    checkpoint_path = root / 'root-source-checkpoint.json'
    freeze_path = root / 'source-freeze.json'
    files = list(dict.fromkeys([str(config_path), str(SUPERVISOR_PATH)] + config['pins']))
    required = [str(roles_path), str(checkpoint_path), str(freeze_path), 'scripts/v2_probe_evidence.py']
    assert all(path in files for path in required)
    freeze = json.loads(freeze_path.read_text())
    checkpoint = json.loads(checkpoint_path.read_text())
    assert checkpoint['verdict'] == 'ACCEPT'
    assert checkpoint['baseline'] == roles['baseline']
    assert checkpoint['contract_sha256'] == roles['contract_sha256']
    expected_pins = list(freeze['helper_pins'].items()) + list(checkpoint['source_pins'].items())
    aliases = {}
    for path, expected in expected_pins:
        actual_path = str(Path(path).resolve())
        assert re.fullmatch('[0-9a-f]{64}', expected)
        assert aliases.setdefault(actual_path, expected) == expected
        assert digest(path) == expected and path in files
    for path in (str(roles_path), roles['supervisor_path'], 'scripts/v2_probe_evidence.py'):
        assert path in freeze['helper_pins'] and path in checkpoint['source_pins']
    assert checkpoint['source_pins'][roles['supervisor_path']] == digest(SUPERVISOR_PATH)
    mode = config['mode']
    assert mode in ('preparation', 'preflight', 'list', 'ordinary', 'native')
    if 'expected_count' in config:
        assert type(config['expected_count']) is int and config['expected_count'] > 0
    seconds = config.get('preparation_seconds', 60) if mode == 'preparation' else (32 if mode == 'native' else 10)
    assert seconds is None or type(seconds) is int
    assert seconds in (None, 60) if mode == 'preparation' else seconds == (32 if mode == 'native' else 10)
    assert mode == 'preparation' or 'preparation_seconds' not in config
    assert config['environment'].get('RUST_BACKTRACE') == '0'
    if mode not in ('preparation', 'native'):
        assert config['environment'] == {'RUST_BACKTRACE': '0'}
    if mode == 'preparation':
        assert not any(name.startswith('SLUG_NATIVE_') for name in config['environment'])
    log_limit = 4 << 20 if mode == 'preparation' else 1 << 20
    file_size_bytes = 16 << 20
    if mode == 'native':
        assert config['stem'] == 'native' and config['native_output_bytes'] == 32 << 20
        roles = json.loads((root / 'roles.json').read_text())
        inherited_path = str(Path(roles['prior']) / 'native-config.json')
        assert digest(inherited_path) == roles['immediate_pins'][inherited_path]
        inherited = json.loads(Path(inherited_path).read_text())
        policy = dict(qualified_work_seconds=26, qualified_cleanup_seconds=29, native_outer_seconds=32,
                      default_work_seconds=5, default_cleanup_seconds=8, ordinary_outer_seconds=10)
        assert config['native_time_policy'] == roles['native_time_policy'] == policy
        assert seconds == policy['native_outer_seconds']
        allowed = {'command', 'executable', 'executable_sha256', 'pins', 'native_time_policy', 'sources'}
        assert {k: v for k, v in config.items() if k not in allowed} == {
            k: v for k, v in inherited.items() if k not in allowed}
        assert config['command'][0] == config['executable']
        assert config['command'][1:] == inherited['command'][1:]
        policy = roles['source_vector_policy']
        historical_root = Path(roles['historical_vector_root'])
        source_root = Path(roles['rust_source_evidence_root'])
        historical = json.loads((historical_root / 'source-vector.json').read_text())
        assert len(config['sources']) == 361 and len(config['environment']) == 18
        assert config['sources'] == inherited['sources'] and len(inherited['sources']) == 361
        assert config['sources'][:360] == historical and len(historical) == policy['current_rows'] == 360
        original_path = Path(roles['original_vector_root']) / 'source-vector.json'
        original = json.loads(original_path.read_text())
        assert len(original) == policy['original_rows'] == 359
        assert digest(original_path) == policy['original_sha256'] == roles['inherited_pins'][str(original_path)]
        assert config['sources'][:359] == original and config['sources'][359:360] == [policy['appended_row']]
        assert config['sources'] == json.loads((root / 'source-vector.json').read_text())
        leaf_policy = roles['rust_source_policy']
        assert leaf_policy['binding_state'] == 'accepted_actual_cohort'
        binding_path = source_root / 'rust-source-binding.json'
        assert str(binding_path) in config['pins'] and digest(binding_path) == leaf_policy['binding_sha256']
        binding = json.loads(binding_path.read_text())
        assert binding['verdict'] == 'ACCEPT'
        assert leaf_policy['relative'] == 'rust/BUILD.bazel'
        assert leaf_policy['inherited_rows'] == 360 and leaf_policy['current_rows'] == 361
        row = binding['row']
        assert row['path'] == str(Path(inherited['environment']['SLUG_NATIVE_RULES_RUST_ROOT']) / leaf_policy['relative'])
        assert type(row['bytes']) is int and 0 <= row['bytes'] <= leaf_policy['body_limit_bytes'] == 65536
        assert re.fullmatch('[0-9a-f]{64}', row['sha256'])
        assert config['sources'][360:] == [row]
        assert len({item['path'] for item in config['sources']}) == 361
        assert digest(root / 'source-vector.json') == binding['source_vector_sha256']
        assert (root / 'source-vector.json').read_bytes() == (Path(roles['prior']) / 'source-vector.json').read_bytes() == (source_root / 'source-vector.json').read_bytes()
        assert (root / 'source-vector.json').read_bytes().startswith((historical_root / 'source-vector.json').read_bytes().rstrip()[:-1])
        assert set(binding['evidence_sha256']) == {str(source_root / name) for name in leaf_policy['evidence_names']}
        for path, expected in binding['evidence_sha256'].items():
            assert path in config['pins'] and digest(path) == expected
        assert digest(historical_root / 'source-vector.json') == policy['current_sha256']
        assert policy['appended_row'] == dict(
            path=str(Path(inherited['environment']['SLUG_NATIVE_BAZEL_SKYLIB_ROOT']) / 'lib/collections.bzl'),
            bytes=2176, sha256='89533929b5b22427531df5505aef56b9e6876bd91b4a56df56b94f0d9c33a1de')
        for path, expected in roles['source_evidence_pins'].items():
            assert path in config['pins'] and digest(path) == expected
        file_size_bytes = config['native_output_bytes']
    elif 'fixture_output_bytes' in config:
        assert mode == 'preflight' and config['stem'] == 'fixture-tests'
        assert config['fixture_output_bytes'] == 32 << 20 and 'native_output_bytes' not in config and 'native_time_policy' not in config
        file_size_bytes = config['fixture_output_bytes']
    else:
        assert 'native_output_bytes' not in config and 'native_time_policy' not in config
    if config.get('sources'):
        assert set(roles['source_owner_pins']) == {'target/wp1117/fd_source_r7.py', 'target/wp933/read.py'}
        for path, expected in roles['source_owner_pins'].items():
            assert path in files and digest(path) == expected
    pins = {p: digest(p) for p in files}
    if mode != 'preparation':
        assert pins[config['executable']] == config['executable_sha256']
    return config, root, files, pins, seconds, log_limit, file_size_bytes


def main(config_path=None):
    if config_path is None:
        assert len(sys.argv) == 2
        config_path = sys.argv[1]
    config, root, files, pins, seconds, log_limit, file_size_bytes = validate_config(config_path)
    mode = config['mode']
    directory = root / config['stem']
    directory.mkdir()
    out, err = directory / 'stdout', directory / 'stderr'
    env = os.environ.copy()
    # Do not let ambient diagnostic opt-ins broaden an ordinary gate.
    for name in list(env):
        if name.startswith('SLUG_NATIVE_'):
            del env[name]
    env.update(config['environment'])
    if mode == 'native':
        assert env['SLUG_NATIVE_PROTOBUF_OFFLINE_ARCHIVE'] == 'true'
    reports = []
    sampled = 0
    reason = None
    exit_code = None
    joined = False
    empty = False
    group = None
    start = time.monotonic()

    def live():
        if seconds is not None and time.monotonic() - start > seconds:
            raise TimeoutError('whole_operation_deadline')

    def cap():
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
        if mode != 'preparation':
            resource.setrlimit(resource.RLIMIT_AS, (512 << 20, 512 << 20))
            resource.setrlimit(resource.RLIMIT_FSIZE, (file_size_bytes, file_size_bytes))

    try:
        with leases(config.get('sources', []), reports, live):
            live()
            with out.open('xb') as stdout, err.open('xb') as stderr:
                child = subprocess.Popen(config['command'], stdin=subprocess.DEVNULL,
                                         stdout=stdout, stderr=stderr, env=env,
                                         start_new_session=True, preexec_fn=cap)
                group = child.pid
                cleanup_deadline = None
                try:
                    while child.poll() is None:
                        rss = sum(v for _, v in members(group))
                        sampled = max(sampled, rss)
                        if seconds is not None and time.monotonic() - start > seconds:
                            reason = 'deadline'
                        elif mode != 'preparation' and rss > 262144:
                            reason = 'rss'
                        elif out.stat().st_size > log_limit or err.stat().st_size > log_limit:
                            reason = 'log_limit'
                        if reason:
                            stop(group, signal.SIGTERM)
                            break
                        time.sleep(.01)
                    cleanup_deadline = time.monotonic() + 3
                    grace_deadline = min(cleanup_deadline, time.monotonic() + 1)
                    while child.poll() is None and time.monotonic() < grace_deadline:
                        time.sleep(.01)
                    if child.poll() is None:
                        stop(group, signal.SIGKILL)
                    exit_code = child.wait(timeout=max(.01, cleanup_deadline-time.monotonic()))
                    joined = True
                finally:
                    if cleanup_deadline is None:
                        cleanup_deadline = time.monotonic() + 3
                    if child.poll() is None:
                        stop(group, signal.SIGKILL)
                        child.wait(timeout=max(.01, cleanup_deadline-time.monotonic()))
                        joined = True
                    # A child exit cannot leave owned descendants behind.
                    if members(group):
                        stop(group, signal.SIGTERM)
                        grace_deadline = min(cleanup_deadline, time.monotonic() + 1)
                        while members(group) and time.monotonic() < grace_deadline:
                            time.sleep(.01)
                        if members(group):
                            stop(group, signal.SIGKILL)
                        while members(group) and time.monotonic() < cleanup_deadline:
                            time.sleep(.01)
                    empty = not members(group)
                    if not empty and reason is None:
                        reason = 'owned_group_not_empty'
            assert out.stat().st_size <= log_limit and err.stat().st_size <= log_limit
    except Exception as error:
        reason = reason or type(error).__name__ + ':' + str(error)
    after = {}
    for p in files:
        try:
            live()
            after[p] = digest(p)
            live()
        except Exception as error:
            after[p] = None
            reason = reason or type(error).__name__ + ':' + str(error)
    if pins != after:
        reason = reason or 'pins_changed'
    peak = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    if mode != 'preparation' and peak > 262144:
        reason = reason or 'rusage_rss'
    if seconds is not None and time.monotonic() - start > seconds:
        reason = reason or 'whole_operation_deadline'
    raw = ''
    streams = {}
    for path in (out, err):
        try:
            if path.stat().st_size > log_limit:
                raise ValueError('log_limit')
            streams[str(path)] = digest(path)
            if path == out:
                raw = path.read_text(errors='replace')
        except Exception as error:
            streams[str(path)] = None
            reason = reason or type(error).__name__ + ':' + str(error)
    selected = passed = 0
    if mode in ('ordinary', 'native'):
        running = re.search(r'^running (\d+) tests?$', raw, re.M)
        summary = re.search(r'^test result: ok\. (\d+) passed; 0 failed;', raw, re.M)
        selected = int(running.group(1)) if running else 0
        passed = int(summary.group(1)) if summary else 0
        expected = config.get('expected_count', 1)
        if expected <= 0 or selected != expected or passed != expected:
            reason = reason or 'test_count_mismatch'
    elapsed = time.monotonic()-start
    if seconds is not None and elapsed > seconds:
        reason = reason or 'whole_operation_deadline'
    observed_passes = passed
    if reason is not None or exit_code != 0 or not joined or not empty:
        passed = 0
    record = dict(mode=mode, file_size_bytes=file_size_bytes, deadline_seconds=seconds, command=config['command'], environment=config['environment'],
                  elapsed_seconds=elapsed, exit_code=exit_code,
                  stop_reason=reason, selected_count=selected, pass_count=passed,
                  observed_test_passes=observed_passes,
                  joined=joined, owned_group_empty=empty, monitored_rss_kib=sampled,
                  child_rusage_peak_kib=peak,
                  pins_before=pins, pins_after=after, source_bindings=reports,
                  stdout_sha256=streams[str(out)], stderr_sha256=streams[str(err)])
    with (directory / 'operation.json').open('x') as f:
        json.dump(record, f, indent=2)
        f.write('\n')
    print(json.dumps({k:record[k] for k in ('mode','elapsed_seconds','exit_code','stop_reason',
                                          'pass_count','monitored_rss_kib')}))
    assert exit_code == 0 and reason is None and joined and empty


if __name__ == '__main__':
    main()
