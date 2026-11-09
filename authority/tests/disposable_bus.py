"""Exercise real unique-name credential queries on a throwaway private bus.

This probes D-Bus behavior only; it does not simulate logind or Polkit.
"""
import os
import subprocess
import unittest

import dbus


class CredentialsFixture(unittest.TestCase):
    def test_unique_sender_credentials_and_disconnect(self):
        daemon = subprocess.Popen(
            ["dbus-daemon", "--session", "--nofork", "--print-address=1"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        try:
            address = daemon.stdout.readline().strip()
            self.assertTrue(address.startswith("unix:"), address)
            first = dbus.bus.BusConnection(address)
            second = dbus.bus.BusConnection(address)
            self.assertNotEqual(first.get_unique_name(), second.get_unique_name())
            bus = dbus.Interface(
                first.get_object("org.freedesktop.DBus", "/org/freedesktop/DBus"),
                "org.freedesktop.DBus",
            )
            own = bus.GetConnectionCredentials(first.get_unique_name())
            other = bus.GetConnectionCredentials(second.get_unique_name())
            self.assertEqual(int(own["UnixUserID"]), os.getuid())
            self.assertEqual(int(other["UnixUserID"]), os.getuid())
            self.assertIn("ProcessID", own)
            self.assertEqual(int(own["ProcessID"]), os.getpid())
            departed = second.get_unique_name()
            second.close()
            with self.assertRaises(dbus.DBusException):
                bus.GetConnectionCredentials(departed)
            print("private bus credential keys:", sorted(own.keys()))
            first.close()
        finally:
            daemon.terminate()
            try:
                daemon.communicate(timeout=3)
            except subprocess.TimeoutExpired:
                daemon.kill()
                daemon.communicate()


if __name__ == "__main__":
    unittest.main()
