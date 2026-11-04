#!/bin/sh
# Install Project Hub for the current user: binary to ~/.local/bin, menu entry and icon to ~/.local/share.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
bin="$HOME/.local/bin"
share="${XDG_DATA_HOME:-$HOME/.local/share}"
install -Dm755 "$here/project-hub" "$bin/project-hub"
install -Dm644 "$here/project-hub.svg" "$share/icons/hicolor/scalable/apps/project-hub.svg"
sed "s|^Exec=.*|Exec=$bin/project-hub|" "$here/project-hub.desktop" > "$here/.desktop.tmp"
install -Dm644 "$here/.desktop.tmp" "$share/applications/project-hub.desktop"
rm -f "$here/.desktop.tmp"
echo "Installed. Start it from your app menu or run: $bin/project-hub"
