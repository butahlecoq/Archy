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
  }
  function close() {
    closingFromHost = true
    doctor.panelOpen = false
    window.visible = false
    closingFromHost = false
  }
  function requestClose() {
    doctor.panelOpen = false
    if (shell && typeof shell.hide === "function") shell.hide("archy.status")
    else window.visible = false
  }

  DoctorProcess { id: doctor }
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
          PanelSectionHeader { text: "PROJECT ACTIONS" }
          Text { width: parent.width; text: "Project, run, report, preview and Blender actions unavailable until H1–H3 typed CLI capabilities exist."; textFormat: Text.PlainText; wrapMode: Text.Wrap; color: Commons.Color.muted; font.family: Style.font.family; font.pixelSize: Style.font.body }
          Repeater {
            model: ["Project status", "Run status", "Open report", "Open preview", "Open Blender"]
            delegate: Button { required property string modelData; text: modelData + " · unavailable"; enabled: false; focusable: false }
          }
          Button { text: "Close"; focusable: true; onClicked: root.requestClose() }
        }
      }
    }
  }
}
