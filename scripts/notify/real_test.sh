#!/bin/sh
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
OUT=${1:-/mnt/windata/project-phoenix/compare/notifs}
PROBE=${PROBE:-$HOME/discord-phoenix/target/debug/examples/notify_probe}
SECS=${SECS:-180}
mkdir -p "$OUT"
rm -f "$OUT"/notifications.jsonl* "$OUT"/monitor.log
REAL_BUS=${DBUS_SESSION_BUS_ADDRESS:-unix:path=/run/user/$(id -u)/bus}
export HERE OUT PROBE SECS REAL_BUS
exec dbus-run-session -- sh -c '
  set -eu
  case "$DBUS_SESSION_BUS_ADDRESS" in *"/run/user/"*) echo "not a private bus" >&2; exit 1;; esac
  dbus-monitor --session "interface=org.freedesktop.Notifications" > "$OUT/monitor.log" 2>&1 &
  MON=$!
  FAKE_NOTIFYD_CLICK=1 python3 "$HERE/fake_notifyd.py" "$OUT/notifications.jsonl" &
  FAKE=$!
  i=0
  until gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus --method org.freedesktop.DBus.NameHasOwner org.freedesktop.Notifications 2>/dev/null | grep -q true || [ $i -ge 50 ]; do i=$((i+1)); sleep 0.1; done
  RC=0
  PHOENIX_NOTIFY_PRIVATE_BUS="$DBUS_SESSION_BUS_ADDRESS" DBUS_SESSION_BUS_ADDRESS="$REAL_BUS" "$PROBE" "$SECS" || RC=$?
  kill $FAKE $MON 2>/dev/null || true
  exit $RC
'
