_FIRST = """
import errno
import os
from pathlib import Path
import socket

assert Path.cwd() == Path(Path('root.txt').read_text())
root = Path('.').stat()
Path('root-id').write_text(f'{root.st_dev}:{root.st_ino}')
status = dict(line.split(':', 1) for line in Path('/proc/self/status').read_text().splitlines())
for name in ('CapInh', 'CapPrm', 'CapEff', 'CapBnd', 'CapAmb'):
    assert int(status[name], 16) == 0, name
assert status['NoNewPrivs'].strip() == '1'
assert status['Seccomp'].strip() == '2'

for entry in os.listdir('/proc/self/fd'):
    fd = int(entry)
    if fd < 3:
        continue
    try:
        os.fstat(fd)
    except OSError as error:
        assert error.errno == errno.EBADF, fd
    else:
        raise AssertionError(('inherited descriptor', fd))

try:
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM):
        pass
except OSError as error:
    assert error.errno == errno.EPERM
else:
    raise AssertionError('payload created a Unix socket')

Path('out').write_text('written')
print('confinement-ok')
raise SystemExit(7)
"""

_SECOND = """
from pathlib import Path

assert Path.cwd() == Path(Path('root.txt').read_text())
root = Path('.').stat()
assert Path('root-id').read_text() == f'{root.st_dev}:{root.st_ino}'
out = Path('out')
assert not out.exists()
out.write_text(Path('rendered').read_text() + '!')
"""

def _repository(ctx):
    root = str(ctx.path("."))
    if not root.startswith("/__slug_repositories__/v1/"):
        fail("unreserved repository path")
    ctx.file("root.txt", root, executable = False)
    first = ctx.execute(["/usr/bin/python3", "-I", "-c", _FIRST], quiet = False)
    if type(first) != "exec_result" or first.return_code != 7 or first.stdout != "confinement-ok\n":
        fail("first Execute failed qualification: %s" % first.stderr)
    if not ctx.path("out").exists or ctx.read(ctx.path("out")) != "written":
        fail("first Execute did not publish its actual output")
    ctx.template("rendered", ctx.path("out"), {"written": "rendered"}, executable = False)
    if not ctx.delete(ctx.path("out")):
        fail("trusted delete did not remove the first output")
    second = ctx.execute(["/usr/bin/python3", "-I", "-c", _SECOND])
    if second.return_code != 0 or ctx.read("out") != "rendered!":
        fail("continuation did not preserve the same repository root: %s" % second.stderr)
    ctx.file("BUILD.bazel", "exports_files(['out'])\n", executable = False)

_proof = repository_rule(implementation = _repository)

def _extension(_ctx):
    _proof(name = "proof")

extension = module_extension(implementation = _extension)
