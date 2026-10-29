import QtQuick
import Quickshell

ShellRoot {
  OpenProcess {
    id: opener
    property bool started: false
    onBusyChanged: {
      if (busy) started = true
      else if (started) {
        console.log("ARCHY_OPEN_RESULT " + JSON.stringify({message: message, artifact: expectedArtifact, kind: expectedKind}))
        Qt.quit()
      }
    }
    Component.onCompleted: openArtifact("/tmp/My Project/project.yaml", JSON.parse(Quickshell.env("ARCHY_STATUS_JSON")), "report", "report.md")
  }
  Timer { interval: 5000; running: true; repeat: false; onTriggered: { console.log("ARCHY_OPEN_TIMEOUT"); Qt.quit() } }
}
