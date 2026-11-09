import unittest

from nonmutation_harness import audit_traces


class TraceAttributionTests(unittest.TestCase):
    def setUp(self):
        self.traces = {
            "calls.100": 'execve("/usr/bin/bwrap", [], []) = 0\nsocket(AF_NETLINK, SOCK_RAW, 0) = 4\nopenat(4, "uid_map", O_RDWR) = 5\nclone() = 101\n',
            "calls.101": 'socket(AF_UNIX, SOCK_DGRAM, 0) = 4\nopenat(4, "gid_map", O_RDWR) = 5\nexecve("/tmp/archy", ["doctor"], []) = 0\nopenat(AT_FDCWD, "/etc/os-release", O_RDONLY) = 3\nclone() = 102\n',
            "calls.102": 'openat(AT_FDCWD, "/proc/sys/kernel/osrelease", O_RDONLY) = 3\n',
        }

    def audit(self):
        return audit_traces(self.traces, "/tmp/archy", "/usr/bin/bwrap", "/tmp/shim")

    def test_preexec_bubblewrap_calls_are_not_attributed_to_doctor(self):
        self.assertEqual(self.audit(), 2)

    def test_postexec_doctor_network_call_is_rejected(self):
        self.traces["calls.101"] += 'socket(AF_INET, SOCK_STREAM, 0) = 4\n'
        with self.assertRaisesRegex(AssertionError, "network"):
            self.audit()

    def test_descendant_write_attempt_is_rejected(self):
        self.traces["calls.102"] += 'openat(AT_FDCWD, "/tmp/x", O_WRONLY|O_CREAT, 0600) = -1 EROFS\n'
        with self.assertRaisesRegex(AssertionError, "writable open"):
            self.audit()

    def test_unapproved_exec_remains_rejected(self):
        self.traces["calls.102"] += 'execve("/tmp/shim", [], []) = 0\n'
        with self.assertRaises(AssertionError):
            self.audit()


if __name__ == "__main__":
    unittest.main()
