.import "DesktopModel.js" as DesktopModel
var MAX_BYTES = 8192;
function validArtifact(kind, artifact) {
  if (typeof artifact !== 'string' || artifact.length > 128) return false;
  if (kind === 'report') return artifact === 'report.md' || artifact === 'report.json';
  if (kind === 'preview') return (/^previews\/[A-Za-z0-9_/-]+\.png$/.test(artifact) && artifact.indexOf('..') < 0)
    || /^renders\/variant-[0-9a-f]{64}\/view-(top|front|iso)\.png$/.test(artifact);
  if (kind === 'blender') return (/^scenes\/[A-Za-z0-9_/-]+\.blend$/.test(artifact) && artifact.indexOf('..') < 0)
    || /^renders\/variant-[0-9a-f]{64}\/scene\.blend$/.test(artifact);
  return false;
}
function argv(manifest, status, kind, artifact) {
  if (!DesktopModel.validManifest(manifest) || !status || !DesktopModel.validStatus(status, status.selected_run ? status.selected_run.summary.run_id : null)
      || !status.selected_run || !validArtifact(kind, artifact)
      || !status.selected_run.artifacts.some(function(a) { return a.id === artifact && a.kind === kind && a.integrity === 'verified'; })) return null;
  return ['/usr/bin/archy','desktop','open','--project',manifest,'--run',status.selected_run.summary.run_id,
    '--artifact',artifact,'--kind',kind,'--json'];
}
function complete(stdout, code, exitStatus, expectedRun, expectedArtifact, kind, expectedSha) {
  if (code !== 0 || exitStatus !== 0 || typeof stdout !== 'string' || stdout.length === 0 || stdout.length > MAX_BYTES || /[^\x00-\x7f]/.test(stdout)) return false;
  try {
    var r = JSON.parse(stdout);
    return r && Object.keys(r).length === 6 && r.schema_version === 'archy-desktop-open-v1'
      && r.status === 'handed_to_viewer' && r.run_id === expectedRun && r.artifact_id === expectedArtifact
      && r.kind === kind && r.sha256 === expectedSha && /^[0-9a-f]{64}$/.test(r.sha256);
  } catch (e) { return false; }
}
