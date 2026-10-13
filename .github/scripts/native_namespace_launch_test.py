from pathlib import Path
import tempfile
import unittest
from native_namespace_launch import fixture_command, runner_ids, write_mappings


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
        self.assertEqual(fixture_command(1001, 1002, ["/usr/bin/python3", "fixture.py"]),
                         ["/usr/bin/setpriv", "--reuid=1001", "--regid=1002",
                          "--clear-groups", "--inh-caps=+sys_admin",
                          "--ambient-caps=+sys_admin", "--no-new-privs", "--",
                          "/usr/bin/python3", "fixture.py"])
        for uid, gid, command in [(0, 1001, ["true"]), (1001, 0, ["true"]), (1001, 1001, [])]:
            with self.subTest(uid=uid), self.assertRaises(ValueError):
                fixture_command(uid, gid, command)


if __name__ == "__main__":
    unittest.main()
