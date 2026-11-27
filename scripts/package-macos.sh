#!/usr/bin/env bash
set -euo pipefail

# Packages AGY Swap as a native macOS .app bundle (Menu Bar agent) + .dmg installer.
# Usage:
#   ./scripts/package-macos.sh            # build release + .app + .dmg in ./dist
#   ./scripts/package-macos.sh --install  # also install into /Applications and ~/.local/bin

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

INSTALL_LOCAL=0
for arg in "$@"; do
  case "$arg" in
    --install) INSTALL_LOCAL=1 ;;
    *)
      echo "Unknown argument: $arg" >&2
      exit 2
      ;;
  esac
done

VERSION="$(awk -F'"' '/^version *=/{print $2; exit}' Cargo.toml)"
ARCH="$(uname -m)"
APP_NAME="AGY Swap"
BUNDLE_ID="dev.kjaiml.agyswap"
DIST_DIR="$ROOT/dist"
APP_DIR="$DIST_DIR/${APP_NAME}.app"
DMG_NAME="AGY-Swap-v${VERSION}-macos-${ARCH}.dmg"
DMG_PATH="$DIST_DIR/$DMG_NAME"
CLI_ZIP="$DIST_DIR/agswitch-cli-v${VERSION}-macos-${ARCH}.tar.gz"

echo "==> Building release binaries (v${VERSION}, ${ARCH})..."
cargo build --release -p agswitch-desktop -p agswitch-cli

rm -rf "$APP_DIR" "$DMG_PATH" "$DIST_DIR/dmg-stage" "$DIST_DIR/AppIcon.iconset" "$DIST_DIR/AppIcon.icns"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"

cp "$ROOT/target/release/agswitch-desktop" "$APP_DIR/Contents/MacOS/agswitch-desktop"
cp "$ROOT/target/release/agswitch-cli" "$APP_DIR/Contents/MacOS/agswitch"
chmod +x "$APP_DIR/Contents/MacOS/agswitch-desktop" "$APP_DIR/Contents/MacOS/agswitch"

echo "==> Generating PHOSPHOR AppIcon.icns..."
ICON_PNG="$DIST_DIR/icon-1024.png"
python3 - "$ICON_PNG" <<'PY'
import math, struct, sys, zlib

out_path = sys.argv[1]
W = H = 1024
buf = bytearray(W * H * 4)

# Matte obsidian squircle background with emerald PHOSPHOR 7x7 LED matrix
def set_px(x, y, r, g, b, a):
    idx = (y * W + x) * 4
    buf[idx] = r
    buf[idx + 1] = g
    buf[idx + 2] = b
    buf[idx + 3] = a

# Precompute 7x7 emerald pulse matrix centers
grid = 7
margin = 196.0
span = W - 2.0 * margin
step = span / (grid - 1)
cells = []
for row in range(grid):
    for col in range(grid):
        if (row in (0, 6)) and (col in (0, 6)):
            continue
        cx = margin + col * step
        cy = margin + row * step
        dist = math.hypot(col - 3.0, row - 3.0)
        # Bright emerald center + glowing inner ring + subtle outer sockets
        if dist < 0.5:
            intensity = 1.0
        elif dist < 1.5:
            intensity = 0.72
        elif dist < 2.5:
            intensity = 0.28
        else:
            intensity = 0.11
        cells.append((cx, cy, intensity))

pad = 64.0
rad = 224.0
for y in range(H):
    dy = max(0.0, abs(y - (H - 1) * 0.5) - ((H * 0.5 - pad) - rad))
    for x in range(W):
        dx = max(0.0, abs(x - (W - 1) * 0.5) - ((W * 0.5 - pad) - rad))
        d_edge = math.hypot(dx, dy) - rad
        if d_edge > 1.5:
            continue
        alpha = max(0.0, min(1.0, 1.0 - (d_edge + 0.5) / 2.0))
        # Subtle obsidian vertical gradient + hairline rim
        grad = y / float(H)
        r = 12.0 + 4.0 * (1.0 - grad)
        g = 14.0 + 5.0 * (1.0 - grad)
        b = 15.0 + 6.0 * (1.0 - grad)
        if -5.0 < d_edge <= 0.0:
            r += 24.0
            g += 28.0
            b += 30.0
        set_px(x, y, int(min(255, r)), int(min(255, g)), int(min(255, b)), int(alpha * 255))

# Draw LED dots + soft bloom
dot_r = 31.0
glow_r = 86.0
for cx, cy, inten in cells:
    x0 = max(0, int(cx - glow_r))
    x1 = min(W - 1, int(cx + glow_r))
    y0 = max(0, int(cy - glow_r))
    y1 = min(H - 1, int(cy + glow_r))
    for y in range(y0, y1 + 1):
        dy = y - cy
        for x in range(x0, x1 + 1):
            dx = x - cx
            d = math.hypot(dx, dy)
            if d > glow_r:
                continue
            idx = (y * W + x) * 4
            if buf[idx + 3] == 0:
                continue
            r, g, b = float(buf[idx]), float(buf[idx + 1]), float(buf[idx + 2])
            if d <= dot_r:
                edge = max(0.0, min(1.0, (dot_r - d) / 2.5))
                if inten > 0.9:
                    lr, lg, lb = 228.0, 255.0, 240.0
                elif inten > 0.5:
                    lr, lg, lb = 34.0, 229.0, 132.0
                elif inten > 0.2:
                    lr, lg, lb = 18.0, 118.0, 72.0
                else:
                    lr, lg, lb = 19.0, 56.0, 40.0
                r = r * (1.0 - edge) + lr * edge
                g = g * (1.0 - edge) + lg * edge
                b = b * (1.0 - edge) + lb * edge
            if inten > 0.4:
                falloff = math.exp(-((d / (glow_r * 0.45)) ** 2)) * inten * 0.55
                r += 24.0 * falloff
                g += 235.0 * falloff
                b += 136.0 * falloff
            buf[idx] = int(min(255, r))
            buf[idx + 1] = int(min(255, g))
            buf[idx + 2] = int(min(255, b))

def chunk(tag, data):
    return struct.pack("!I", len(data)) + tag + data + struct.pack("!I", zlib.crc32(tag + data) & 0xFFFFFFFF)

raw = bytearray()
stride = W * 4
for y in range(H):
    raw.append(0)
    raw.extend(buf[y * stride : (y + 1) * stride])

ihdr = struct.pack("!IIBBBBB", W, H, 8, 6, 0, 0, 0)
png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(bytes(raw), 6)) + chunk(b"IEND", b"")
with open(out_path, "wb") as f:
    f.write(png)
PY

ICONSET="$DIST_DIR/AppIcon.iconset"
mkdir -p "$ICONSET"
for sz in 16 32 128 256 512; do
  sips -z "$sz" "$sz" "$ICON_PNG" --out "$ICONSET/icon_${sz}x${sz}.png" >/dev/null
  sz2=$((sz * 2))
  sips -z "$sz2" "$sz2" "$ICON_PNG" --out "$ICONSET/icon_${sz}x${sz}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP_DIR/Contents/Resources/AppIcon.icns"
rm -rf "$ICONSET" "$ICON_PNG"

cat > "$APP_DIR/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>${APP_NAME}</string>
  <key>CFBundleExecutable</key>
  <string>agswitch-desktop</string>
  <key>CFBundleIconFile</key>
  <string>AppIcon</string>
  <key>CFBundleIdentifier</key>
  <string>${BUNDLE_ID}</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>${APP_NAME}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${VERSION}</string>
  <key>CFBundleVersion</key>
  <string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>LSUIElement</key>
  <true/>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
PLIST

echo "==> Ad-hoc signing ${APP_NAME}.app..."
codesign --force --deep --sign - "$APP_DIR"

echo "==> Creating DMG (${DMG_NAME})..."
STAGE="$DIST_DIR/dmg-stage"
mkdir -p "$STAGE"
cp -R "$APP_DIR" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create \
  -volname "$APP_NAME" \
  -srcfolder "$STAGE" \
  -ov \
  -format UDZO \
  "$DMG_PATH" >/dev/null
rm -rf "$STAGE"

tar -czf "$CLI_ZIP" -C "$ROOT/target/release" agswitch-cli
(cd "$DIST_DIR" && shasum -a 256 "$DMG_NAME" "$(basename "$CLI_ZIP")" > "SHA256SUMS-macos-${ARCH}.txt")

echo "==> Packaged artifacts:"
ls -lh "$DMG_PATH" "$CLI_ZIP" "$DIST_DIR/SHA256SUMS-macos-${ARCH}.txt"

if [ "$INSTALL_LOCAL" -eq 1 ]; then
  echo "==> Installing '${APP_NAME}.app' into /Applications..."
  # Stop any previous running instance cleanly before replacing bundle
  # (macOS `pkill -x` truncates names at 15 chars; match the bundle binary path via `-f`)
  pkill -f "AGY Swap.app/Contents/MacOS/agswitch-desktop" 2>/dev/null || true
  sleep 0.3
  rm -rf "/Applications/${APP_NAME}.app"
  cp -R "$APP_DIR" "/Applications/${APP_NAME}.app"
  mkdir -p "$HOME/.local/bin"
  cp "$ROOT/target/release/agswitch-cli" "$HOME/.local/bin/agswitch"
  chmod +x "$HOME/.local/bin/agswitch"
  echo "Installed:"
  echo "  App: /Applications/${APP_NAME}.app"
  echo "  CLI: $HOME/.local/bin/agswitch"
fi
