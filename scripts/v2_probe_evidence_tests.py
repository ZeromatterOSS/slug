"""Owned local controls for the fixed supervisor and direct evidence contract."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import time
import unittest

import v2_probe_supervisor as supervisor
import v2_probe_evidence as evidence


ROOT = Path('target/wp1133')
SHARED = ('scripts/v2_probe_supervisor.py', 'scripts/v2_probe_evidence.py',
          'scripts/v2_probe_evidence_tests.py')


def dump(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + '\n')


class DirectEvidenceControls(unittest.TestCase):
    def setUp(self):
        self.roles = json.loads((ROOT / 'roles.json').read_text())

    def test_fixed_role_schema_counts_and_introduction(self):
        evidence.validate_roles(self.roles)
        for key, value in (('extra_owner', 'invented'), ('owner_test_introduction_root', 'target/wp1132'),
                           ('core_counts', {**self.roles['core_counts'], 'current': 51}),
                           ('native_time_policy', {**self.roles['native_time_policy'], 'native_outer_seconds': 33})):
            changed = copy.deepcopy(self.roles)
            changed[key] = value
            with self.assertRaises((AssertionError, KeyError, ValueError)):
                evidence.validate_roles(changed)

    def test_current_root_seam_and_escape_refusal(self):
        changed = copy.deepcopy(self.roles)
        changed['root'] = 'target/wp1134'
        self.assertEqual(evidence.validate_roles(changed, Path('target/wp1134')), Path('target/wp1134'))
        with self.assertRaises(AssertionError):
            evidence.validate_roles(changed, ROOT)
        changed['root'] = 'target/wp1133/../wp1134'
        with self.assertRaises(AssertionError):
            evidence.validate_roles(changed)

    def test_direct_pin_missing_and_wrong_alias_digest(self):
        changed = copy.deepcopy(self.roles)
        path = 'target/wp1131/source-freeze.json'
        del changed['direct_pins'][path]
        with self.assertRaises((AssertionError, KeyError, ValueError)):
            evidence.validate_roles(changed)
        changed = copy.deepcopy(self.roles)
        changed['direct_pins'][str(Path(path).absolute())] = '0' * 64
        with self.assertRaises((AssertionError, KeyError, ValueError)):
            evidence.validate_roles(changed)
        changed = copy.deepcopy(self.roles)
        changed['direct_pins'][path] = '0' * 64
        with self.assertRaises((AssertionError, KeyError, ValueError)):
            evidence.validate_roles(changed)

    def test_native_policy_count_order_and_environment_are_exact(self):
        config = json.loads((ROOT / 'native-config.json').read_text())
        evidence.native_policy_equivalent(self.roles, config)
        for key, value in (('expected_count', 2), ('native_output_bytes', 64 << 20),
                           ('sources', list(reversed(config['sources']))),
                           ('command', config['command'] + ['--ignored']),
                           ('environment', {**config['environment'], 'RUST_BACKTRACE': '1'}),
                           ('native_time_policy', {**config['native_time_policy'], 'qualified_work_seconds': 27})):
            changed = copy.deepcopy(config)
            changed[key] = value
            with self.assertRaises((AssertionError, KeyError, ValueError)):
                evidence.native_policy_equivalent(self.roles, changed)
        for path in ('target/wp1131/source-freeze.json', 'scripts/v2_probe_evidence.py',
                     str(ROOT / 'root-source-checkpoint.json')):
            changed = copy.deepcopy(config)
            changed['pins'].remove(path)
            with self.assertRaises((AssertionError, KeyError, ValueError)):
                evidence.native_policy_equivalent(self.roles, changed)


class SupervisorControls(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory(prefix='owned-control-', dir=ROOT)
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name)
        self.program = self.root / 'child.py'
        self.program.write_text("print('owned local child')\n")
        self.executable = str(Path(sys.executable).resolve())
        self.roles = dict(root=str(self.root), supervisor_path=SHARED[0],
                          baseline='owned fixture', contract_sha256='owned fixture')
        dump(self.root / 'roles.json', self.roles)
        self.checkpoint = dict(verdict='ACCEPT', baseline=self.roles['baseline'],
                               contract_sha256=self.roles['contract_sha256'], source_pins={})
        self.config = dict(mode='list', stem='child', command=[self.executable, str(self.program)],
                           environment={'RUST_BACKTRACE': '0'}, executable=self.executable,
                           executable_sha256=supervisor.digest(self.executable), pins=[])
        self.seal()

    def seal(self):
        dump(self.root / 'roles.json', self.roles)
        pins = {path: supervisor.digest(path) for path in SHARED}
        pins[str(self.root / 'roles.json')] = supervisor.digest(self.root / 'roles.json')
        pins[str(self.program)] = supervisor.digest(self.program)
        self.checkpoint['source_pins'] = pins
        dump(self.root / 'root-source-checkpoint.json', self.checkpoint)
        dump(self.root / 'source-freeze.json', dict(helper_pins=pins))
        self.config['pins'] = list(pins) + [str(self.root / 'root-source-checkpoint.json'),
                                          str(self.root / 'source-freeze.json'), self.executable]
        self.config_path = self.root / 'config.json'
        dump(self.config_path, self.config)

    def reject(self):
        dump(self.config_path, self.config)
        with self.assertRaises((AssertionError, KeyError, ValueError)):
            supervisor.validate_config(self.config_path)
        self.assertFalse((self.root / self.config['stem']).exists())

    def test_aliases_preserve_spelling_and_reject_conflicting_digest(self):
        alias = str(Path(SHARED[0]).absolute())
        self.checkpoint['source_pins'][alias] = supervisor.digest(alias)
        self.config['pins'].append(alias)
        dump(self.root / 'root-source-checkpoint.json', self.checkpoint)
        dump(self.config_path, self.config)
        actual = supervisor.validate_config(self.config_path)[3]
        self.assertEqual(actual[alias], actual[SHARED[0]])
        self.checkpoint['source_pins'][alias] = '0' * 64
        dump(self.root / 'root-source-checkpoint.json', self.checkpoint)
        self.reject()

    def test_required_source_and_checkpoint_pins(self):
        for path in (SHARED[1], str(self.root / 'root-source-checkpoint.json')):
            original = list(self.config['pins'])
            self.config['pins'].remove(path)
            self.reject()
            self.config['pins'] = original

    def test_policy_mode_environment_count_and_root_refuse_before_dispatch(self):
        original = copy.deepcopy(self.config)
        for changes in ({'mode': 'unknown'}, {'rss_bytes': 1 << 30},
                        {'preparation_seconds': 30}, {'expected_count': 0},
                        {'expected_count': True}, {'environment': {'RUST_BACKTRACE': '0',
                                                                   'SLUG_NATIVE_TEST': '1'}},
                        {'native_output_bytes': 64 << 20}):
            self.config = {**copy.deepcopy(original), **changes}
            self.reject()
        self.config = original
        self.roles['root'] = str(self.root / 'other')
        self.seal()
        self.reject()

    def test_real_join_and_current_before_after_pins(self):
        supervisor.main(self.config_path)
        operation = json.loads((self.root / 'child/operation.json').read_text())
        self.assertEqual(operation['exit_code'], 0)
        self.assertIsNone(operation['stop_reason'])
        self.assertTrue(operation['joined'] and operation['owned_group_empty'])
        self.assertEqual(operation['pins_before'], operation['pins_after'])
        self.assertEqual(operation['pass_count'], 0)  # This fixture grants no Rust credit.

    def held_source(self, mutate):
        source = self.root / 'source.txt'
        source.write_text('owned small source\n')
        self.roles['source_owner_pins'] = {path: supervisor.digest(path) for path in
                                          ('target/wp1117/fd_source_r7.py', 'target/wp933/read.py')}
        self.config['sources'] = [dict(path=str(source.absolute()), bytes=source.stat().st_size,
                                       sha256=supervisor.digest(source))]
        if mutate:
            self.program.write_text("from pathlib import Path\nPath(" + repr(str(source)) +
                                    ").write_text('owned changed data\\n')\n")
        self.seal()
        self.config['pins'] += list(self.roles['source_owner_pins'])
        dump(self.config_path, self.config)
        if mutate:
            with self.assertRaises(AssertionError):
                supervisor.main(self.config_path)
        else:
            supervisor.main(self.config_path)
        operation = json.loads((self.root / 'child/operation.json').read_text())
        self.assertTrue(operation['joined'] and operation['owned_group_empty'])
        self.assertEqual(operation['pass_count'], 0)
        self.assertFalse(operation['source_bindings'][0]['fd_inherited'])
        if mutate:
            self.assertIsNotNone(operation['stop_reason'])
            self.assertIsNone(operation['source_bindings'][0]['after'])
        else:
            self.assertIsNone(operation['stop_reason'])
            self.assertEqual(operation['source_bindings'][0]['before'],
                             operation['source_bindings'][0]['after'])

    def test_owned_held_fd_before_after(self):
        self.held_source(False)

    def test_owned_held_fd_change_refuses_after_join(self):
        self.held_source(True)


if __name__ == '__main__':
    started = time.monotonic()
    result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromModule(sys.modules[__name__]))
    record = dict(selected=result.testsRun, passed=result.testsRun - len(result.errors) - len(result.failures),
                  errors=len(result.errors), failures=len(result.failures), elapsed_seconds=time.monotonic() - started,
                  scope='owned local fixtures and metadata; no native/protected Rust credit')
    with (ROOT / 'helper-controls.json').open('x') as stream:
        stream.write(json.dumps(record, indent=2) + '\n')
    raise SystemExit(0 if result.wasSuccessful() else 1)
