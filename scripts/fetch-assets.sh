#!/usr/bin/env bash
# Fetches the third-party images that Mouse Studio embeds at build time. They are not stored in this
# repository: the Logitech product photographs and the Visual Studio Code icon belong to their owners
# and no redistribution licence is asserted (see assets/README.md).
#
#   scripts/fetch-assets.sh        download whatever is missing and verify its SHA-256
#
# The 11 files below are downloaded from the official sources and must match the checksum the project
# was developed against. The two Signature M650 L photos (assets/m650-l/) were browser captures of
# Logitech's gallery and cannot be re-downloaded exactly. If they are missing, plain placeholder images
# are generated so the app still builds; the M650 L pages then show an empty picture. Copy your own
# 2560x1440 captures over them to get the photos.
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root/assets"

FILES='
mouse-3qtr-front.png|https://resource.logitech.com/content/dam/logitech/en/products/mice/mx-anywhere-3s/product-gallery/graphite/mx-anywhere-3s-mouse-3qtr-front-graphite.png|c7aba1e23c32013840804e9e606e601007ae5534ee59addefae1e4568d713c9f
mouse-3qtr-back.png|https://resource.logitech.com/content/dam/logitech/en/products/mice/mx-anywhere-3s/product-gallery/graphite/mx-anywhere-3s-mouse-3qtr-back-graphite.png|fec56e81ca826dcc2ec0afefa5db1b3287380db3a1a4f6ba9f719ad9cff513ff
mouse-top-view.png|https://resource.logitech.com/content/dam/logitech/en/products/mice/mx-anywhere-3s/product-gallery/graphite/mx-anywhere-3s-mouse-top-view-graphite.png|7312999d945f18245e637ebb8395abace2ec78133f57276c2ca95a5fb5dc3fea
mouse-side-left.png|https://resource.logitech.com/content/dam/logitech/en/products/mice/mx-anywhere-3s/product-gallery/graphite/mx-anywhere-3s-mouse-side-left-graphite.png|31b957cbe2c33b1f348737f85c00971b9985cf8a2a447030edcb337816d91c67
mx-master-4/front-angle.png|https://resource.logitech.com/content/dam/logitech/en/products/mice/mx-master-4/gallery/mx-master-4-graphite-front-3qtr-angle-gallery-10.png|d6d28c4b985068e5b1c0f4e9276dba926f4817cfa5ed2edc8155892e55dc7027
mx-master-4/top.png|https://resource.logitech.com/content/dam/logitech/en/products/mice/mx-master-4/gallery/mx-master-4-graphite-top-angle-gallery-1.png|fadba3d1c0c2af4322ba67696ac2754ba23df450d1287a0d222ad2dccd5684ca
mx-master-4/side.png|https://resource.logitech.com/content/dam/logitech/en/products/mice/mx-master-4/gallery/mx-master-4-graphite-profile-angle-gallery-11.png|71f71fa6d3bf799db6fb5240e96d5d7efa430ab31435f44f2ee934bcd93a00b6
mx-master-4/front.png|https://resource.logitech.com/content/dam/logitech/en/products/mice/mx-master-4/gallery/mx-master-4-graphite-front-angle-gallery-9.png|336b49b230d38dbf6c8b5909450634d48c5f65412827d2cb83bc93f9575399f0
mx-master-4/perspective.png|https://resource.logitech.com/content/dam/logitech/en/products/mice/mx-master-4/gallery/mx-master-4-graphite-3qtr-angle-gallery-8.png|70ee0f185c3d67af9cf892e761ba0a5e7bb1b60891986c7916c3a9405a58f5b8
mx-master-4/bottom.png|https://resource.logitech.com/content/dam/logitech/en/products/mice/mx-master-4/gallery/mx-master-4-graphite-bottom-angle-gallery-12.png|8eb629c49e91ac76b74d58a1c1063cd1bfcb310829ba26eacbce9d07ee02087c
vscode-stable.png|https://code.visualstudio.com/assets/apple-touch-icon.png|6baf5c58f993ac6a564ac851b90e154816f4245d2e36cb5e6da99393350b6161
'

failed=0
while IFS='|' read -r path url sum; do
    [ -n "$path" ] || continue
    if [ -f "$path" ] && [ "$(sha256sum "$path" | cut -d' ' -f1)" = "$sum" ]; then
        printf '  ok        %s\n' "$path"
        continue
    fi
    mkdir -p "$(dirname "$path")"
    tmp="$(mktemp)"
    if curl -fsSL --max-time 120 -A 'Mozilla/5.0' -o "$tmp" "$url" && [ "$(sha256sum "$tmp" | cut -d' ' -f1)" = "$sum" ]; then
        mv "$tmp" "$path"
        printf '  fetched   %s\n' "$path"
    else
        rm -f "$tmp"
        printf '  FAILED    %s (download failed or the file changed upstream; expected sha256 %s)\n' "$path" "${sum:0:12}" >&2
        failed=1
    fi
done <<<"$FILES"

# Placeholders for the two M650 L photos, only when they are missing.
for name in perspective top; do
    f="m650-l/$name.png"
    if [ ! -f "$f" ]; then
        mkdir -p m650-l
        python3 - "$f" <<'PY'
import struct, sys, zlib
w, h = 2560, 1440
row = b"\x00" + bytes((0x13, 0x16, 0x16)) * w          # filter byte + solid #131616 (the app background)
raw = zlib.compress(row * h, 9)
def chunk(tag, data):
    return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xffffffff)
png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", raw) + chunk(b"IEND", b"")
open(sys.argv[1], "wb").write(png)
PY
        printf '  placeholder  %s (replace with a real 2560x1440 capture for the photo)\n' "$f"
    else
        printf '  ok        %s\n' "$f"
    fi
done

[ "$failed" -eq 0 ] || { echo "Some images could not be fetched; the build will fail until they exist." >&2; exit 1; }
echo "Assets ready."
