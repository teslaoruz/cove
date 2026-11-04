#!/bin/sh
# Install the Cove binary that sits beside this script, for the current user (no root needed):
# binary to ~/.local/bin, menu entry and icon to ~/.local/share.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
bin="$HOME/.local/bin"
share="${XDG_DATA_HOME:-$HOME/.local/share}"
mkdir -p "$bin"
# copy, then rename over the old file: works while Cove is running
cp "$here/cove" "$bin/.cove.new"
chmod 755 "$bin/.cove.new"
mv -f "$bin/.cove.new" "$bin/cove"
install -Dm644 "$here/cove.svg" "$share/icons/hicolor/scalable/apps/cove.svg"
mkdir -p "$share/applications"
sed "s|^Exec=.*|Exec=$bin/cove|" "$here/cove.desktop" > "$share/applications/.cove.desktop.new"
mv -f "$share/applications/.cove.desktop.new" "$share/applications/cove.desktop"
echo "Cove is installed. Start it from your app menu, or run: cove"
case ":$PATH:" in
  *":$bin:"*) ;;
  *) echo "Note: $bin is not on your PATH. Add it to run 'cove' from a terminal." ;;
esac
