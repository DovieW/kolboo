from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from native_namespace_launch import drop_child_privileges, runner_ids, write_mappings


class PrivilegedMappingContract(unittest.TestCase):
    def environment(self):
        return {"SUDO_UID": "1001", "SUDO_GID": "1001",
                "KOLBOO_NATIVE_EXPECTED_UID": "1001",
                "KOLBOO_NATIVE_EXPECTED_GID": "1001"}

    def test_requires_root_parent_and_exact_nonroot_sudo_caller(self):
        self.assertEqual(runner_ids(self.environment(), 0), (1001, 1001))
        with self.assertRaises(ValueError):
            runner_ids(self.environment(), 1001)
        for key, value in [("SUDO_UID", "0"), ("SUDO_GID", "0"),
                           ("KOLBOO_NATIVE_EXPECTED_UID", "1002"),
                           ("KOLBOO_NATIVE_EXPECTED_GID", "1002")]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                runner_ids({**self.environment(), key: value}, 0)

    def test_maps_only_the_blocked_child_to_unchanged_host_ids(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            child = root / "123"
            child.mkdir()
            write_mappings(123, 1001, 1002, root)
            self.assertEqual((child / "uid_map").read_text(), "1001 1001 1\n")
            self.assertEqual((child / "gid_map").read_text(), "1002 1002 1\n")
            self.assertEqual(sorted(p.name for p in child.iterdir()), ["gid_map", "uid_map"])
            for pid, uid, gid in [(0, 1001, 1001), (123, 0, 1001), (123, 1001, 0)]:
                with self.subTest(pid=pid), self.assertRaises(ValueError):
                    write_mappings(pid, uid, gid, root)

    def test_command_drops_identity_and_retains_only_namespace_network_capability(self):
        operations = []

        class KernelBoundary:
            def prctl(self, *args):
                operations.append(args)
                return 0

            def capset(self, header, data):
                operations.append((header._obj.version, header._obj.pid))
                operations.append(tuple((item.effective, item.permitted, item.inheritable)
                                        for item in data))
                return 0

        with patch("native_namespace_launch.os.setresgid", side_effect=lambda *args: operations.append(("gid", *args))), \
                patch("native_namespace_launch.os.setresuid", side_effect=lambda *args: operations.append(("uid", *args))):
            drop_child_privileges(1001, 1002, KernelBoundary())
        self.assertEqual(operations, [(8, 1, 0, 0, 0),
                                     ("gid", 1002, 1002, 1002),
                                     ("uid", 1001, 1001, 1001),
                                     (0x20080522, 0),
                                     ((1 << 21, 1 << 21, 1 << 21), (0, 0, 0)),
                                     (47, 2, 21, 0, 0), (38, 1, 0, 0, 0)])

    def test_failed_kernel_call_never_continues_to_exec(self):
        class RejectedKernel:
            def prctl(self, *_):
                return -1

        with patch("native_namespace_launch.os.setresuid") as setuid, self.assertRaises(OSError):
            drop_child_privileges(1001, 1001, RejectedKernel())
        setuid.assert_not_called()


if __name__ == "__main__":
    unittest.main()
