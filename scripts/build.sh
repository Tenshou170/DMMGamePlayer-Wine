#!/usr/bin/env bash
#
# build.sh
# Downloads and repacks the specified or latest version of DMM Game Player.
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

VERSION="${1:-}"

if [ -z "$VERSION" ]; then
    echo "==> Fetching latest version from DMM update feed..."
    MANIFEST="$(curl -sSL https://dlapp-dmmgameplayer.games.dmm.com/latest.yml)"
    VERSION="$(echo "$MANIFEST" | grep "^version:" | awk '{print $2}' | tr -d '\r\n')"
    echo "==> Detected latest version: $VERSION"
fi

EXE_NAME="DMMGamePlayer-Setup-${VERSION}.exe"
DOWNLOAD_URL="https://dlapp-dmmgameplayer.games.dmm.com/${EXE_NAME}"
DIST_DIR="$ROOT_DIR/dist/v${VERSION}"
BUILD_DIR="$ROOT_DIR/build"

mkdir -p "$BUILD_DIR" "$DIST_DIR"

if [ ! -f "$BUILD_DIR/$EXE_NAME" ]; then
    echo "==> Downloading $DOWNLOAD_URL..."
    curl -sSL -o "$BUILD_DIR/$EXE_NAME" "$DOWNLOAD_URL"
fi

echo "==> Extracting installer contents..."
rm -rf "$BUILD_DIR/extracted" "$BUILD_DIR/DMMGamePlayer"
mkdir -p "$BUILD_DIR/extracted" "$BUILD_DIR/DMMGamePlayer"

7z x -y "$BUILD_DIR/$EXE_NAME" -o"$BUILD_DIR/extracted" >/dev/null
7z x -y "$BUILD_DIR/extracted/\$PLUGINSDIR/app-64.7z" -o"$BUILD_DIR/DMMGamePlayer" >/dev/null

echo "==> Verifying upstream payload & registry against known dataset..."
python3 "$SCRIPT_DIR/verify_dataset.py" \
    "$BUILD_DIR/$EXE_NAME" \
    "$BUILD_DIR/DMMGamePlayer" \
    "$VERSION" \
    "$ROOT_DIR/data/known_dataset.json" \
    "$DIST_DIR"

if [ -f "$SCRIPT_DIR/extract_registry.py" ]; then
    echo "==> Analyzing and generating registry configuration..."
    python3 "$SCRIPT_DIR/extract_registry.py" \
        "$BUILD_DIR/$EXE_NAME" \
        "$BUILD_DIR/DMMGamePlayer" \
        "$VERSION" \
        "$BUILD_DIR/DMMGamePlayer/dmmgameplayer.reg" \
        "$DIST_DIR/discovery-report.json" || true
fi

echo "==> Building Wine-friendly Rust setup wizard (x86_64-pc-windows-msvc)..."
cd "$ROOT_DIR"
python3 -c "
import re
p = '$ROOT_DIR/Cargo.toml'
with open(p, 'r') as f:
    c = f.read()
new_c = re.sub(r'(?m)^version = \".*\"', f'version = \"$VERSION\"', c, count=1)
if c != new_c:
    with open(p, 'w') as f:
        f.write(new_c)
"
cargo xwin build --release --target x86_64-pc-windows-msvc
INSTALLER_BIN="$ROOT_DIR/target/x86_64-pc-windows-msvc/release/DMMGamePlayer-Setup-Wine.exe"
PACKAGE_EXE="DMMGamePlayer-Setup-Wine-${VERSION}.exe"

# 1. Create the standalone self-contained installer by concatenating the binary + 7z payload archive
echo "==> Embedding payload into standalone package installer ($PACKAGE_EXE)..."
cat "$INSTALLER_BIN" "$BUILD_DIR/extracted/\$PLUGINSDIR/app-64.7z" > "$DIST_DIR/$PACKAGE_EXE"
chmod +x "$DIST_DIR/$PACKAGE_EXE"

# 2. Copy the standalone helper executables into the unpacked portable directory
cp "$INSTALLER_BIN" "$BUILD_DIR/DMMGamePlayer/DMMGamePlayer-Setup-Wine.exe"
cp "$INSTALLER_BIN" "$BUILD_DIR/DMMGamePlayer/Uninstall DMMGamePlayer.exe"

echo "==> Packaging distribution archives..."
cd "$BUILD_DIR"
tar -czf "$DIST_DIR/DMMGamePlayer-v${VERSION}-portable.tar.gz" DMMGamePlayer/
7z a -y "$DIST_DIR/DMMGamePlayer-v${VERSION}-portable.zip" DMMGamePlayer/ >/dev/null

echo "==> Build finished!"
echo "    Artifacts created in: $DIST_DIR"
ls -lh "$DIST_DIR"

