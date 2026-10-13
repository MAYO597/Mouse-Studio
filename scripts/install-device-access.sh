#!/usr/bin/env bash
set -euo pipefail
if [[ "$EUID" -ne 0 ]]; then
    printf '%s\n' 'Run this script with sudo to install access for supported mice and the Bolt configuration interface.' >&2
    exit 1
fi
mouse_studio_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
mouse_studio_rules="$(mktemp)"
trap 'rm -f -- "$mouse_studio_rules"' EXIT
cat -- "$mouse_studio_root/packaging/70-mouse-studio.rules" > "$mouse_studio_rules"
# Grant only a model-specific Bluetooth product ID discovered on this machine.
# The packaged Bolt rule includes only its configuration interface.
python3 - <<'PY' >> "$mouse_studio_rules"
from pathlib import Path
import re
names = {'logitech signature m650 l', 'logitech m650 l', 'logi m650 l', 'signature m650 l', 'm650 l',
         'logitech m650', 'logi m650', 'm650', 'logitech signature m650', 'signature m650',
         'logitech mx master 4', 'logi mx master 4', 'mx master 4'}
products = set()
existing=Path('/etc/udev/rules.d/70-mouse-studio.rules')
if existing.exists():
    # Preserve already-installed, product-specific grants when that mouse is asleep.
    for line in existing.read_text().splitlines():
        match=re.fullmatch(r'SUBSYSTEM=="hidraw", KERNELS=="0005:046D:([0-9A-F]{4})\.\*", TAG\+="uaccess"',line)
        if match: products.add(match.group(1))
for path in Path('/sys/class/hidraw').glob('*/device/uevent'):
    values = dict(line.split('=', 1) for line in path.read_text().splitlines() if '=' in line)
    match = re.fullmatch(r'0005:0000046D:0000([0-9A-Fa-f]{4})', values.get('HID_ID', ''))
    if match and values.get('HID_NAME', '').lower() in names:
        products.add(match.group(1).upper())
for product in sorted(products - {"B037"}):
    print(f'SUBSYSTEM=="hidraw", KERNELS=="0005:046D:{product}.*", TAG+="uaccess"')
PY
install -m 0644 -- "$mouse_studio_rules" /etc/udev/rules.d/70-mouse-studio.rules
udevadm control --reload-rules
udevadm trigger --subsystem-match=hidraw --action=change
udevadm settle
printf '%s\n' 'Mouse hardware access installed. Connect any additional supported mouse over Bluetooth and rerun this script to include it. The app retries automatically.'
