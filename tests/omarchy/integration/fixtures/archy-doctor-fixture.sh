#!/bin/sh
# Container-only /usr/bin/archy fixture. Production argv and environment stay fixed.
if [ "$#" -ne 2 ] || [ "$1" != doctor ] || [ "$2" != --json ]; then
  exit 2
fi
case "$(cat /run/archy-fixture/mode)" in
  valid0) cat /run/archy-fixture/report.json; exit 0 ;;
  valid1) cat /run/archy-fixture/report.json; exit 1 ;;
  exit2) cat /run/archy-fixture/report.json; exit 2 ;;
  crash) kill -KILL $$ ;;
  stderr-small) printf 'warning\n' >&2; cat /run/archy-fixture/report.json; exit 0 ;;
  nonascii) sed 's/fixture only/café/' /run/archy-fixture/report.json; exit 0 ;;
  malformed) printf '{bad json\n'; exit 0 ;;
  truncated) printf '{"schema_version":"1"'; exit 0 ;;
  extra-document) cat /run/archy-fixture/report.json; printf '{}\n'; exit 0 ;;
  stderr-flood) head -c 70000 /dev/zero | tr '\000' x >&2; cat /run/archy-fixture/report.json; exit 0 ;;
  stdout-flood) head -c 70000 /dev/zero | tr '\000' x; exit 0 ;;
  hang) exec sleep 30 ;;
  split-unicode)
    # Valid schema report, with a UTF-8 codepoint split between writes.
    sed 's/fixture only/café/' /run/archy-fixture/report.json > /tmp/archy-unicode.json
    /usr/bin/python3 -c 'import sys,time; b=open("/tmp/archy-unicode.json","rb").read(); i=b.index(b"\xc3"); sys.stdout.buffer.write(b[:i+1]); sys.stdout.buffer.flush(); time.sleep(.05); sys.stdout.buffer.write(b[i+1:]); sys.stdout.buffer.flush()'
    exit 0 ;;
  close-reopen)
    if [ ! -e /tmp/archy-fixture-first ]; then
      : > /tmp/archy-fixture-first
      trap 'sleep 0.3; exit 0' TERM
      printf 'old request started\n' >&2
      while :; do :; done
    fi
    cat /run/archy-fixture/report.json
    exit 0 ;;
  *) exit 2 ;;
esac
