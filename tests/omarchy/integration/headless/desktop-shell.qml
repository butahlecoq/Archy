import QtQuick
import Quickshell
import "." as ArchyPlugin

Scope {
  id: root
  property string scenario: Quickshell.env("ARCHY_DESKTOP_CASE")
  property var busyEvents: []
  property int statusCount: 0
  property bool inFlightInvalidSelected: false
  property bool inFlightSettled: false
  property bool selectedWhileBusy: false
  property bool recovered: false
  property bool finished: false
  function finish() {
    if (finished) return
    finished = true
    console.log("ARCHY_DESKTOP_RESULT " + JSON.stringify({
      capabilities: desktop.capabilities, status: desktop.status,
      error: desktop.error, stale: desktop.stale, busy: desktop.busy,
      busyEvents: busyEvents, selectedWhileBusy: selectedWhileBusy,
      inFlightSettled: inFlightSettled,
      manifest: desktop.manifest,
      selectedRunId: desktop.selectedRunId, pageCursor: desktop.pageCursor
    }))
    Qt.quit()
  }
  ArchyPlugin.DesktopProcess {
    id: desktop
    panelOpen: true
    onBusyChanged: {
      root.busyEvents = root.busyEvents.concat([busy])
      if (root.scenario === "switch-manifest" && busy && root.busyEvents.length === 3) switchTimer.restart()
      if (root.scenario === "invalid-selection-in-flight" && busy && root.busyEvents.length === 5) invalidSelectionTimer.restart()
      if (root.scenario === "invalid-selection-in-flight" && !busy && root.inFlightInvalidSelected) inFlightSettleTimer.restart()
    }
    onCapabilitiesChanged: if (capabilities) Qt.callLater(function() { desktop.selectManifest("/tmp/My Project/project.yaml") })
    onErrorChanged: {
      if (root.scenario === "stale-cursor" && error.indexOf("Page cursor stale") === 0 && !root.recovered) {
        root.recovered = true
        Qt.callLater(function() { desktop.firstPage() })
      }
    }
    onStatusChanged: {
      if (!status) return
      root.statusCount++
      if (root.scenario === "invalid-selection" && root.statusCount === 1) {
        Qt.callLater(function() { desktop.selectManifest("relative/project.yaml") })
      } else if (root.scenario === "invalid-selection-in-flight" && root.statusCount === 1) {
        Qt.callLater(function() { desktop.refreshStatus() })
      } else if ((root.scenario === "selected-run" || root.scenario === "wrong-explicit") && root.statusCount === 1) {
        Qt.callLater(function() { desktop.selectRun(status.runs[1].run_id) })
      } else if (root.scenario === "stale-cursor" && root.statusCount === 1) {
        Qt.callLater(function() { desktop.nextPage() })
      }
    }
  }
  Component.onCompleted: desktop.refreshCapabilities()
  Timer {
    interval: 25; repeat: true; running: true
    onTriggered: {
      if (desktop.busy || root.busyEvents.length === 0) return
      if ((root.scenario === "selected-run" || root.scenario === "wrong-explicit") && root.busyEvents.length < 6) return
      if (root.scenario === "stale-cursor" && root.busyEvents.length < 8) return
      if (root.scenario === "switch-manifest" && root.busyEvents.length < 6) return
      if (root.scenario === "normal" && root.busyEvents.length < 4) return
      if (root.scenario === "invalid-selection" && (root.busyEvents.length < 4 || root.statusCount < 1)) return
      if (root.scenario === "invalid-selection-in-flight") return
      if (root.scenario === "malformed" || root.scenario === "flood" || root.scenario === "stderr") {
        if (root.busyEvents.length < 4) return
      }
      root.finish()
    }
  }
  Timer { id: switchTimer; interval: 100; repeat: false; onTriggered: desktop.selectManifest("/tmp/Second Project/project.yaml") }
  Timer {
    id: invalidSelectionTimer; interval: 100; repeat: false
    onTriggered: {
      root.selectedWhileBusy = desktop.busy
      root.inFlightInvalidSelected = true
      desktop.selectManifest("relative/project.yaml")
    }
  }
  Timer {
    id: inFlightSettleTimer; interval: 300; repeat: false
    onTriggered: {
      if (!root.inFlightInvalidSelected || desktop.busy) return
      root.inFlightSettled = true
      root.finish()
    }
  }
  Timer { interval: 5000; running: true; onTriggered: root.finish() }
}
