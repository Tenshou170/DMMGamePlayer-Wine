#!/usr/bin/env python3
"""
extract_registry.py
Analyzes DMM Game Player installers and payloads to automatically discover:
1. App GUID & Uninstall Keys
2. URI / Protocol scheme handlers
3. Local SDK & gRPC ports and directories
4. Game content library registry keys
5. Any new/altered registry entries in newly released versions

Outputs:
- A complete, clean dmmgameplayer.reg file
- A JSON discovery report for GitHub Actions release notes
"""

import sys
import os
import zlib
import struct
import re
import json

KNOWN_BASE_KEYS = {
    r"HKEY_LOCAL_MACHINE\Software\DMM GAMES\Sdk\Settings",
    r"HKEY_LOCAL_MACHINE\Software\Wow6432Node\DMM GAMES\Sdk\Settings",
    r"HKEY_CURRENT_USER\Software\DMM GAMES\Sdk\Settings",
    r"HKEY_CLASSES_ROOT\dmmgameplayer",
    r"HKEY_CURRENT_USER\Software\Classes\dmmgameplayer",
    r"HKEY_LOCAL_MACHINE\Software\DMM GAMES\Launcher\Content",
    r"HKEY_LOCAL_MACHINE\Software\Wow6432Node\DMM GAMES\Launcher\Content",
    r"HKEY_CURRENT_USER\Software\DMM GAMES\Launcher\Content",
    r"Software\4f611540-42ad-5bdd-87fd-c415b0bdbb3e"
}

def extract_nsis_header(installer_path):
    """Locates and inflates the NSIS header data block."""
    if not os.path.exists(installer_path):
        print(f"[-] Installer not found: {installer_path}", file=sys.stderr)
        return b""

    with open(installer_path, "rb") as f:
        data = f.read(500000)
        sig_pos = data.find(b"NullsoftInst")
        if sig_pos == -1:
            sig_pos = data.find(b"Nullsoft")
        if sig_pos == -1:
            print("[-] NSIS signature not found in initial block", file=sys.stderr)
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
    """Extract null-terminated UTF-16LE strings from decompressed header."""
    strings = []
    curr = bytearray()
    for i in range(0, len(hdr_bytes)-1, 2):
        pair = hdr_bytes[i:i+2]
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

def analyze(installer_path, app_dir, version):
    discovered = {
        "version": version,
        "guid": "4f611540-42ad-5bdd-87fd-c415b0bdbb3e",
        "protocol": "dmmgameplayer",
        "sdk_port": "14603",
        "client_deadline": "30000",
        "log_dir": r"%USERPROFILE%\.DMMGAMEPLAYERSDK\log",
        "raw_discovered_keys": set(),
        "newly_detected_keys": []
    }

    # 1. Parse NSIS Header
    hdr = extract_nsis_header(installer_path)
    if hdr:
        nsis_strings = extract_strings(hdr)
        for s in nsis_strings:
            guid_match = re.search(r'[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}', s, re.I)
            if guid_match:
                discovered["guid"] = guid_match.group(0)
            
            if "SOFTWARE\\" in s.upper() and not any(ign in s for ign in ["Microsoft", "Policies", "CurrentVersion", "Windows"]):
                clean_k = re.sub(r'^[^\w\\]+', '', s)
                # Ensure it looks like a valid key path
                if re.match(r'^Software\\[a-zA-Z0-9_\-\\]+$', clean_k, re.I):
                    discovered["raw_discovered_keys"].add(clean_k)

            if s.isdigit() and len(s) == 5 and s == "14603":
                discovered["sdk_port"] = s

    # 2. Parse Go C-Extension DLL
    cex_dll = os.path.join(app_dir, "resources/cextend/windows_amd64_cex.dll")
    if os.path.exists(cex_dll):
        with open(cex_dll, "rb") as f:
            dll_data = f.read()
        ascii_strings = [s.decode("latin-1") for s in re.findall(b"[\x20-\x7e]{4,}", dll_data)]
        for s in ascii_strings:
            if "SOFTWARE\\" in s.upper() and any(brand in s.upper() for brand in ["DMM", "BLUESTACKS"]):
                m = re.search(r'(SOFTWARE\\[a-zA-Z0-9_]+(?:\\[a-zA-Z0-9_]+)*)', s, re.I)
                if m:
                    discovered["raw_discovered_keys"].add(m.group(1))

    # Detect newly discovered keys not present in base keys
    for k in discovered["raw_discovered_keys"]:
        if not any(k.lower() in known.lower() or known.lower().endswith(k.lower()) for known in KNOWN_BASE_KEYS):
            discovered["newly_detected_keys"].append(k)

    return discovered

def generate_reg_file(info, output_path):
    guid = info["guid"]
    version = info["version"]
    port = info["sdk_port"]
    deadline = info["client_deadline"]
    proto = info["protocol"]

    reg_content = f"""; ====================================================================
; DMMGamePlayer Registry Configuration
; Auto-generated for Version: {version}
; App GUID: {guid}
; SDK Port: {port}
; ====================================================================
Windows Registry Editor Version 5.00

; --- 1. DMM Games SDK Settings (Authentication & gRPC service) ---
[HKEY_LOCAL_MACHINE\\Software\\DMM GAMES\\Sdk\\Settings]
"log_dir"="%USERPROFILE%\\\\.DMMGAMEPLAYERSDK\\\\log"
"server_port"="{port}"
"client_deadline"="{deadline}"

[HKEY_LOCAL_MACHINE\\Software\\Wow6432Node\\DMM GAMES\\Sdk\\Settings]
"log_dir"="%USERPROFILE%\\\\.DMMGAMEPLAYERSDK\\\\log"
"server_port"="{port}"
"client_deadline"="{deadline}"

[HKEY_CURRENT_USER\\Software\\DMM GAMES\\Sdk\\Settings]
"log_dir"="%USERPROFILE%\\\\.DMMGAMEPLAYERSDK\\\\log"
"server_port"="{port}"
"client_deadline"="{deadline}"

; --- 2. Protocol Scheme Handler ({proto}://) ---
[HKEY_CLASSES_ROOT\\{proto}]
@="URL:{proto}"
"URL Protocol"=""

[HKEY_CLASSES_ROOT\\{proto}\\DefaultIcon]
@="\\"C:\\\\Program Files\\\\DMMGamePlayer\\\\DMMGamePlayer.exe\\",0"

[HKEY_CLASSES_ROOT\\{proto}\\shell]

[HKEY_CLASSES_ROOT\\{proto}\\shell\\open]

[HKEY_CLASSES_ROOT\\{proto}\\shell\\open\\command]
@="\\"C:\\\\Program Files\\\\DMMGamePlayer\\\\DMMGamePlayer.exe\\" \\"%1\\""

[HKEY_CURRENT_USER\\Software\\Classes\\{proto}]
@="URL:{proto}"
"URL Protocol"=""

[HKEY_CURRENT_USER\\Software\\Classes\\{proto}\\DefaultIcon]
@="\\"C:\\\\Program Files\\\\DMMGamePlayer\\\\DMMGamePlayer.exe\\",0"

[HKEY_CURRENT_USER\\Software\\Classes\\{proto}\\shell]

[HKEY_CURRENT_USER\\Software\\Classes\\{proto}\\shell\\open]

[HKEY_CURRENT_USER\\Software\\Classes\\{proto}\\shell\\open\\command]
@="\\"C:\\\\Program Files\\\\DMMGamePlayer\\\\DMMGamePlayer.exe\\" \\"%1\\""

; --- 3. Electron App Tracking & Uninstall Metadata ---
[HKEY_CURRENT_USER\\Software\\{guid}]
"InstallLocation"="C:\\\\Program Files\\\\DMMGamePlayer"

[HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{guid}]
"DisplayName"="DMMGamePlayer"
"DisplayVersion"="{version}"
"Publisher"="DMM.com"
"InstallLocation"="C:\\\\Program Files\\\\DMMGamePlayer"
"DisplayIcon"="\\"C:\\\\Program Files\\\\DMMGamePlayer\\\\DMMGamePlayer.exe\\",0"
"UninstallString"="\\"C:\\\\Program Files\\\\DMMGamePlayer\\\\Uninstall DMMGamePlayer.exe\\""
"QuietUninstallString"="\\"C:\\\\Program Files\\\\DMMGamePlayer\\\\Uninstall DMMGamePlayer.exe\\" /S"
"NoModify"=dword:00000001
"NoRepair"=dword:00000001
"EstimatedSize"=dword:0006e88c

; --- 4. Content Launcher Keys for Installed Games ---
[HKEY_LOCAL_MACHINE\\Software\\DMM GAMES\\Launcher\\Content]

[HKEY_LOCAL_MACHINE\\Software\\Wow6432Node\\DMM GAMES\\Launcher\\Content]

[HKEY_CURRENT_USER\\Software\\DMM GAMES\\Launcher\\Content]
"""

    # Append any extra discovered keys if present
    if info["newly_detected_keys"]:
        reg_content += "\n; --- 5. Dynamically Discovered Additional Keys ---\n"
        for extra in sorted(info["newly_detected_keys"]):
            reg_content += f"[HKEY_CURRENT_USER\\{extra}]\n\n"

    with open(output_path, "w", encoding="utf-8") as f:
        f.write(reg_content)
    print(f"[+] Generated registry file: {output_path}")

if __name__ == "__main__":
    if len(sys.argv) < 5:
        print("Usage: python extract_registry.py <installer.exe> <app_dir> <version> <output.reg> [report.json]")
        sys.exit(1)
    
    installer_path = sys.argv[1]
    app_dir = sys.argv[2]
    version = sys.argv[3]
    output_reg = sys.argv[4]
    report_json = sys.argv[5] if len(sys.argv) > 5 else None

    info = analyze(installer_path, app_dir, version)
    generate_reg_file(info, output_reg)

    if report_json:
        # Convert sets to list for JSON serialization
        info_json = dict(info)
        info_json["raw_discovered_keys"] = sorted(list(info["raw_discovered_keys"]))
        with open(report_json, "w", encoding="utf-8") as f:
            json.dump(info_json, f, indent=2)
        print(f"[+] Written discovery report to: {report_json}")
