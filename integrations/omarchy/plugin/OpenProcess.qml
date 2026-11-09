import QtQuick
import Quickshell
import Quickshell.Io
import "OpenModel.js" as Model

Item {
  id: root
  property bool busy: false
  property string message: "Select a verified run artifact to open"
  property string expectedRun: ""
  property string expectedArtifact: ""
  property string expectedKind: ""
  property string expectedSha: ""
  property string output: ""
  property bool invalid: false
  property bool sawRunning: false
  function openArtifact(manifest, status, kind, artifact) {
    if (busy) return
    var args = Model.argv(manifest, status, kind, artifact)
    if (!args) { message = "Selected run or artifact is unavailable"; return }
    expectedRun = status.selected_run.summary.run_id
    expectedArtifact = artifact
    expectedKind = kind
    expectedSha = status.selected_run.artifacts.find(function(a) { return a.id === artifact }).sha256
    output = ""; invalid = false; sawRunning = false; busy = true
    message = "Rechecking accepted artifact…"
    process.command = args
    deadline.restart()
    process.running = true
  }
  function abort(reason) {
    if (!busy) return
    invalid = true; message = reason; deadline.stop()
    if (process.running) { process.signal(15); killDelay.restart() }
    else if (!sawRunning) busy = false
  }
  Timer { id: deadline; interval: 20000; repeat: false; onTriggered: root.abort("Open request timed out") }
  Timer { id: killDelay; interval: 250; repeat: false; onTriggered: if (process.running) process.signal(9) }
  Process {
    id: process
    workingDirectory: "/"
    clearEnvironment: true
    environment: ({ "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8", "PATH": "/usr/bin:/bin", "HOME": Quickshell.env("HOME") || "", "XDG_RUNTIME_DIR": Quickshell.env("XDG_RUNTIME_DIR") || "", "DISPLAY": Quickshell.env("DISPLAY") || "", "WAYLAND_DISPLAY": Quickshell.env("WAYLAND_DISPLAY") || "", "DBUS_SESSION_BUS_ADDRESS": Quickshell.env("DBUS_SESSION_BUS_ADDRESS") || "", "XDG_CURRENT_DESKTOP": Quickshell.env("XDG_CURRENT_DESKTOP") || "" })
    stdout: SplitParser { splitMarker: ""; onRead: function(data) {
      if (root.invalid || !root.busy) return
      if (typeof data !== "string" || data.length > 8192 - root.output.length || /[^\x00-\x7f]/.test(data)) root.abort("Invalid open receipt")
      else root.output += data
    } }
    stderr: SplitParser { splitMarker: ""; onRead: function(data) { if (data.length) root.abort("Open request failed") } }
    onExited: function(code, exitStatus) {
      deadline.stop(); killDelay.stop()
      if (!root.invalid) root.message = Model.complete(root.output, code, exitStatus, root.expectedRun, root.expectedArtifact, root.expectedKind, root.expectedSha)
        ? "Verified artifact handed to viewer" : "Open request refused or failed"
      root.sawRunning = false; root.busy = false
    }
    onRunningChanged: {
      if (process.running) root.sawRunning = true
      else if (root.busy && !root.sawRunning) {
        deadline.stop()
        if (!root.invalid) root.message = "Open process unavailable"
        root.busy = false
      }
    }
  }
}
