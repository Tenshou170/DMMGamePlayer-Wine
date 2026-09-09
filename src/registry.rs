use std::path::Path;
use windows_registry::{CLASSES_ROOT, CURRENT_USER, LOCAL_MACHINE};

pub const DMM_APP_GUID: &str = "4f611540-42ad-5bdd-87fd-c415b0bdbb3e";
pub const DMM_PROTOCOL: &str = "dmmgameplayer";
pub const DEFAULT_PORT: &str = "14603";
pub const DEFAULT_DEADLINE: &str = "30000";

pub fn apply_dmm_registry_settings(install_dir: &Path, version: &str) -> Result<(), Box<dyn std::error::Error>> {
    let install_dir_str = install_dir.to_str().ok_or("Invalid install path")?;
    let exe_path = install_dir.join("DMMGamePlayer.exe");
    let exe_str = exe_path.to_str().ok_or("Invalid exe path")?;
    let uninstaller_path = install_dir.join("Uninstall DMMGamePlayer.exe");
    let uninstaller_str = uninstaller_path.to_str().unwrap_or(exe_str);

    // 1. DMM GAMES Sdk Settings
    let sdk_keys = [
        r"Software\DMM GAMES\Sdk\Settings",
        r"Software\Wow6432Node\DMM GAMES\Sdk\Settings",
    ];

    for key_path in sdk_keys {
        if let Ok(key) = LOCAL_MACHINE.create(key_path) {
            let _ = key.set_string("log_dir", r"%USERPROFILE%\.DMMGAMEPLAYERSDK\log");
            let _ = key.set_string("server_port", DEFAULT_PORT);
            let _ = key.set_string("client_deadline", DEFAULT_DEADLINE);
        }
        if let Ok(key) = CURRENT_USER.create(key_path) {
            let _ = key.set_string("log_dir", r"%USERPROFILE%\.DMMGAMEPLAYERSDK\log");
            let _ = key.set_string("server_port", DEFAULT_PORT);
            let _ = key.set_string("client_deadline", DEFAULT_DEADLINE);
        }
    }

    // 2. Protocol Scheme Handler (dmmgameplayer://)
    let protocol_bases = [
        (CLASSES_ROOT, DMM_PROTOCOL),
        (CURRENT_USER, r"Software\Classes\dmmgameplayer"),
    ];

    for (root, base_path) in protocol_bases {
        if let Ok(proto_key) = root.create(base_path) {
            let _ = proto_key.set_string("", "URL:dmmgameplayer");
            let _ = proto_key.set_string("URL Protocol", "");
        }

        let icon_path = format!(r"{}\DefaultIcon", base_path);
        if let Ok(icon_key) = root.create(&icon_path) {
            let _ = icon_key.set_string("", format!(r#""{}",0"#, exe_str));
        }

        let cmd_path = format!(r"{}\shell\open\command", base_path);
        if let Ok(cmd_key) = root.create(&cmd_path) {
            let _ = cmd_key.set_string("", format!(r#""{}" "%1""#, exe_str));
        }
    }

    // 3. App Tracking & Uninstall Key
    let app_guid_key = format!(r"Software\{}", DMM_APP_GUID);
    if let Ok(key) = CURRENT_USER.create(&app_guid_key) {
        let _ = key.set_string("InstallLocation", install_dir_str);
    }

    let uninstall_key_path = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{}",
        DMM_APP_GUID
    );
    if let Ok(uninst_key) = CURRENT_USER.create(&uninstall_key_path) {
        let _ = uninst_key.set_string("DisplayName", "DMMGamePlayer");
        let _ = uninst_key.set_string("DisplayVersion", version);
        let _ = uninst_key.set_string("Publisher", "DMM.com");
        let _ = uninst_key.set_string("InstallLocation", install_dir_str);
        let _ = uninst_key.set_string("DisplayIcon", format!(r#""{}",0"#, exe_str));
        let _ = uninst_key.set_string("UninstallString", format!(r#""{}""#, uninstaller_str));
        let _ = uninst_key.set_string("QuietUninstallString", format!(r#""{}" /S"#, uninstaller_str));
        let _ = uninst_key.set_u32("NoModify", 1);
        let _ = uninst_key.set_u32("NoRepair", 1);
    }

    // 4. Content / Launcher Keys for Installed Games
    let _ = LOCAL_MACHINE.create(r"Software\DMM GAMES\Launcher\Content");
    let _ = LOCAL_MACHINE.create(r"Software\Wow6432Node\DMM GAMES\Launcher\Content");
    let _ = CURRENT_USER.create(r"Software\DMM GAMES\Launcher\Content");

    Ok(())
}

pub fn remove_dmm_registry_settings() {
    let _ = CURRENT_USER.remove_tree(r"Software\Classes\dmmgameplayer");
    let _ = CLASSES_ROOT.remove_tree(DMM_PROTOCOL);

    let _ = CURRENT_USER.remove_tree(format!(r"Software\{}", DMM_APP_GUID));
    let _ = CURRENT_USER.remove_tree(r"Software\DMM GAMES");

    let _ = LOCAL_MACHINE.remove_tree(format!(r"Software\{}", DMM_APP_GUID));
    let _ = LOCAL_MACHINE.remove_tree(r"Software\DMM GAMES");
    let _ = LOCAL_MACHINE.remove_tree(r"Software\Wow6432Node\DMM GAMES");

    let _ = CURRENT_USER.remove_tree(format!(
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{}",
        DMM_APP_GUID
    ));
    let _ = LOCAL_MACHINE.remove_tree(format!(
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{}",
        DMM_APP_GUID
    ));
}

pub fn get_installed_dmm_version() -> Option<String> {
    let uninstall_key_path = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{}",
        DMM_APP_GUID
    );
    if let Ok(key) = CURRENT_USER.open(&uninstall_key_path) {
        if let Ok(val) = key.get_string("DisplayVersion") {
            if !val.trim().is_empty() {
                return Some(val);
            }
        }
    }
    if let Ok(key) = LOCAL_MACHINE.open(&uninstall_key_path) {
        if let Ok(val) = key.get_string("DisplayVersion") {
            if !val.trim().is_empty() {
                return Some(val);
            }
        }
    }
    None
}

