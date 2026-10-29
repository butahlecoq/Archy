import QtQuick
import QtQuick.Controls
import Quickshell
import qs.Ui
import qs.Commons
import qs.Commons as Commons

Item {
  id: root
  property var shell: null
  property bool closingFromHost: false
  function open(payloadJson) {
    if (payloadJson && String(payloadJson).trim() !== "" && String(payloadJson).trim() !== "{}") return
    closingFromHost = false
    window.visible = true
    doctor.panelOpen = true
    doctor.refresh(true)
    desktop.panelOpen = true
    desktop.refreshCapabilities()
  }
  function refresh(payloadJson) {
    if (payloadJson && String(payloadJson).trim() !== "") return
    if (!window.visible) return
    doctor.refresh()
    if (desktop.manifest !== "") desktop.refreshStatus()
  }
  function close() {
    closingFromHost = true
    opener.abort("Panel closed")
    doctor.panelOpen = false
    desktop.panelOpen = false
    window.visible = false
    closingFromHost = false
  }
  function requestClose() {
    opener.abort("Panel closed")
    doctor.panelOpen = false
    desktop.panelOpen = false
    if (shell && typeof shell.hide === "function") shell.hide("archy.status")
    else window.visible = false
  }

  DoctorProcess { id: doctor }
  DesktopProcess { id: desktop }
  OpenProcess { id: opener }
  FloatingWindow {
    id: window
    title: "Archy status"
    color: Commons.Color.background
    implicitWidth: 560
    implicitHeight: 680
    minimumSize: Qt.size(420, 440)
    onVisibleChanged: if (!visible && !root.closingFromHost) root.requestClose()
    FocusScope {
      anchors.fill: parent
      focus: true
      Keys.onEscapePressed: root.requestClose()
      ScrollView {
        id: scrollArea
        anchors.fill: parent
        anchors.margins: Style.space(16)
        clip: true
        Column {
          width: scrollArea.availableWidth
          spacing: Style.space(10)
          PanelSectionHeader { text: "ARCHY DOCTOR" }
          Text { text: doctor.error || (doctor.stale ? "Stale doctor report" : "Doctor report available"); color: doctor.error ? Commons.Color.urgent : Commons.Color.foreground; textFormat: Text.PlainText; wrapMode: Text.Wrap; width: parent.width; font.family: Style.font.family; font.pixelSize: Style.font.body }
          Text { text: "Last refresh: " + doctor.refreshedAt; color: Commons.Color.muted; textFormat: Text.PlainText; font.family: Style.font.family; font.pixelSize: Style.font.body }
          Button { text: doctor.busy ? "Refreshing…" : "Refresh doctor"; focusable: true; enabled: !doctor.busy; onClicked: doctor.refresh() }
          PanelSeparator {}
          PanelSectionHeader { text: "HOST AND BASELINE" }
          Text { width: parent.width; text: doctor.report ? ((doctor.report.host.os_id || "Unknown OS") + " · " + doctor.report.host.environment + " · baseline " + doctor.report.platform.supported_baseline + (doctor.report.platform.development_override ? " · development override" : "")) : "Unavailable"; textFormat: Text.PlainText; wrapMode: Text.Wrap; color: Commons.Color.foreground; font.family: Style.font.family; font.pixelSize: Style.font.body }
          PanelSectionHeader { text: "CAPABILITIES (DIAGNOSTIC ONLY)" }
          Repeater {
            model: doctor.report ? ["core", "blender", "ifc", "containment", "desktop"] : []
            delegate: Text { required property string modelData; text: modelData + ": " + (doctor.report.capabilities[modelData].ready ? "prerequisites observed" : "prerequisites unmet"); textFormat: Text.PlainText; color: Commons.Color.foreground; font.family: Style.font.family; font.pixelSize: Style.font.body }
          }
          PanelSectionHeader { text: "CHECKS" }
          Repeater {
            model: doctor.report ? doctor.report.checks : []
            delegate: Text { required property var modelData; width: scrollArea.availableWidth; text: modelData.id + " · " + modelData.status + "\n" + modelData.reason + (modelData.remedy ? "\nRemedy: " + modelData.remedy : ""); textFormat: Text.PlainText; wrapMode: Text.Wrap; color: modelData.status === "available" ? Commons.Color.foreground : Commons.Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.body }
          }
          PanelSectionHeader { text: "TARGET GATES · DEFERRED" }
          Repeater {
            model: doctor.report ? doctor.report.target_gates : []
            delegate: Text { required property var modelData; width: scrollArea.availableWidth; text: modelData.id + ": " + modelData.reason; textFormat: Text.PlainText; wrapMode: Text.Wrap; color: Commons.Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.body }
          }
          PanelSeparator {}
          PanelSectionHeader { text: "SELECTED PROJECT · READ ONLY" }
          Text { width: parent.width; text: "Enter the absolute path to a local project.yaml, project.yml or project.json manifest. Nothing is scanned or saved."; textFormat: Text.PlainText; wrapMode: Text.Wrap; color: Commons.Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.body }
          TextField { id: manifestInput; width: parent.width; placeholderText: "/absolute/path/to/project.yaml"; selectByMouse: true }
          Button { text: "Load selected manifest"; focusable: true; enabled: !desktop.busy; onClicked: { opener.abort("Project selection changed"); desktop.selectManifest(manifestInput.text) } }
          Text { width: parent.width; text: desktop.error || (desktop.stale ? "Project status stale" : "Verified project status available"); textFormat: Text.PlainText; wrapMode: Text.Wrap; color: desktop.error ? Commons.Color.urgent : Commons.Color.foreground; font.family: Style.font.family; font.pixelSize: Style.font.body }
          Text { width: parent.width; text: desktop.capabilities ? (desktop.hasStatusCapability() ? "Project and run status available" : "Desktop status capability unavailable") : "Checking desktop capabilities"; textFormat: Text.PlainText; wrapMode: Text.Wrap; color: Commons.Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.body }
          Button { text: desktop.busy ? "Reading…" : "Refresh status"; focusable: true; enabled: !desktop.busy && desktop.manifest !== ""; onClicked: desktop.refreshStatus() }
          Button { text: "Refresh first page"; focusable: true; enabled: !desktop.busy && desktop.manifest !== ""; onClicked: desktop.firstPage() }
          Text { width: parent.width; text: desktop.status ? ("Project " + desktop.status.project.id + " · revision " + desktop.status.project.revision + " · source " + desktop.status.project.source_sha256) : "No verified project snapshot"; textFormat: Text.PlainText; wrapMode: Text.Wrap; color: Commons.Color.foreground; font.family: Style.font.family; font.pixelSize: Style.font.body }
          PanelSectionHeader { text: "NEWEST VERIFIED RUNS" }
          Repeater {
            model: desktop.status ? desktop.status.runs : []
            delegate: Button { required property var modelData; width: scrollArea.availableWidth; text: modelData.run_id + " · " + modelData.operation + " · revision " + modelData.revision + " · required checks " + (modelData.all_required_checks_passed ? "passed" : "unmet"); focusable: true; enabled: !desktop.busy; onClicked: { opener.abort("Run selection changed"); desktop.selectRun(modelData.run_id) } }
          }
          Text { width: parent.width; text: desktop.status && desktop.status.runs.length === 0 ? "No accepted runs" : ""; textFormat: Text.PlainText; color: Commons.Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.body }
          Button { text: "Next 20 verified runs"; focusable: true; enabled: !desktop.busy && desktop.status && desktop.status.next_cursor !== null; onClicked: desktop.nextPage() }
          PanelSectionHeader { text: "SELECTED RUN · VERIFIED METADATA" }
          Text { width: parent.width; text: desktop.status && desktop.status.selected_run ? (desktop.status.selected_run.summary.run_id + " · " + desktop.status.selected_run.summary.operation + " · variants " + desktop.status.selected_run.summary.variant_count + " · required checks " + (desktop.status.selected_run.summary.all_required_checks_passed ? "passed" : "unmet")) : "No run selected"; textFormat: Text.PlainText; wrapMode: Text.Wrap; color: Commons.Color.foreground; font.family: Style.font.family; font.pixelSize: Style.font.body }
          Repeater {
            model: desktop.status && desktop.status.selected_run ? desktop.status.selected_run.artifacts : []
            delegate: Text { required property var modelData; width: scrollArea.availableWidth; text: modelData.id + " · " + modelData.kind + " · " + modelData.byte_len + " bytes · " + modelData.integrity + "\nSHA-256 " + modelData.sha256; textFormat: Text.PlainText; wrapMode: Text.Wrap; color: Commons.Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.body }
          }
          Text { width: parent.width; text: desktop.status ? ("Backend jobs: " + desktop.status.backend_jobs.status + " · " + desktop.status.backend_jobs.reason) : "Backend jobs: unknown"; textFormat: Text.PlainText; wrapMode: Text.Wrap; color: Commons.Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.body }
          PanelSeparator {}
          PanelSectionHeader { text: "OPEN SELECTED VERIFIED ARTIFACT" }
          Text { width: parent.width; text: opener.message; textFormat: Text.PlainText; wrapMode: Text.Wrap; color: Commons.Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.body }
          Repeater {
            model: desktop.status && !desktop.stale && desktop.status.selected_run ? desktop.status.selected_run.artifacts.filter(function(a) {
              return a.kind === "report" || a.id.indexOf("previews/") === 0 || a.id.indexOf("scenes/") === 0
            }) : []
            delegate: Button {
              required property var modelData
              property string openKind: modelData.kind === "report" ? "report" : (modelData.id.indexOf("previews/") === 0 ? "preview" : "blender")
              text: "Open " + modelData.id + (desktop.hasOpenCapability(openKind) ? "" : " · viewer unavailable")
              enabled: !desktop.busy && !opener.busy && desktop.hasOpenCapability(openKind)
              focusable: enabled
              onClicked: opener.openArtifact(desktop.manifest, desktop.status, openKind, modelData.id)
            }
          }
          Text { width: parent.width; text: desktop.status && desktop.status.selected_run && desktop.status.selected_run.artifacts.every(function(a) { return a.kind !== "report" && a.id.indexOf("previews/") !== 0 && a.id.indexOf("scenes/") !== 0 }) ? "No openable verified artifacts in this run" : ""; textFormat: Text.PlainText; color: Commons.Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.body }
          Button { text: "Close"; focusable: true; onClicked: root.requestClose() }
        }
      }
    }
  }
}
