# Recovery test CI race

PR run 40933696136 failed because a `/proc/<pid>/stat` read returned
`ESRCH` after the writer exited. Opening that file while a child is alive,
then killing and reaping the child before reading, reproduces `ESRCH` on this
Linux host. The test now treats that error and `ENOENT` as stopped, while
other read errors remain failures. Running and zombie states remain distinct.

A separate local parallel stress run failed at iteration 14 because the parent
observed an empty readiness marker after file creation but before its write.
Both readiness markers are now written to temporary paths and renamed within
the fixture directory only after the contents have been written.

The focused recovery suite passed all nine tests and 100 repeated parallel
runs. Independent review passed the suite and 20 further parallel runs. The
reviewed test source SHA-256 is
`71f8dca53aa89a54f34094b802c6b914411294656c161ec22e7ce2ddef3fbd31`.
On an exceptional failure to open `/proc` immediately after spawning the
regression test's `/bin/sleep` child, that child can remain until its
30-second timeout; no such leftover was observed in the review runs.

The integration workspace checks are recorded separately in the local logs
under `.tools/ci-a17-race-fix-logs`. A new GitHub CI run is required to verify
the fix on that environment.
