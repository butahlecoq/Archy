import QtQuick
import Quickshell
import Quickshell.Io

Scope {
  id: root
  property var events: []
  property string stdoutText: ""
  property int stderrChars: 0

  Process {
    id: doctor
    command: ["/usr/bin/archy", "doctor", "--json"]
    workingDirectory: "/"
    clearEnvironment: true
    environment: ({ "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8" })
    stdout: SplitParser {
      splitMarker: ""
      onRead: function(data) {
        root.events = root.events.concat(["stdout"])
        root.stdoutText += data
      }
    }
    stderr: SplitParser {
      splitMarker: ""
      onRead: function(data) {
        root.events = root.events.concat(["stderr"])
        root.stderrChars += data.length
      }
    }
    onExited: function(exitCode, exitStatus) {
      root.events = root.events.concat(["exited"])
      console.log("ARCHY_HARNESS_RESULT " + JSON.stringify({
        events: root.events, stdout: root.stdoutText,
        stderrChars: root.stderrChars, exitCode: exitCode, exitStatus: exitStatus
      }))
      Qt.quit()
    }
  }
  Component.onCompleted: doctor.running = true
  Timer { interval: Quickshell.env("ARCHY_HARNESS_CASE") === "real-doctor" ? 21000 : 4000; running: true; onTriggered: Qt.quit() }
}
