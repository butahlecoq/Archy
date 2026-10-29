import QtQuick
import Quickshell
import Quickshell.Io
import "DesktopModel.js" as Model

Item {
  id: root
  property bool panelOpen: false
  property bool busy: false
  property var capabilities: null
  property var status: null
  property string manifest: ""
  property string selectedRunId: ""
  property string pageCursor: ""
  property string error: "Select an absolute local project manifest"
  property bool stale: false
  property int generation: 0
  property var probe: null
  property bool pending: false
  property string pendingKind: ""

  function hasStatusCapability() {
    if (!capabilities) return false
    return capabilities.operations.every(function(o) { return o.available })
  }
  function start(kind) {
    if (!panelOpen) return
    if (busy) { pending = true; pendingKind = kind; return }
    var requestedRunId = kind === "status" ? selectedRunId || null : null
    var argv = kind === "capabilities" ? Model.capabilitiesArgv()
      : Model.statusArgv(manifest, requestedRunId, pageCursor || null)
    if (!argv) { error = "Invalid selected manifest, run, or cursor"; return }
    generation++
    probe = Model.newProbe(generation, kind, requestedRunId)
    busy = true
    stale = status !== null
    error = kind === "status" ? "Reading verified project status…" : "Checking desktop capabilities…"
    process.command = argv
    deadline.restart()
    process.running = true
  }
  function refreshCapabilities() { start("capabilities") }
  function refreshStatus() {
    if (!Model.validManifest(manifest)) { error = "Select an absolute local project manifest"; return }
    if (!capabilities) { refreshCapabilities(); return }
    if (!hasStatusCapability()) { error = "Desktop status capability unavailable"; return }
    start("status")
  }
  function selectManifest(path) {
    if (!Model.validManifest(path)) { error = "Enter an absolute local manifest path"; return }
    if (busy) abort("Project selection changed")
    manifest = path
    status = null
    selectedRunId = ""
    pageCursor = ""
    stale = false
    refreshStatus()
  }
  function selectRun(id) {
    if (!status || !status.runs.some(function(r) { return r.run_id === id }) || !Model.validRunId(id)) return
    selectedRunId = id
    refreshStatus()
  }
  function nextPage() {
    if (!status || !Model.validCursor(status.next_cursor)) return
    pageCursor = status.next_cursor
    selectedRunId = ""
    refreshStatus()
  }
  function firstPage() { pageCursor = ""; selectedRunId = ""; refreshStatus() }
  function abort(reason) {
    if (!busy) return
    generation++
    if (probe) Model.cancel(probe)
    pending = false
    error = reason
    stale = status !== null
    deadline.stop()
    if (process.running) { process.signal(15); killDelay.restart() }
    else reap()
  }
  function reap() {
    var next = pendingKind
    var queued = pending
    pending = false; pendingKind = ""; probe = null; busy = false
    if (queued && panelOpen) start(next)
  }
  onPanelOpenChanged: if (!panelOpen) abort("Panel closed; project status stale")
  Component.onDestruction: abort("Panel destroyed")

  Timer { id: deadline; interval: 20000; repeat: false; onTriggered: root.abort("Desktop status timed out") }
  Timer { id: killDelay; interval: 250; repeat: false; onTriggered: if (process.running) process.signal(9) }
  Process {
    id: process
    workingDirectory: "/"
    clearEnvironment: true
    environment: ({ "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8" })
    stdout: SplitParser {
      splitMarker: ""
      onRead: function(data) {
        var p = root.probe
        if (p && !p.cancelled && !Model.append(p,"stdout",data)) root.abort("Desktop output exceeded limit or invalid encoding")
      }
    }
    stderr: SplitParser {
      splitMarker: ""
      onRead: function(data) {
        var p = root.probe
        if (p && !p.cancelled && !Model.append(p,"stderr",data)) root.abort("Desktop stderr output rejected")
      }
    }
    onExited: function(exitCode, exitStatus) {
      deadline.stop(); killDelay.stop()
      var p = root.probe
      var kind = p ? p.kind : ""
      var result = Model.complete(p,exitCode,exitStatus,root.generation)
      if (root.panelOpen && result.capabilities) {
        root.capabilities = result.capabilities
        root.error = ""
        if (Model.validManifest(root.manifest)) {
          root.pending = false; root.pendingKind = ""
          root.reap(); root.refreshStatus(); return
        }
      } else if (root.panelOpen && result.status) {
        root.status = result.status
        root.stale = false
        root.error = ""
      } else if (p && !p.cancelled) {
        root.stale = root.status !== null
        root.error = kind === "status" && root.pageCursor
          ? "Page cursor stale or unavailable; refresh first page" : result.reason
      }
      root.reap()
    }
    onRunningChanged: {
      if (!process.running && root.busy) {
        deadline.stop(); killDelay.stop()
        var p = root.probe
        if (p && !p.cancelled) { root.error = "Desktop process unavailable"; root.stale = root.status !== null }
        root.reap()
      }
    }
  }
}
