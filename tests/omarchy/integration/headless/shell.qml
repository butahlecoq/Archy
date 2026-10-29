import QtQuick
import Quickshell
import "." as ArchyPlugin

Scope {
  id: root
  property string scenario: Quickshell.env("ARCHY_HARNESS_CASE")
  property int starts: 0
  property var busyEvents: []
  property bool sawBusy: false
  property bool reopened: false
  property bool finished: false

  function finish() {
    if (finished) return
    finished = true
    console.log("ARCHY_HARNESS_RESULT " + JSON.stringify({
      scenario: scenario, starts: starts, busyEvents: busyEvents, report: doctor.report,
      error: doctor.error, busy: doctor.busy, stale: doctor.stale,
      refreshedAt: doctor.refreshedAt
    }))
    Qt.quit()
  }

  ArchyPlugin.DoctorProcess {
    id: doctor
    panelOpen: true
    onBusyChanged: {
      root.busyEvents = root.busyEvents.concat([busy])
      if (busy) { root.starts++; root.sawBusy = true }
    }
  }
  Component.onCompleted: doctor.refresh()
  Timer {
    interval: 25
    repeat: true
    running: true
    onTriggered: {
      if (!root.sawBusy || doctor.busy) return
      if (root.scenario === "close-reopen" && root.starts < 2) return
      root.finish()
    }
  }
  Timer {
    interval: 100
    running: root.scenario === "close-reopen"
    repeat: false
    onTriggered: {
      doctor.panelOpen = false
      doctor.panelOpen = true
      doctor.refresh(true)
      root.reopened = true
    }
  }
  Timer {
    interval: root.scenario === "hang" ? 21000 : 4000
    running: true
    repeat: false
    onTriggered: root.finish()
  }
}
