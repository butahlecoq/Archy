# Human-bus recovery candidate (not installed)

The service keeps the 65,536-byte receive allocation bound. A 70 KiB call
closes its zbus connection before Review dispatch; the private-bus fixture
repeats this four times. The broker retries at 1, 2 and 4 seconds, then exits.
A connection that lasted at least 60 seconds starts a new four-incident
budget. Short bursts remain bounded, including failed connection builds.

`archy-human-busd.service` is a proposed external supervisor: at most three
starts in ten minutes, separated by 30 seconds after failure. After the
systemd start limit is hit, an administrator must investigate and restart it;
there is no unbounded privileged retry. A sustained enrolled sender can still
force a fail-closed outage. The policy restricts calls to the `archy-review`
group and name ownership to `archy-broker`, but it cannot rate-limit an
already enrolled sender. A production package needs a tested sender-side
rate limit or stricter ingress design before claiming sustained availability.

Deployment gates:

1. Package the broker binary, unit, policy, runtime notices and fixed service
   accounts together; do not enable the unit or Confirm from this source tree.
2. On a disposable native target, verify effective system-bus policy ordering:
   only `archy-broker` owns the name, only the enrolled human UID (in
   `archy-review`) can call it, agent and other UIDs cannot, and no broad
   rule overrides these denials. Verify local logind transitions and pidfd.
3. Verify `systemctl show` reports the intended StartLimitIntervalUSec,
   StartLimitBurst, RestartUSec, User, Group, BusName and effective sandbox;
   run four 70 KiB calls against the installed unit and observe its exit,
   bounded restart, name reacquisition, and a valid Review afterward. Repeat
   through the start limit and confirm it stays stopped until operator action.
4. Load-test enrolled sender abuse and bus restart. If sustained availability
   is required, design and verify an ingress limit before deployment. Keep
   approval disabled until the separate human-intent and Polkit gates pass.
