#!/bin/sh
# Container-only fixed /usr/bin/archy fixture; tests inspect each exact argv.
printf 'BEGIN\n' >> /tmp/archy-desktop-argv
printf '%s\n' "$@" >> /tmp/archy-desktop-argv
case "$1 $2 $3" in
  'desktop capabilities --json')
    if [ "$(cat /run/archy-desktop/mode)" = cap-malformed ]; then printf '{bad'; exit 0; fi
    cat /run/archy-desktop/capabilities.json; exit 0 ;;
esac
if [ "$1" != desktop ] || [ "$2" != status ]; then exit 2; fi
project=''
run=''
previous=''
for arg in "$@"; do
  if [ "$previous" = --project ]; then project=$arg; fi
  if [ "$previous" = --run ]; then run=$arg; fi
  previous=$arg
done
if [ ! -f "$project" ]; then exit 2; fi
case "$(cat /run/archy-desktop/mode)" in
  wrong-explicit)
    cat /run/archy-desktop/status.json; exit 0 ;;
  malformed) printf '{broken'; exit 0 ;;
  flood) head -c 1100000 /dev/zero | tr '\000' x; exit 0 ;;
  stderr) printf 'warning\n' >&2; cat /run/archy-desktop/status.json; exit 0 ;;
  stale-cursor)
    for arg in "$@"; do
      if [ "$arg" = --cursor ]; then printf '{"code":"REVISION_CONFLICT"}\n'; exit 2; fi
    done
    cat /run/archy-desktop/status.json; exit 0 ;;
  switch-manifest)
    if [ "$project" = '/tmp/My Project/project.yaml' ]; then
      trap 'sleep 0.3; exit 0' TERM
      while :; do :; done
    fi
    cat /run/archy-desktop/status.json; exit 0 ;;
  *)
    if [ -n "$run" ]; then
      /usr/bin/python3 - "$run" <<'PY'
import json,sys
r=json.load(open('/run/archy-desktop/status.json'))
selected=next((x for x in r['runs'] if x['run_id']==sys.argv[1]),None)
if selected is None: sys.exit(2)
r['selected_run']['summary']=selected
print(json.dumps(r,ensure_ascii=True))
PY
      exit $?
    fi
    cat /run/archy-desktop/status.json; exit 0 ;;
esac
