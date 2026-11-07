#!/usr/bin/python3
"""Root-authorized WP425 experiment only. No original Starlark or user process."""
import argparse
import array
import hashlib
import fcntl
import json
import os
from pathlib import Path
import selectors
import signal
import socket
import stat
import subprocess
import sys
import tempfile
import time

PROTOCOL_CAP = 8192
STDERR_CAP = 65536
FIRST = b"WP425-first-root\n".ljust(4096, b"a")
SECOND = b"WP425-second-root\n".ljust(4096, b"b")
THIRD = b"WP425-third-root\n".ljust(4096, b"c")


def sha(path):
    digest = hashlib.sha256()
    with open(path, "rb") as source:
        for part in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(part)
    return digest.hexdigest()


def ident(fd):
    item = os.fstat(fd)
    if not stat.S_ISDIR(item.st_mode):
        raise RuntimeError("received non-directory")
    return {"dev": item.st_dev, "ino": item.st_ino, "mode": item.st_mode}


def namespace_ident(fd):
    if fcntl.ioctl(fd, 0xb703) != 0x10000000:
        raise RuntimeError("received descriptor is not a user namespace")
    item = os.fstat(fd)
    return {"dev": item.st_dev, "ino": item.st_ino}


def read_sentinel(root, name, expected):
    fd = os.open(name, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW, dir_fd=root)
    try:
        if os.read(fd, len(expected) + 1) != expected:
            raise RuntimeError("parent openat sentinel mismatch: " + name)
    finally:
        os.close(fd)


def terminate(child, record):
    # Only signal an unreaped live child owned by this Popen, never an old PID.
    if child.poll() is None:
        record["termination"] = "TERM_THEN_KILL_IF_NEEDED"
        try:
            os.killpg(child.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            child.wait(timeout=0.5)
        except subprocess.TimeoutExpired:
            if child.poll() is None:
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            child.wait(timeout=2.0)
    record["reaped_returncode"] = child.returncode


def run_phase(plan, phase, display, root, output, whole_deadline, receipt, owned_received):
    if time.monotonic() >= whole_deadline:
        raise TimeoutError("whole deadline before phase launch")
    parent, child_socket = socket.socketpair(socket.AF_UNIX, socket.SOCK_SEQPACKET)
    parent.setblocking(False)
    nonce = os.urandom(16).hex()
    namespace_args = ["--unshare-user"] if root is None else ["--userns", str(root[2])]
    argv = [plan["bwrap"]] + namespace_args + ["--unshare-pid", "--unshare-net",
            "--unshare-ipc", "--unshare-uts", "--die-with-parent", "--new-session",
            "--uid", "0", "--gid", "0",
            "--clearenv", "--cap-drop", "ALL", "--cap-add", "CAP_SYS_ADMIN",
            "--ro-bind", "/", "/"]
    if phase == "first":
        argv += ["--size", "131072", "--tmpfs", display]
    argv += ["--proc", "/proc", "--dev", "/dev", "--chdir", display, "--",
             plan["python"], "-I", "-S", plan["helper"], phase, display, nonce]
    record = {"phase": phase, "argv": argv, "env": {}, "stdin": "DEVNULL",
              "root_authority": "NONE" if root is None else "SCM_RIGHTS_AFTER_READY",
              "stdout": "AF_UNIX_SEQPACKET_CONTROL_FD1",
              "stderr_cap": STDERR_CAP, "execution_deadline_seconds": 17,
              "cleanup_allowance_seconds": 2.5, "per_phase_max_seconds": 20,
              "start_monotonic": time.monotonic(), "messages": [], "child_launched": False,
              "no_unreaped_child": True}
    receipt["phases"].append(record)
    child = None
    received = []
    stderr = bytearray()
    protocol = bytearray()
    selector = selectors.DefaultSelector()
    failure = None
    try:
        try:
            child = subprocess.Popen(argv, stdin=subprocess.DEVNULL,
                                     stdout=child_socket, stderr=subprocess.PIPE,
                                     close_fds=True, pass_fds=() if root is None else (root[2],),
                                     env={}, start_new_session=True)
            record["child_launched"] = True
            record["no_unreaped_child"] = False
            record["owned_launcher_pid"] = child.pid
            child_socket.close()
            os.set_blocking(child.stderr.fileno(), False)
            selector.register(parent, selectors.EVENT_READ, "protocol")
            selector.register(child.stderr, selectors.EVENT_READ, "stderr")
            deadline = min(whole_deadline, time.monotonic() + 17.0)
            while selector.get_map():
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise TimeoutError("bounded phase deadline")
                for key, _ in selector.select(min(remaining, 0.1)):
                    if key.data == "stderr":
                        data = os.read(key.fileobj.fileno(), 4096)
                        if not data:
                            selector.unregister(key.fileobj)
                        else:
                            room = STDERR_CAP - len(stderr)
                            stderr.extend(data[:room])
                            if len(data) > room:
                                record["stderr_overflow"] = True
                                raise RuntimeError("stderr exceeded fixed capture cap")
                    else:
                        data, ancillary, flags, _ = parent.recvmsg(
                            PROTOCOL_CAP, socket.CMSG_SPACE(4 * array.array("i").itemsize),
                            getattr(socket, "MSG_CMSG_CLOEXEC", 0))
                        fresh = []
                        for level, kind, payload in ancillary:
                            if level != socket.SOL_SOCKET or kind != socket.SCM_RIGHTS:
                                raise RuntimeError("unexpected ancillary control")
                            integers = array.array("i")
                            integers.frombytes(payload[:len(payload) - len(payload) % integers.itemsize])
                            fresh.extend(integers)
                            received.extend(integers)
                            owned_received.update(integers)
                        if flags & (socket.MSG_TRUNC | socket.MSG_CTRUNC):
                            raise RuntimeError("truncated protocol or descriptor control")
                        if not data:
                            if fresh:
                                raise RuntimeError("descriptors on empty protocol")
                            selector.unregister(parent)
                            continue
                        if len(protocol) + len(data) > PROTOCOL_CAP:
                            raise RuntimeError("cumulative protocol capture exceeds fixed cap")
                        protocol.extend(data)
                        message = json.loads(data.decode("ascii"))
                        record["messages"].append(message)
                        if message.get("kind") == "ready":
                            if (phase not in ("second", "third") or len(record["messages"]) != 1 or fresh
                                    or message.get("phase") != phase or message.get("nonce") != nonce):
                                raise RuntimeError("unexpected ready message")
                            descriptors = array.array("i", root[:2])
                            request = ("ROOT:" + nonce).encode("ascii")
                            if parent.sendmsg([request], [(socket.SOL_SOCKET, socket.SCM_RIGHTS,
                                                           descriptors.tobytes())]) != len(request):
                                raise RuntimeError("short root-authority handoff")
                            record["root_authority_handoff"] = {"root": ident(root[0]), "userns": namespace_ident(root[2]),
                                                                "after_actual_ready": True}
                            continue
                        expected_messages = 1 if phase == "first" else 2
                        if len(record["messages"]) != expected_messages or len(fresh) != (3 if phase == "first" else 2 if phase == "second" else 1):
                            raise RuntimeError("unexpected root message or descriptor count")
                        if (message.get("kind"), message.get("phase"), message.get("nonce")) != (
                                "root", phase, nonce):
                            raise RuntimeError("protocol identity mismatch")
                        if message.get("environment_empty") is not True:
                            raise RuntimeError("helper did not confirm empty environment")
                        if ident(fresh[0]) != message["root"]:
                            raise RuntimeError("received descriptor differs from helper fstat")
                        if phase == "first":
                            if ident(fresh[1]) != ident(fresh[0]) or namespace_ident(fresh[2]) != message["userns"]:
                                raise RuntimeError("detached mount/user namespace identity mismatch")
                        elif message.get("userns") != namespace_ident(root[2]):
                            raise RuntimeError("fresh helper is not in saved mount owner user namespace")
                        if phase == "second" and ident(fresh[1]) != ident(fresh[0]):
                            raise RuntimeError("successor detached lease differs from attached root")
                        if root is not None and message.get("received_root") != ident(root[0]):
                            raise RuntimeError("child root differs from original parent authority")
                        record["root_message"] = message
                        parent.send(("ACK:" + nonce).encode("ascii"))
            child.wait(timeout=max(0.001, deadline - time.monotonic()))
            record["reaped_returncode"] = child.returncode
            if (child.returncode != 0 or len(record["messages"]) != (1 if phase == "first" else 2)
                    or len(received) != (3 if phase == "first" else 2 if phase == "second" else 1) or "root_message" not in record):
                raise RuntimeError("launcher/helper did not complete the full fixed protocol")
        except BaseException as error:
            failure = {"type": type(error).__name__, "text": str(error)}
            record["failure"] = failure
            raise
        finally:
            # No descriptor or child is allowed to outlive this phase on failure.
            try:
                if child is not None:
                    terminate(child, record)
            finally:
                record["no_unreaped_child"] = child is None or child.returncode is not None
                selector.close()
                parent.close()
                child_socket.close()
                if child is not None and child.stderr is not None:
                    try:
                        tail = child.stderr.read(STDERR_CAP - len(stderr) + 1) or b""
                        room = STDERR_CAP - len(stderr)
                        stderr.extend(tail[:room])
                        if len(tail) > room:
                            record["stderr_overflow"] = True
                    finally:
                        child.stderr.close()
                path = output / (phase + ".stderr")
                path.write_bytes(stderr)
                record["stderr_path"] = str(path)
                record["stderr_sha256"] = sha(path)
                protocol_path = output / (phase + ".protocol.bin")
                protocol_path.write_bytes(protocol)
                record["protocol_path"] = str(protocol_path)
                record["protocol_sha256"] = sha(protocol_path)
                record["elapsed_seconds"] = time.monotonic() - record["start_monotonic"]
        result = tuple(received) if phase in ("first", "second") else received[0]
        record["returned_descriptors_owned_by_parent"] = all(fd in owned_received for fd in received)
        received.clear()
        return result
    except BaseException as error:
        record.setdefault("failure", {"type": type(error).__name__, "text": str(error)})
        for descriptor in list(received):
            try:
                os.close(descriptor)
            except OSError as close_error:
                record.setdefault("descriptor_close_errors", []).append(
                    {"errno": close_error.errno, "text": str(close_error)})
            else:
                received.remove(descriptor)
                owned_received.remove(descriptor)
        raise


def main():
    started = time.monotonic()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", type=Path, required=True)
    parser.add_argument("--plan-sha256", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--ack-experiment-only", action="store_true", required=True)
    args = parser.parse_args()
    plan_path = args.plan.resolve(strict=True)
    if sha(plan_path) != args.plan_sha256:
        raise RuntimeError("frozen plan hash mismatch")
    plan = json.loads(plan_path.read_text())
    output = args.output.resolve()
    configured_root = Path(plan["parent_only_output_root"])
    if not configured_root.is_absolute():
        raise RuntimeError("frozen output root must be absolute")
    allowed = configured_root.resolve()
    if not output.is_relative_to(allowed) or output == allowed:
        raise RuntimeError("output must be a fresh child of the frozen parent-only output root")
    output.mkdir(parents=True, exist_ok=False)
    receipt = {"status": "STARTED_NOT_PROOF", "plan_path": str(plan_path),
               "plan_sha256": args.plan_sha256, "phases": [],
               "security_claim": False, "production_activation": False,
               "original_execution": False, "nr_inodes_gate": "UNPROVED",
               "platform_readiness": "UNPROVED_CONFINEMENT_AND_SOURCE_OBSERVER_NOT_TESTED",
               "start_monotonic": started, "provenance": {}}
    root = None
    second = None
    reservation = None
    owned_received = set()
    try:
        if sys.platform != "linux" or os.uname().machine != "x86_64":
            raise RuntimeError("frozen experiment is Linux x86_64 only")
        for path, expected in plan["pins"].items():
            actual = sha(path)
            receipt["provenance"][path] = actual
            if actual != expected:
                raise RuntimeError("actual input differs from frozen pin: " + path)
        if Path(sys.executable).resolve() != Path(plan["python"]).resolve():
            raise RuntimeError("parent interpreter is not the frozen Python executable")
        deadline = started + 55.0  # Leaves bounded cleanup below the 60s whole budget.
        reservation = tempfile.TemporaryDirectory(prefix="slug-rootfd-probe-", dir="/tmp")
        display = str(Path(reservation.name) / "root")
        os.mkdir(display, 0o700)
        receipt["owned_display_root"] = display
        root = run_phase(plan, "first", display, None, output, deadline, receipt, owned_received)
        first_identity = ident(root[0])
        read_sentinel(root[0], "first", FIRST)
        receipt["after_first_launcher_reaped"] = {"root": first_identity,
                                                  "parent_openat_first_matches": True}
        second = run_phase(plan, "second", display, root, output, deadline, receipt, owned_received)
        if ident(second[0]) != first_identity or ident(root[0]) != first_identity:
            raise RuntimeError("fresh namespace did not bind the exact retained root")
        read_sentinel(root[0], "first", FIRST)
        read_sentinel(root[0], "second", SECOND)
        receipt["after_second_launcher_reaped"] = {"root": ident(root[0]), "userns": namespace_ident(root[2]),
                                                   "parent_openat_second_matches": True}
        next_lease = (root[0], second[1], root[2])
        third = run_phase(plan, "third", display, next_lease, output, deadline, receipt, owned_received)
        if ident(third) != first_identity:
            raise RuntimeError("third attachment differs from original retained root")
        read_sentinel(root[0], "first", FIRST)
        read_sentinel(root[0], "second", THIRD)
        receipt["after_third_launcher_reaped"] = {"root": ident(root[0]),
                                                  "parent_original_fd_reads_third_write": True}
        quota_record = receipt["phases"][1]["root_message"].get("quota", {})
        if quota_record.get("status") != "EXACT_65536_DATA_BYTES_THEN_ONE_BYTE_ENOSPC":
            raise RuntimeError("actual aggregate data ceiling proof missing")
        for path, expected in plan["pins"].items():
            if sha(path) != expected:
                raise RuntimeError("pinned input changed during experiment: " + path)
        inode = receipt["phases"][1]["root_message"].get("inode_quota", {})
        setup = receipt["phases"][0]["root_message"].get("inode_setup", {})
        if (inode.get("status") == "EXACT_ACTUAL_64_INODES_THEN_ONE_INODE_ENOSPC"
                and setup.get("status") == "REMOUNT_SUCCEEDED"):
            receipt["nr_inodes_gate"] = "ACTUAL_64_INODES_EXACT_PLUS_ONE_CONFIRMED"
        receipt["status"] = ("ROLLING_LEASE_THREE_PHASE_QUOTA_TRANSITION_CONFIRMED_"
                             + ("INODES_CONFIRMED" if receipt["nr_inodes_gate"].startswith("ACTUAL_")
                                else "INODES_UNPROVED"))
    except BaseException as error:
        receipt["status"] = "REFUSED_OR_FAILED_NO_PLATFORM_PROOF"
        receipt["failure"] = {"type": type(error).__name__, "text": str(error)}
    finally:
        for descriptor in list(owned_received):
            try:
                os.close(descriptor)
            except OSError as close_error:
                receipt.setdefault("descriptor_close_errors", []).append(
                    {"errno": close_error.errno, "text": str(close_error)})
            else:
                owned_received.remove(descriptor)
        receipt["unclosed_received_descriptor_count"] = len(owned_received)
        receipt["all_received_root_descriptors_closed"] = not owned_received
        receipt["all_owned_launchers_reaped"] = all(
            not phase["child_launched"] or phase.get("reaped_returncode") is not None
            for phase in receipt["phases"])
        if reservation is not None:
            reservation.cleanup()
            receipt["owned_host_reservation_removed"] = not Path(reservation.name).exists()
        receipt["elapsed_seconds"] = time.monotonic() - receipt["start_monotonic"]
        receipt["whole_operational_deadline_seconds"] = 60
        if (receipt["elapsed_seconds"] > 60 or not receipt["all_owned_launchers_reaped"]
                or not receipt["all_received_root_descriptors_closed"]):
            receipt["status"] = "LIFECYCLE_BOUND_FAILED_NO_PLATFORM_PROOF"
        (output / "receipt.json").write_text(json.dumps(receipt, sort_keys=True, indent=2) + "\n")
    return 0 if receipt["status"].startswith("ROLLING_LEASE_") else 1


if __name__ == "__main__":
    raise SystemExit(main())
