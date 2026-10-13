import unittest
from native_namespace_check import verify_identity, verify_mapping, verify_privileges


class NativeNamespaceContract(unittest.TestCase):
    def test_only_same_single_host_id_is_allowed(self):
        verify_mapping("      1001       1001          1\n", 1001)
        for text, expected in [
            ("1001 0 1", 1001), ("0 1001 1", 1001), ("1001 1001 2", 1001),
            ("1001 1001 1\n0 0 1", 1001), ("", 1001), ("0 0 1", 0),
        ]:
            with self.subTest(text=text, expected=expected), self.assertRaises(ValueError):
                verify_mapping(text, expected)

    def test_same_namespace_or_root_or_changed_identity_is_rejected(self):
        verify_identity("user:[1]", "user:[2]", 1001, 1001)
        for parent, current, expected, actual in [
            ("user:[1]", "user:[1]", 1001, 1001), ("", "user:[2]", 1001, 1001),
            ("user:[1]", "user:[2]", 1001, 0), ("user:[1]", "user:[2]", 0, 0),
        ]:
            with self.subTest(actual=actual), self.assertRaises(ValueError):
                verify_identity(parent, current, expected, actual)

    def test_post_exec_capabilities_are_minimal_and_privilege_escalation_is_blocked(self):
        fields = {name: "00200000" for name in ["CapEff", "CapPrm", "CapInh", "CapAmb"]}
        fields["NoNewPrivs"] = "1"
        status = lambda data: "\n".join(f"{key}:\t{value}" for key, value in data.items())
        verify_privileges(status(fields))
        for name in ["CapEff", "CapPrm", "CapInh", "CapAmb", "NoNewPrivs"]:
            with self.subTest(name=name), self.assertRaises(ValueError):
                verify_privileges(status({**fields, name: "0"}))
        with self.assertRaises(ValueError):
            verify_privileges(status({**fields, "CapEff": "00200001"}))


if __name__ == "__main__":
    unittest.main()
