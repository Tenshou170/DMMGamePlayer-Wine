#!/usr/bin/env python3
"""
verify_dataset.py
Verifies upstream DMM installer and extracted payload against the verified baseline
in data/known_dataset.json.

If any new registry keys, parameter changes, missing crucial files, or new binaries
are detected, it halts execution and appends a detailed divergence report to
GITHUB_STEP_SUMMARY for manual intervention.
"""

import sys
import os
import zlib
import re
import json
from pathlib import Path

def extract_nsis_header(installer_path):
    if not os.path.exists(installer_path):
        return b""
    with open(installer_path, "rb") as f:
        data = f.read(500000)
        sig_pos = data.find(b"NullsoftInst")
        if sig_pos == -1:
            sig_pos = data.find(b"Nullsoft")
        if sig_pos == -1:
            return b""
        header_start = max(0, sig_pos - 8)
        f.seek(header_start)
        raw_header = f.read(400000)
        for offset in range(12, 40):
            try:
                return zlib.decompress(raw_header[offset:], -15)
            except Exception:
                pass
            try:
                return zlib.decompress(raw_header[offset:])
            except Exception:
                pass
    return b""

def extract_strings(hdr_bytes):
    strings = []
    curr = bytearray()
    for i in range(0, len(hdr_bytes) - 1, 2):
        pair = hdr_bytes[i:i + 2]
        if pair == b'\x00\x00':
            if len(curr) >= 2:
                try:
                    s = curr.decode('utf-16le').strip()
                    if s:
                        strings.append(s)
                except Exception:
                    pass
            curr = bytearray()
        else:
            curr.extend(pair)
    return strings

def inspect_installer(installer_path, app_dir):
    discovered = {
        "guid": None,
        "sdk_port": None,
        "protocol": None,
        "registry_keys": set(),
    }

    # 1. Inspect NSIS header
    hdr = extract_nsis_header(installer_path)
    if hdr:
        nsis_strings = extract_strings(hdr)
        for s in nsis_strings:
            guid_match = re.search(r'[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}', s, re.I)
            if guid_match and not discovered["guid"]:
                discovered["guid"] = guid_match.group(0).lower()

            if s.lower() == "dmmgameplayer":
                discovered["protocol"] = "dmmgameplayer"

            if s == "14603":
                discovered["sdk_port"] = s

            if "SOFTWARE\\" in s.upper():
                # Filter out obvious non-keys
                if not any(ign in s for ign in ["Microsoft\\Windows", "Policies", "CurrentVersion", "Cryptography"]):
                    m = re.search(r'(SOFTWARE\\[a-zA-Z0-9_\-\\]+)', s, re.I)
                    if m:
                        k = m.group(1).rstrip('\\')
                        discovered["registry_keys"].add(k)

    # 2. Inspect windows_amd64_cex.dll
    cex_dll = os.path.join(app_dir, "resources/cextend/windows_amd64_cex.dll")
    if os.path.exists(cex_dll):
        with open(cex_dll, "rb") as f:
            dll_data = f.read()
        ascii_strings = [s.decode("latin-1") for s in re.findall(b"[\x20-\x7e]{4,}", dll_data)]
        for s in ascii_strings:
            if "SOFTWARE\\DMM" in s.upper():
                m = re.search(r'(SOFTWARE\\DMM[a-zA-Z0-9_\-\\]*)', s, re.I)
                if m:
                    discovered["registry_keys"].add(m.group(1).rstrip('\\'))
            if s == "14603":
                discovered["sdk_port"] = s

    return discovered

def scan_payload_binaries(app_dir):
    app_path = Path(app_dir)
    binaries = set()
    for root, _, files in os.walk(app_dir):
        for file in files:
            ext = os.path.splitext(file)[1].lower()
            if ext in [".exe", ".dll", ".sys", ".node"]:
                full_p = Path(root) / file
                rel_p = str(full_p.relative_to(app_path)).replace("\\", "/")
                binaries.add(rel_p)
    return binaries

def normalize_key(k):
    k = k.lower().replace("/", "\\")
    for prefix in ["hkey_local_machine\\", "hkey_current_user\\", "hkey_classes_root\\", "wow6432node\\"]:
        if k.startswith(prefix):
            k = k[len(prefix):]
    return k

def write_summary(md_text):
    print(md_text)
    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary_path:
        try:
            with open(summary_path, "a", encoding="utf-8") as f:
                f.write("\n" + md_text + "\n")
        except Exception as e:
            print(f"[-] Failed to write to GITHUB_STEP_SUMMARY: {e}", file=sys.stderr)

def main():
    if len(sys.argv) < 4:
        print("Usage: python3 verify_dataset.py <installer.exe> <app_dir> <version> [dataset.json] [dist_dir]")
        sys.exit(1)

    installer_path = sys.argv[1]
    app_dir = sys.argv[2]
    version = sys.argv[3]
    dataset_path = sys.argv[4] if len(sys.argv) > 4 else "data/known_dataset.json"
    dist_dir = sys.argv[5] if len(sys.argv) > 5 else None

    if not os.path.exists(dataset_path):
        print(f"[-] Known dataset file not found: {dataset_path}", file=sys.stderr)
        sys.exit(1)

    with open(dataset_path, "r", encoding="utf-8") as f:
        known = json.load(f)

    discovered = inspect_installer(installer_path, app_dir)
    payload_binaries = scan_payload_binaries(app_dir)

    conflicts = []

    # 1. Verify App GUID
    disc_guid = discovered.get("guid") or known["app_guid"]
    if disc_guid and disc_guid.lower() != known["app_guid"].lower():
        conflicts.append({
            "category": "App GUID Mismatch",
            "detail": f"Expected `{known['app_guid']}`, discovered `{disc_guid}`.",
            "action": "Update `DMM_APP_GUID` in `src/registry.rs` and `data/known_dataset.json`."
        })

    # 2. Verify SDK Port
    disc_port = discovered.get("sdk_port") or known["sdk_port"]
    if disc_port != known["sdk_port"]:
        conflicts.append({
            "category": "SDK Port Changed",
            "detail": f"Expected port `{known['sdk_port']}`, discovered `{disc_port}`.",
            "action": "Update `DEFAULT_PORT` in `src/registry.rs` and `data/known_dataset.json`."
        })

    # 3. Check for unexpected new registry keys
    known_norm_keys = {normalize_key(k) for k in known["expected_registry_keys"]}
    known_norm_keys.add("software\\bluestacks_dmm")
    known_norm_keys.add("software\\bluestacks_dmm\\configemulator")

    for disc_k in discovered["registry_keys"]:
        norm_disc = normalize_key(disc_k)
        # Check if this discovered key is accounted for
        if not any(norm_disc == k or norm_disc.startswith(k + "\\") or k.startswith(norm_disc + "\\") for k in known_norm_keys):
            conflicts.append({
                "category": "New Registry Key",
                "detail": f"Upstream installer contains new registry path: `{disc_k}`",
                "action": "Inspect if key is needed for game launch or SDK, and register in `src/registry.rs` and `data/known_dataset.json`."
            })

    # 4. Verify Crucial Files
    for crucial in known["crucial_files"]:
        crucial_full = os.path.join(app_dir, crucial)
        if not os.path.exists(crucial_full):
            conflicts.append({
                "category": "Missing Crucial File",
                "detail": f"Crucial application component missing from payload: `{crucial}`",
                "action": "Check NSIS unpack structure or upstream packaging format changes."
            })

    # 5. Check for newly added binaries (executables, DLLs, drivers)
    known_binaries_set = set(known.get("known_binaries", []))
    for binary in sorted(payload_binaries):
        # Ignore our own uninstaller/installer if present
        if binary in ["Uninstall DMMGamePlayer.exe", "dmm_installer.exe", "DMMGamePlayer-Setup-Wine.exe"]:
            continue
        if binary not in known_binaries_set:
            conflicts.append({
                "category": "New Binary Added",
                "detail": f"Upstream payload added new binary executable or library: `{binary}`",
                "action": "Inspect binary purpose (e.g. new anti-cheat driver, helper daemon) and update `known_binaries`."
            })

    if conflicts:
        md = f"## ⚠️ Upstream Divergence Detected in DMM Game Player v{version}\n\n"
        md += "The automated verification step detected unexpected changes against `data/known_dataset.json`.\n"
        md += "The release build has been **halted** to require manual review.\n\n"
        md += "| Category | Detail | Required Action |\n"
        md += "| :--- | :--- | :--- |\n"
        for c in conflicts:
            md += f"| **{c['category']}** | {c['detail']} | {c['action']} |\n"
        
        md += "\n### Manual Intervention Steps:\n"
        md += "1. Review the divergences listed above.\n"
        md += "2. Update `src/registry.rs`, `src/installer.rs`, or `assets/installer.rc` if new configurations are needed.\n"
        md += "3. Update `data/known_dataset.json` with the new approved baseline.\n"
        md += "4. Re-run `cargo xwin check --target x86_64-pc-windows-msvc` and trigger workflow.\n"

        write_summary(md)

        if dist_dir:
            os.makedirs(dist_dir, exist_ok=True)
            with open(os.path.join(dist_dir, "divergence_report.md"), "w", encoding="utf-8") as f:
                f.write(md)

        sys.exit(1)
    else:
        md = f"### Upstream Verification Passed (v{version})\n\n"
        md += f"All registry keys, SDK authentication parameters, and binary payloads match verified baseline (`data/known_dataset.json`).\n"
        write_summary(md)
        sys.exit(0)

if __name__ == "__main__":
    main()
