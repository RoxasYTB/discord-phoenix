#!/usr/bin/env bash
set -euo pipefail
VERSION="$1" BIN="$2" OUT="$3"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$HERE/../.."
ICON="$HERE/../icons/discord-phoenix.png"
ARCH=x86_64 DEBARCH=amd64
mkdir -p "$OUT"; OUT="$(cd "$OUT" && pwd)"
WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
NAME="discord-phoenix-$VERSION-linux-$ARCH"

FHS="$WORK/fhs/usr"
install -Dm755 "$BIN" "$FHS/bin/discord-phoenix"
install -Dm644 "$HERE/discord-phoenix.desktop" "$FHS/share/applications/discord-phoenix.desktop"
if [ -f "$ICON" ]; then
  install -Dm644 "$ICON" "$FHS/share/icons/hicolor/512x512/apps/io.github.RoxasYTB.DiscordPhoenix.png"
  for f in "$HERE"/../icons/hicolor/*/apps/io.github.RoxasYTB.DiscordPhoenix.png; do
    install -Dm644 "$f" "$FHS/share/icons/hicolor/${f#"$HERE"/../icons/hicolor/}"
  done
else
  echo "warning: $ICON missing; packages will have no icon" >&2
fi
for f in LICENSE-MIT LICENSE-APACHE README.md; do
  install -Dm644 "$ROOT/$f" "$FHS/share/doc/discord-phoenix/$f"
done

mkdir -p "$WORK/$NAME"
cp -r "$FHS/bin" "$FHS/share" "$WORK/$NAME/"
tar -C "$WORK" -czf "$OUT/$NAME.tar.gz" "$NAME"

DEB="$WORK/deb"
mkdir -p "$DEB/DEBIAN"; cp -r "$WORK/fhs/usr" "$DEB/"
DEBVER="${VERSION/-/\~}"
cat > "$DEB/DEBIAN/control" <<CTRL
Package: discord-phoenix
Version: $DEBVER
Architecture: $DEBARCH
Maintainer: Discord Phoenix contributors <noreply@github.com>
Homepage: https://github.com/RoxasYTB/project-phoenix
Section: net
Priority: optional
Depends: libc6, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libvulkan1, libxcb1, libfontconfig1, libasound2 | libasound2t64, libssl3 | libssl3t64
Description: Unofficial native Discord client
 GPU-accelerated desktop client written in Rust with GPUI.
 Not affiliated with or endorsed by Discord Inc.
CTRL
dpkg-deb --root-owner-group --build "$DEB" "$OUT/discord-phoenix_${DEBVER}_${DEBARCH}.deb"

if [ "${SKIP_APPIMAGE:-0}" != 1 ]; then
  APPDIR="$WORK/AppDir"
  cp -r "$WORK/fhs" "$APPDIR"
  cp "$HERE/discord-phoenix.desktop" "$APPDIR/"
  if [ -f "$ICON" ]; then cp "$ICON" "$APPDIR/io.github.RoxasYTB.DiscordPhoenix.png"; else
    printf '\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0\x1f\x15\xc4\x89\0\0\0\rIDATx\x9cc\xf8\x0f\0\0\x01\x01\0\x05\x18\xd8N\0\0\0\0IEND\xaeB`\x82' > "$APPDIR/io.github.RoxasYTB.DiscordPhoenix.png"
  fi
  cat > "$APPDIR/AppRun" <<'RUN'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/discord-phoenix" "$@"
RUN
  chmod +x "$APPDIR/AppRun"
  TOOL="$WORK/appimagetool"
  curl -fsSL -o "$TOOL" \
    "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
  chmod +x "$TOOL"
  ARCH=$ARCH VERSION="$VERSION" "$TOOL" --appimage-extract-and-run "$APPDIR" "$OUT/discord-phoenix-$VERSION-$ARCH.AppImage"
fi
ls -l "$OUT"
