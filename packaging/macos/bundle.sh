#!/usr/bin/env bash
set -euo pipefail
VERSION="$1" BIN="$2" OUT="$3" SUFFIX="$4"
HERE="$(cd "$(dirname "$0")" && pwd)"
STAGE="$(mktemp -d)"
APP="$STAGE/Discord Phoenix.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$OUT"
install -m 755 "$BIN" "$APP/Contents/MacOS/discord-phoenix"
sed "s/@VERSION@/$VERSION/g" "$HERE/Info.plist" > "$APP/Contents/Info.plist"
ICNS="$HERE/../icons/discord-phoenix.icns"
[ -f "$ICNS" ] && cp "$ICNS" "$APP/Contents/Resources/discord-phoenix.icns"
codesign --force --deep --sign - "$APP" || true
NAME="discord-phoenix-$VERSION-$SUFFIX"
OUT="$(cd "$OUT" && pwd)"
(cd "$STAGE" && ditto -c -k --keepParent "Discord Phoenix.app" "$OUT/$NAME.app.zip")
hdiutil create -volname "Discord Phoenix" -srcfolder "$APP" -ov -format UDZO "$OUT/$NAME.dmg"
rm -rf "$STAGE"
