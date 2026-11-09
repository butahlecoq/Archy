"""Reconcile stable-ID backlog issues; dry-run unless --apply is supplied.

Uses the authenticated gh account. Never closes issues or changes repository rules.
Issue bodies are generated from backlog.md, with dependency links added after creation.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
REPO = 'butahlecoq/Archy'
BASE = f'https://github.com/{REPO}/blob/feat/release-0.1/'


def gh(*args):
    result = subprocess.run(['gh', *args], check=True, capture_output=True, text=True, encoding='utf-8')
    return result.stdout.strip()


def canonical_body(body):
    return body.replace('\r\n', '\n').strip()


def issue_identity(issue):
    markers = set(re.findall(r'<!-- archy-backlog:([AR]\d{2}) -->', issue['body']))
    title = re.match(r'\[([AR]\d{2})\] ', issue['title'])
    identities = markers | ({title[1]} if title else set())
    if len(identities) > 1:
        raise RuntimeError(f'Conflicting stable IDs on {issue["url"]}: {sorted(identities)}')
    return next(iter(identities), None)


def definitions():
    text = (ROOT / 'docs/plans/backlog.md').read_text(encoding='utf-8')
    matches = list(re.finditer(r'^## (A\d{2})(?: / D\d+)? — (.+)$', text, re.M))
    result = []
    for match in matches:
        end = text.find('\n## ', match.end())
        body = text[match.end():end if end != -1 else None].strip()
        labels = re.search(r'\*\*Labels:\*\* (.+?)\. \*\*Dependencies:', body).group(1).split(', ')
        result.append((match[1], match[2], body, ['release:0.1', *labels]))
    roadmap = [
        ('R01', 'Month 2 richer massing, facades and Pareto comparison',
         'Rectangle/L alternatives cannot express richer courtyard forms or multi-objective exploration.',
         'Explore richer/courtyard footprints, parametric facade modules, explicit design objectives and Pareto-front scenario comparison. Preserve deterministic identities, bounded generation and evidence; do not replace required 0.1 rectangles/L fixtures.',
         '`crates/archy-generate/`, `crates/archy-eval/`, `crates/archy-report/`, new versioned schemas and `tests/roadmap/massing/` after an approved design.',
         'Analytic shape validity/area fixtures, deterministic seeded enumeration, objective provenance, dominance/tie fixtures and original golden-workflow regression.',
         'A03, A04, A14, A20'),
        ('R02', 'Month 3 space graphs and expanded IFC interoperability',
         'Schematic massing IFC does not model rooms, circulation or detailed exchange semantics.',
         'Research room/circulation graphs, preliminary space planning, audited expanded IFC semantics and round-trip/interoperability fixtures. Require explicit semantic mapping and limitations; do not infer egress, permitting or structural validity.',
         'Future bounded modules under `crates/`, `workers/ifc/`, versioned schemas, `docs/ifc-mapping.md` and `tests/roadmap/interoperability/` after an approved design.',
         'Graph invariants, schematic-to-expanded schema migration fixtures, independent-engine IFC geometry/hierarchy comparisons and third-party exchange/round-trip failures retained as evidence.',
         'A12, A13, A19, A20'),
        ('R03', 'Later validated analysis, CAD backends and collaborative review',
         'Massing metrics cannot support validated energy/daylight, cost predictions or professional collaborative review.',
         'Investigate independently validated daylight/energy, cost databases with provenance, justified FreeCAD/Truck/CAD backends, interactive review UI, collaboration and professional approval workflows. Separate experiments and obtain a bounded design per feature. No automatic compliance, RVT production, arbitrary mesh semantics, new kernel/viewport/node editor, cloud service, scheduler or unrestricted execution is authorized by this ticket.',
         'Research-first `docs/research/`, `docs/adr/`, independent validation fixtures under `tests/roadmap/`; implementation file ownership requires a follow-up approved design.',
         'Document reference datasets/engine versions, numerical validation bounds and authority/licensing evidence; prototype reproducibility, access/approval separation and security tests before any product claim.',
         'A16, A19, A20; R01/R02 only where the eventual approved design requires them'),
    ]
    for key, title, problem, scope, files, tests, deps in roadmap:
        body = (f'**Release status:** Future roadmap, explicitly excluded from release 0.1. '
                f'Not a substitute for any required release gate.\n\n**Problem:** {problem}\n\n'
                f'**Intended behavior/scope:** {scope}\n\n**Files:** {files}\n\n'
                f'**Tests:** {tests}\n\n**Dependencies:** {deps}.\n\n'
                '**Acceptance evidence:** Approved bounded design and independent review, exact commands/versions, '
                'source/authority and license records, reproducible fixtures and explicit limitations. '
                'Research does not imply implementation or professional acceptance.')
        result.append((key, title, body, ['roadmap', 'spec', 'documentation', 'testing']))
    assert [entry[0] for entry in result] == [f'A{i:02}' for i in range(1, 24)] + ['R01', 'R02', 'R03']
    return result


def body_for(entry, urls):
    key, _, body, _ = entry
    def link(match):
        stable = match[0]
        return f'[{stable}]({urls[stable]})' if stable in urls else stable
    # Expand dependency ranges before linking so every owning issue is visible.
    body = re.sub(r'A(\d{2})–A(\d{2})', lambda m: ', '.join(f'A{i:02}' for i in range(int(m[1]), int(m[2]) + 1)), body)
    body = re.sub(r'\b[AR]\d{2}\b', link, body)
    return (f'<!-- archy-backlog:{key} -->\n\n{body}\n\n'
            '### Execution and closure contract\n\n'
            '- Record a bounded implementation plan, risks and dependency rationale before edits; write meaningful failing tests/fixtures before or alongside implementation.\n'
            '- Run relevant checks, retain exact commands/results and immutable artifact evidence, obtain independent review and hand off changed files/blockers/next action.\n'
            '- Never close on mocked target evidence. Windows/WSL/CI cannot establish actual Omarchy, GPU, plugin or visual acceptance. Human approval cannot be granted by agent text.\n'
            '- Preserve failing designs, scoped paths, immutable history and constraint source/authority; no regulatory/professional compliance claims.\n\n'
            f'Sources: [release specification]({BASE}docs/specs/release-0.1.txt), '
            f'[backlog and shared contracts]({BASE}docs/plans/backlog.md), '
            f'[coverage audit]({BASE}docs/plans/backlog-audit.md), '
            f'[current decisions]({BASE}docs/plans/current-handoff.md), '
            f'[Omarchy/reuse research]({BASE}docs/research/omarchy-and-reuse.md). '
            'Current founder decisions and reconciled backlog supersede historical authentication/license assumptions in the audit.\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--apply', action='store_true')
    args = parser.parse_args()
    entries = definitions()
    remote = json.loads(gh('issue', 'list', '--repo', REPO, '--state', 'all', '--limit', '1000', '--json', 'number,title,url,body,labels,state'))
    existing = {}
    for issue in remote:
        identity = issue_identity(issue)
        if identity:
            if identity in existing:
                raise RuntimeError(f'Duplicate stable ID {identity}; reconcile manually before writes')
            existing[identity] = issue
    if not args.apply:
        print(json.dumps({'tickets': len(entries), 'create': [e[0] for e in entries if e[0] not in existing], 'reconcile': [e[0] for e in entries if e[0] in existing]}))
        return
    colors = {'release:0.1': '0052CC', 'roadmap': 'BFDADC', 'spec': '5319E7', 'core': '1D76DB', 'adapter': '0E8A16', 'testing': 'FBCA04', 'security': 'D93F0B', 'documentation': '0075CA', 'omarchy': 'C5DEF5'}
    labels = {label['name'] for label in json.loads(gh('label', 'list', '--repo', REPO, '--limit', '100', '--json', 'name'))}
    for label, color in colors.items():
        if label not in labels:
            gh('label', 'create', label, '--repo', REPO, '--color', color, '--description', f'Archy {label} tracking')
    urls = {key: issue['url'] for key, issue in existing.items()}
    with tempfile.TemporaryDirectory(prefix='archy-issues-') as directory:
        path = Path(directory) / 'body.md'
        for entry in entries:
            key, title, _, ticket_labels = entry
            if key in existing:
                continue
            path.write_text(body_for(entry, urls), encoding='utf-8')
            urls[key] = gh('issue', 'create', '--repo', REPO, '--title', f'[{key}] {title}', '--body-file', str(path), '--label', ','.join(ticket_labels))
            print(f'Created {key}: {urls[key]}', flush=True)
        for entry in entries:
            key, title, _, ticket_labels = entry
            body = body_for(entry, urls)
            path.write_text(body, encoding='utf-8')
            current = existing.get(key)
            if current and canonical_body(current['body']) == canonical_body(body) and current['title'] == f'[{key}] {title}' and set(ticket_labels).issubset({x['name'] for x in current['labels']}):
                continue
            gh('issue', 'edit', urls[key], '--repo', REPO, '--title', f'[{key}] {title}', '--body-file', str(path), '--add-label', ','.join(ticket_labels))
    checked = json.loads(gh('issue', 'list', '--repo', REPO, '--state', 'all', '--limit', '1000', '--json', 'title,url,body,labels,state'))
    by_url = {issue['url']: issue for issue in checked}
    lines = ['# GitHub issue map', '', 'Verified 29 October 2026 by `scripts/sync-backlog-issues.py --apply` against remote issue bodies and labels.', '', '23 release tickets and 3 future-roadmap tickets. Publication is tracker evidence only; implementation and target acceptance remain unclaimed. Closed issues are never reopened automatically.', '', '| Stable ID | GitHub issue | Tracker state | Scope |', '|---|---|---|---|']
    for entry in entries:
        key, title, _, ticket_labels = entry
        actual = by_url[urls[key]]
        assert issue_identity(actual) == key, key
        assert actual['title'] == f'[{key}] {title}', key
        assert canonical_body(actual['body']) == canonical_body(body_for(entry, urls)), key
        assert set(ticket_labels).issubset({label['name'] for label in actual['labels']}), key
        lines.append(f'| {key} | [{title}]({urls[key]}) | {actual["state"].lower()} | {"release 0.1" if key.startswith("A") else "future; excluded from 0.1"} |')
    (ROOT / 'docs/plans/issue-map.md').write_text('\n'.join(lines) + '\n', encoding='utf-8')
    print('Verified 26 remote bodies and label sets; wrote docs/plans/issue-map.md')


if __name__ == '__main__':
    main()
