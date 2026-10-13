"""Fail before compilation unless the native fixture keeps the host runner IDs."""
import os
from pathlib import Path
import subprocess


def verify_mapping(text, expected):
    if expected <= 0:
        raise ValueError("Native acceptance must not run as host root")
    rows = [line.split() for line in text.splitlines() if line.strip()]
    if rows != [[str(expected), str(expected), "1"]]:
        raise ValueError("Native acceptance must map only the original runner ID")


def verify_identity(parent_namespace, current_namespace, expected, current):
    if not parent_namespace or current_namespace == parent_namespace:
        raise ValueError("Native acceptance requires a private user namespace")
    if expected != current or expected <= 0:
        raise ValueError("Native acceptance must retain the original non-root identity")


def main():
    for kind, actual in [("UID", os.getuid()), ("GID", os.getgid())]:
        expected = int(os.environ[f"KOLBOO_NATIVE_EXPECTED_{kind}"])
        verify_identity(
            os.environ["KOLBOO_NATIVE_PARENT_USER_NAMESPACE"],
            os.readlink("/proc/self/ns/user"), expected, actual,
        )
        verify_mapping(Path(f"/proc/self/{kind.lower()}_map").read_text(), expected)
    # Offline startup must be able to enter a separate network namespace after
    # exec, not just immediately after unshare. No network requests are made.
    subprocess.run(["unshare", "--net", "--", "true"], check=True, timeout=5)
    print("Native user/network namespaces verified with unchanged host runner IDs")


if __name__ == "__main__":
    main()
