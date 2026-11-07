#!/usr/bin/python3
"""Trusted, fixed-operation WP425 experiment only; never a production runner."""
import array
import ctypes
import errno
import fcntl
import json
import os
import socket
import stat
import sys

PAGE = 4096
LIMIT = 65536
FIRST = b"WP425-first-root\n".ljust(PAGE, b"a")
SECOND = b"WP425-second-root\n".ljust(PAGE, b"b")
THIRD = b"WP425-third-root\n".ljust(PAGE, b"c")


class StatFs(ctypes.Structure):
    # Frozen experiment platform: Linux x86_64 glibc bits/statfs.h, pinned in plan.
    _fields_ = [("f_type", ctypes.c_long), ("f_bsize", ctypes.c_long),
                ("f_blocks", ctypes.c_ulong), ("f_bfree", ctypes.c_ulong),
                ("f_bavail", ctypes.c_ulong), ("f_files", ctypes.c_ulong),
                ("f_ffree", ctypes.c_ulong), ("f_fsid", ctypes.c_int * 2),
                ("f_namelen", ctypes.c_long), ("f_frsize", ctypes.c_long),
                ("f_flags", ctypes.c_long), ("f_spare", ctypes.c_long * 4)]


def mount(source, target, fs_type, flags, data):
    libc = ctypes.CDLL(None, use_errno=True)
    function = libc.mount
    function.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p,
                         ctypes.c_ulong, ctypes.c_char_p]
    function.restype = ctypes.c_int
    encoded = [None if item is None else os.fsencode(item)
               for item in (source, target, fs_type, data)]
    if function(encoded[0], encoded[1], encoded[2], flags, encoded[3]) != 0:
        number = ctypes.get_errno()
        raise OSError(number, os.strerror(number), target)


def namespace_identity(fd):
    if fcntl.ioctl(fd, 0xb703) != 0x10000000:  # NS_GET_NSTYPE, CLONE_NEWUSER
        raise RuntimeError("not an actual user namespace descriptor")
    value = os.fstat(fd)
    return {"dev": value.st_dev, "ino": value.st_ino}


def mount_owner():
    mount_ns = os.open("/proc/self/ns/mnt", os.O_RDONLY | os.O_CLOEXEC)
    owner = None
    current = None
    try:
        owner = fcntl.ioctl(mount_ns, 0xb701)  # NS_GET_USERNS returns an owned FD.
        os.set_inheritable(owner, False)
        current = os.open("/proc/self/ns/user", os.O_RDONLY | os.O_CLOEXEC)
        if namespace_identity(owner) != namespace_identity(current):
            raise RuntimeError("mount owner differs from helper user namespace")
        result = owner
        owner = None
        return result
    finally:
        os.close(mount_ns)
        if current is not None:
            os.close(current)
        if owner is not None:
            os.close(owner)


def clone_mount(root):
    libc = ctypes.CDLL(None, use_errno=True)
    # Frozen Linux x86_64 syscall numbers pinned to asm/unistd_64.h.
    function = libc.syscall
    function.restype = ctypes.c_long
    result = function(ctypes.c_long(428), ctypes.c_int(root), ctypes.c_char_p(b""),
                      ctypes.c_uint(1 | os.O_CLOEXEC | 0x1000))
    if result < 0:
        number = ctypes.get_errno()
        raise OSError(number, os.strerror(number), "open_tree")
    return result


def attach_mount(descriptor, path):
    libc = ctypes.CDLL(None, use_errno=True)
    function = libc.syscall
    function.restype = ctypes.c_long
    result = function(ctypes.c_long(429), ctypes.c_int(descriptor), ctypes.c_char_p(b""),
                      ctypes.c_int(-100), ctypes.c_char_p(os.fsencode(path)), ctypes.c_uint(4))
    if result < 0:
        number = ctypes.get_errno()
        raise OSError(number, os.strerror(number), "move_mount")


def filesystem(root):
    libc = ctypes.CDLL(None, use_errno=True)
    function = libc.fstatfs
    function.argtypes = [ctypes.c_int, ctypes.POINTER(StatFs)]
    function.restype = ctypes.c_int
    actual = StatFs()
    if function(root, ctypes.byref(actual)) != 0:
        number = ctypes.get_errno()
        raise OSError(number, os.strerror(number))
    if actual.f_type != 0x01021994:
        raise RuntimeError("actual retained filesystem is not tmpfs")
    return {"f_type": actual.f_type, "f_bsize": actual.f_bsize,
            "struct_size": ctypes.sizeof(StatFs)}


def inode_quota(root):
    # Isolate inode exhaustion from the independently recorded full byte quota.
    os.unlink("quota-00", dir_fd=root)
    before = os.fstatvfs(root)
    def counters(item):
        return {"f_files": item.f_files, "f_ffree": item.f_ffree,
                "f_bavail": item.f_bavail, "f_frsize": item.f_frsize}
    record = {"initial": counters(before)}
    if before.f_files != 64 or before.f_ffree > 64 or before.f_bavail == 0:
        return dict(record, status="UNPROVED_INODE_TEST_PREREQUISITE")
    for index in range(64):
        prior = os.fstatvfs(root)
        try:
            write_fixed(root, "inode-%02d" % index, b"")
        except OSError as error:
            after = os.fstatvfs(root)
            record.update(successful_new_empty_files=index,
                          before_refusal=counters(prior), after_refusal=counters(after),
                          refusal_errno=error.errno, refusal_text=str(error))
            if error.errno != errno.ENOSPC:
                raise
            record["status"] = (
                "EXACT_ACTUAL_64_INODES_THEN_ONE_INODE_ENOSPC"
                if prior.f_ffree == 0 and after.f_ffree == 0
                and after.f_files == 64 and prior.f_bavail > 0 and after.f_bavail > 0
                else "UNPROVED_EARLY_ENOSPC_WITH_RECORDED_COUNTERS")
            return record
        record["last_success"] = {"index": index, "before": counters(prior),
                                  "after": counters(os.fstatvfs(root))}
    return dict(record, status="UNPROVED_INODE_CEILING_DID_NOT_REFUSE")


def identity(fd):
    value = os.fstat(fd)
    if not stat.S_ISDIR(value.st_mode):
        raise RuntimeError("root descriptor is not a directory")
    return {"dev": value.st_dev, "ino": value.st_ino, "mode": value.st_mode}


def read_exact(root, name, expected):
    fd = os.open(name, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW, dir_fd=root)
    try:
        value = os.read(fd, PAGE + 1)
        if value != expected:
            raise RuntimeError("fixed sentinel mismatch: " + name)
    finally:
        os.close(fd)


def write_fixed(root, name, body):
    fd = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC
                 | os.O_NOFOLLOW, 0o600, dir_fd=root)
    written = 0
    try:
        while written < len(body):
            count = os.write(fd, body[written:])
            if count <= 0:
                raise RuntimeError("zero write")
            written += count
    finally:
        os.close(fd)
    return written


def quota(root):
    """Two sentinels already occupy exactly two data pages; no inode claim."""
    before = os.fstatvfs(root)
    if before.f_frsize != PAGE or before.f_blocks * before.f_frsize != LIMIT:
        raise RuntimeError("actual tmpfs data layout differs from frozen quota test")
    written = 2 * PAGE
    files = []
    # This hard limit also bounds attempted writes if --size were ineffective.
    for index in range(32):
        if written == LIMIT:
            break
        name = "quota-%02d" % index
        written += write_fixed(root, name, b"q" * PAGE)
        files.append(name)
    if written != LIMIT:
        raise RuntimeError("did not reach the exact fixed data ceiling")
    refusal = None
    try:
        write_fixed(root, "quota-plus-one", b"!")
    except OSError as error:
        refusal = {"errno": error.errno, "text": str(error)}
        if error.errno != errno.ENOSPC:
            raise
    if refusal is None:
        raise RuntimeError("aggregate data ceiling plus one byte was writable")
    after = os.fstatvfs(root)
    return {
        "status": "EXACT_65536_DATA_BYTES_THEN_ONE_BYTE_ENOSPC",
        "known_file_payload_bytes": written,
        "additional_full_page_files": files,
        "plus_one_refusal": refusal,
        "f_frsize": after.f_frsize,
        "f_blocks": after.f_blocks,
        "f_bavail": after.f_bavail,
        "f_files": after.f_files,
        "f_ffree": after.f_ffree,
        "qualification": "Page-aligned fixed-file data ceiling only; metadata/inodes are separate.",
    }


def main():
    if len(sys.argv) != 4 or sys.argv[1] not in ("first", "second", "third"):
        raise RuntimeError("expected fixed phase, absolute reserved root, nonce")
    phase, path, nonce = sys.argv[1:]
    if not path.startswith("/tmp/slug-rootfd-probe-") or not path.endswith("/root"):
        raise RuntimeError("not the fixed owned reservation shape")
    control = socket.socket(fileno=1)
    control.settimeout(5.0)
    # CPython may install LC_CTYPE during locale coercion despite execve env={};
    # record that runtime detail, then clear before any experiment operation.
    startup_keys = sorted(os.environ)
    startup_locale = os.environ.get("LC_CTYPE")
    os.environ.clear()
    if os.geteuid() != 0 or os.getegid() != 0:
        raise RuntimeError("trusted helper was not mapped to namespace uid/gid zero")
    root = None
    detached = None
    owner = None
    inherited = []
    try:
        setup = None
        if phase == "first":
            # Trusted setup only: exact tmpfs owner, no original/untrusted payload.
            try:
                mount(None, path, "tmpfs", 32 | 2 | 4, "size=131072,nr_inodes=128")
                setup = {"status": "REMOUNT_SUCCEEDED", "requested_nr_inodes": 128}
            except OSError as error:
                setup = {"status": "REMOUNT_REFUSED_INODES_UNPROVED", "errno": error.errno,
                         "text": str(error)}
        else:
            # Start Python with normal stdin; receive authority only after startup.
            ready = json.dumps({"kind": "ready", "phase": phase, "nonce": nonce}).encode("ascii")
            if control.send(ready) != len(ready):
                raise RuntimeError("short ready send")
            data, ancillary, flags, _ = control.recvmsg(
                256, socket.CMSG_SPACE(4 * array.array("i").itemsize),
                getattr(socket, "MSG_CMSG_CLOEXEC", 0))
            for level, kind, payload in ancillary:
                if level != socket.SOL_SOCKET or kind != socket.SCM_RIGHTS:
                    raise RuntimeError("unexpected input ancillary control")
                integers = array.array("i")
                integers.frombytes(payload[:len(payload) - len(payload) % integers.itemsize])
                inherited.extend(integers)
            if (flags & (socket.MSG_TRUNC | socket.MSG_CTRUNC) or len(inherited) != 2
                    or data != ("ROOT:" + nonce).encode("ascii")):
                raise RuntimeError("expected bounded exact root and detached mount handoff")
            identity(inherited[0])
            if identity(inherited[0]) != identity(inherited[1]):
                raise RuntimeError("detached clone differs from retained root")
            # Consume this operation's detached lease. Its successor is minted
            # below only from the actually attached root before namespace exit.
            attach_mount(inherited[1], path)
        root = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW)
        os.fchdir(root)
        owner = mount_owner()
        cwd = os.open(".", os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
        try:
            cwd_matches = identity(cwd) == identity(root)
        finally:
            os.close(cwd)
        if not cwd_matches:
            raise RuntimeError("cwd differs from attached actual root")
        record = {"userns": namespace_identity(owner), "cwd_matches_root": cwd_matches, "kind": "root", "phase": phase, "nonce": nonce,
                  "root": identity(root), "pid_in_namespace": os.getpid(),
                  "namespace_euid": os.geteuid(), "namespace_egid": os.getegid(),
                  "environment_empty": not bool(os.environ), "filesystem": filesystem(root),
                  "python_startup_environment_keys": startup_keys,
                  "python_startup_lc_ctype": startup_locale}
        if not record["environment_empty"]:
            raise RuntimeError("trusted helper environment was not empty")
        if phase == "first":
            record["inode_setup"] = setup
            write_fixed(root, "first", FIRST)
            detached = clone_mount(root)
            if identity(detached) != identity(root):
                raise RuntimeError("open_tree clone differs from mounted root")
            record["detached_mount"] = identity(detached)
        else:
            record["received_root"] = identity(inherited[0])
            if record["received_root"] != record["root"]:
                raise RuntimeError("received root and bound absolute path differ")
            read_exact(root, "first", FIRST)
            if phase == "second":
                before = os.fstatvfs(root)
                mount(None, path, "tmpfs", 32 | 2 | 4, "size=65536,nr_inodes=64")
                after = os.fstatvfs(root)
                retained = os.fstatvfs(inherited[0])
                if (retained.f_blocks * retained.f_frsize != 65536 or retained.f_files != 64):
                    raise RuntimeError("retained original FD did not observe same quota shrink")
                if (before.f_blocks * before.f_frsize != 131072 or before.f_files != 128
                        or after.f_blocks * after.f_frsize != 65536 or after.f_files != 64):
                    raise RuntimeError("same-backing quota transition did not take effect")
                record["transition"] = {"before_data_bytes": before.f_blocks * before.f_frsize,
                                        "before_inodes": before.f_files,
                                        "after_data_bytes": after.f_blocks * after.f_frsize,
                                        "after_inodes": after.f_files,
                                        "retained_fd_data_bytes": retained.f_blocks * retained.f_frsize,
                                        "retained_fd_inodes": retained.f_files,
                                        "root_after": identity(root)}
                write_fixed(root, "second", SECOND)
                record["quota"] = quota(root)
                record["inode_quota"] = inode_quota(root)
            else:
                read_exact(root, "second", SECOND)
                actual = os.fstatvfs(root)
                if actual.f_blocks * actual.f_frsize != 65536 or actual.f_files != 64:
                    raise RuntimeError("third attachment lost shared quota transition")
                fd = os.open("second", os.O_WRONLY | os.O_CLOEXEC | os.O_NOFOLLOW, dir_fd=root)
                try:
                    if os.write(fd, THIRD) != len(THIRD):
                        raise RuntimeError("short fixed third overwrite")
                finally:
                    os.close(fd)
                read_exact(root, "second", THIRD)
                record["third_existing_file_overwritten"] = True
                record["quota_retained"] = {"data_bytes": actual.f_blocks * actual.f_frsize,
                                           "inode_capacity": actual.f_files}

        if phase == "second":
            detached = clone_mount(root)
            if identity(detached) != identity(root):
                raise RuntimeError("successor detached lease differs from attached root")
            record["successor_detached_mount"] = identity(detached)
        message = json.dumps(record, sort_keys=True).encode("ascii")
        if len(message) > 8192:
            raise RuntimeError("protocol record exceeds fixed bound")
        descriptors = array.array("i", [root, detached, owner] if phase == "first"
                                  else [root, detached] if phase == "second" else [root])
        if control.sendmsg([message], [(socket.SOL_SOCKET, socket.SCM_RIGHTS,
                                       descriptors.tobytes())]) != len(message):
            raise RuntimeError("short protocol send")
        ack = control.recv(256)
        if ack != ("ACK:" + nonce).encode("ascii"):
            raise RuntimeError("handoff acknowledgement mismatch")
    finally:
        if detached is not None:
            os.close(detached)
        if owner is not None:
            os.close(owner)
        if root is not None:
            os.close(root)
        for descriptor in inherited:
            os.close(descriptor)
        control.close()


if __name__ == "__main__":
    main()
