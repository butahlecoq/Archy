import QtQuick
import Quickshell
import Quickshell.Io
import "DoctorModel.js" as Model

Item {
  id: root
  property bool panelOpen: false
  property var session: Model.newSession()
  property bool busy: false
  property var report: null
  property string error: "Doctor has not been refreshed"
  property bool stale: false
  property string refreshedAt: "Never"
  function refresh(queueIfBusy) {
    if (!panelOpen) return
    var probe = Model.begin(session, queueIfBusy === true)
    if (!probe) {
      if (queueIfBusy) error = "Waiting for previous doctor process to exit…"
      return
    }
    busy = true
    stale = report !== null
    error = "Refreshing doctor…"
    deadline.restart()
    process.running = true
  }
  function abort(reason) {
    if (!session.busy) return
    Model.teardown(session)
    stale = report !== null
    error = reason
    deadline.stop()
    if (process.running) {
      process.signal(15)
      killDelay.restart()
    } else {
      Model.reaped(session)
      busy = false
    }
  }
  onPanelOpenChanged: if (!panelOpen) abort("Panel closed; doctor result stale")
  Component.onDestruction: abort("Panel destroyed")

  Timer { id: deadline; interval: 20000; repeat: false; onTriggered: root.abort("Doctor timed out") }
  Timer { id: killDelay; interval: 250; repeat: false; onTriggered: if (process.running) process.signal(9) }

  Process {
    id: process
    command: ["/usr/bin/archy", "doctor", "--json"]
    workingDirectory: "/"
    clearEnvironment: true
    environment: {
      var env = { "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8" }
      var overridePath = Quickshell.env("OMARCHY_PATH")
      if (overridePath) env.OMARCHY_PATH = overridePath
      return env
    }
    stdout: SplitParser {
      splitMarker: ""
      onRead: function(data) {
        var p = root.session.probe
        if (p && !p.cancelled && !Model.append(p, "stdout", data)) root.abort("Doctor output exceeded limit or invalid encoding")
      }
    }
    stderr: SplitParser {
      splitMarker: ""
      onRead: function(data) {
        var p = root.session.probe
        if (p && !p.cancelled && !Model.append(p, "stderr", data)) root.abort("Doctor stderr exceeded limit or invalid encoding")
      }
    }
    onExited: function(exitCode, exitStatus) {
      deadline.stop()
      killDelay.stop()
      var p = root.session.probe
      var generation = root.session.generation
      // Trusted fixed /usr/bin/archy doctor producer: one bounded ASCII report,
      // no stderr, flushed before normal exit 0/1, no surviving pipe writer.
      // On reviewed QS 0.3.2 / Qt 6.12, both channel reads and parser endings
      // precede exited. This is profile-bound, not a generic child-EOF rule.
      var trustedNormalCompletion = exitStatus === 0 && (exitCode === 0 || exitCode === 1)
        && root.panelOpen && root.session.busy && p && p.generation === generation
        && !p.cancelled && !p.failed && p.stdoutBytes > 0
        && p.stdoutBytes <= Model.MAX_BYTES && p.stderrBytes === 0
      var result = Model.complete(p, exitCode, trustedNormalCompletion, generation)
      var queued = Model.reaped(root.session)
      root.busy = false
      if (result.report && root.panelOpen) {
        root.report = result.report
        root.refreshedAt = new Date().toLocaleString()
        root.stale = false
        root.error = ""
      } else if (p && !p.cancelled) {
        root.stale = root.report !== null
        root.error = p.stderrBytes !== 0 ? "doctor stderr not empty" : result.reason
      }
      if (queued && root.panelOpen) root.refresh(false)
    }
    onRunningChanged: {
      // v0.2.1 reports FailedToStart here without an exited signal.
      if (!process.running && root.session.busy) {
        deadline.stop()
        killDelay.stop()
        var p = root.session.probe
        var queued = Model.reaped(root.session)
        root.busy = false
        if (p && !p.cancelled) {
          root.stale = root.report !== null
          root.error = "Doctor process unavailable"
        }
        if (queued && root.panelOpen) root.refresh(false)
      }
    }
  }
}
