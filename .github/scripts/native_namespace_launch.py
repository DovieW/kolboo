"""CI-only privileged parent; execute fixtures as the unchanged host runner.

Direct kernel mappings avoid newuidmap's subordinate-ID policy. No host policy
or root identity is mapped into the child namespace. The parent never executes
the fixture; the child drops to the runner before executing its command.
"""
import ctypes
import os
from pathlib import Path
import signal
import socket
import sys


def runner_ids(environment, effective_uid):
    if effective_uid != 0:
        raise ValueError("The mapping parent must be invoked with sudo")
    uid = int(environment["SUDO_UID"])
    gid = int(environment["SUDO_GID"])
    if uid <= 0 or gid <= 0:
        raise ValueError("Only a non-root sudo caller may run native acceptance")
    if (str(uid) != environment["KOLBOO_NATIVE_EXPECTED_UID"] or
            str(gid) != environment["KOLBOO_NATIVE_EXPECTED_GID"]):
        raise ValueError("The fixture identity must match the original sudo caller")
    return uid, gid


def write_mappings(pid, uid, gid, proc_root=Path("/proc")):
    if pid <= 0 or uid <= 0 or gid <= 0:
        raise ValueError("Invalid child mapping")
    # Called only for our forked, blocked child. Parent retains CAP_SETUID/GID
    # in the parent namespace; no newuidmap, subuid edits or root mapping.
    for kind, identity in [("uid", uid), ("gid", gid)]:
        (proc_root / str(pid) / f"{kind}_map").write_text(
            f"{identity} {identity} 1\n", encoding="ascii")


class CapabilityHeader(ctypes.Structure):
    _fields_ = [("version", ctypes.c_uint32), ("pid", ctypes.c_int)]


class CapabilityData(ctypes.Structure):
    _fields_ = [("effective", ctypes.c_uint32), ("permitted", ctypes.c_uint32),
                ("inheritable", ctypes.c_uint32)]


def checked(result, operation):
    if result != 0:
        raise OSError(ctypes.get_errno(), operation)


def drop_child_privileges(uid, gid, libc):
    # Do this before any exec: the unmapped initial root ID would otherwise lose
    # namespace capabilities before it could drop to the mapped runner identity.
    checked(libc.prctl(8, 1, 0, 0, 0), "Cannot retain namespace capabilities")
    os.setresgid(gid, gid, gid)
    os.setresuid(uid, uid, uid)
    header = CapabilityHeader(0x20080522, 0)  # Linux capability ABI v3
    data = (CapabilityData * 2)()
    data[0] = CapabilityData(1 << 21, 1 << 21, 1 << 21)  # CAP_SYS_ADMIN only
    checked(libc.capset(ctypes.byref(header), data), "Cannot restrict namespace capabilities")
    checked(libc.prctl(47, 2, 21, 0, 0), "Cannot retain offline network capability")
    checked(libc.prctl(38, 1, 0, 0, 0), "Cannot forbid privilege escalation")


def main(command):
    uid, gid = runner_ids(os.environ, os.geteuid())
    if not command:
        raise ValueError("A fixture command is required")
    parent, child = socket.socketpair()
    parent.settimeout(5)
    child.settimeout(5)
    pid = os.fork()
    if pid == 0:
        parent.close()
        try:
            os.setgroups([])
            libc = ctypes.CDLL(None, use_errno=True)
            libc.prctl.argtypes = [ctypes.c_int, *([ctypes.c_ulong] * 4)]
            if libc.unshare(0x10000000) != 0:  # CLONE_NEWUSER
                raise OSError(ctypes.get_errno(), "Cannot create fixture user namespace")
            child.sendall(b"R")
            if child.recv(1) != b"G":
                raise RuntimeError("Fixture mappings were not acknowledged")
            child.close()
            drop_child_privileges(uid, gid, libc)
            os.execvp(command[0], command)
        except Exception as error:
            print(f"Native namespace child failed: {error}", file=sys.stderr, flush=True)
            os._exit(1)
    child.close()
    handlers = {}
    try:
        if parent.recv(1) != b"R":
            raise RuntimeError("Fixture namespace did not become ready")
        write_mappings(pid, uid, gid)
        parent.sendall(b"G")
        for signum in (signal.SIGINT, signal.SIGTERM):
            handlers[signum] = signal.signal(signum, lambda sig, _: os.kill(pid, sig))
        _, status = os.waitpid(pid, 0)
        code = os.waitstatus_to_exitcode(status)
        return code if code >= 0 else 128 - code
    except BaseException:
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        os.waitpid(pid, 0)
        raise
    finally:
        parent.close()
        for signum, handler in handlers.items():
            signal.signal(signum, handler)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
