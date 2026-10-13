"""CI-only privileged parent; execute fixtures as the unchanged host runner.

Direct kernel mappings avoid newuidmap's subordinate-ID policy. No host policy
or root identity is mapped into the child namespace. The parent never executes
the fixture; setpriv drops the child to the runner before executing its command.
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


def fixture_command(uid, gid, command):
    if not command or uid <= 0 or gid <= 0:
        raise ValueError("A non-root fixture command is required")
    # Only namespace-local CAP_SYS_ADMIN survives exec, for offline networking.
    return ["/usr/bin/setpriv", f"--reuid={uid}", f"--regid={gid}",
            "--clear-groups", "--inh-caps=+sys_admin", "--ambient-caps=+sys_admin",
            "--no-new-privs", "--", *command]


def main(command):
    uid, gid = runner_ids(os.environ, os.geteuid())
    argv = fixture_command(uid, gid, command)
    parent, child = socket.socketpair()
    parent.settimeout(5)
    child.settimeout(5)
    pid = os.fork()
    if pid == 0:
        parent.close()
        try:
            os.setgroups([])
            libc = ctypes.CDLL(None, use_errno=True)
            if libc.unshare(0x10000000) != 0:  # CLONE_NEWUSER
                raise OSError(ctypes.get_errno(), "Cannot create fixture user namespace")
            child.sendall(b"R")
            if child.recv(1) != b"G":
                raise RuntimeError("Fixture mappings were not acknowledged")
            child.close()
            os.execv(argv[0], argv)
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
