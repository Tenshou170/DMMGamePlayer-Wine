use std::{ffi::CStr, path::Path};
use windows_registry::{CURRENT_USER, LOCAL_MACHINE};
use windows::{
    core::s,
    Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress},
};

#[allow(dead_code)]
pub struct EnvironmentDiagnostics {
    pub runner_name: String,
    pub runner_flavor: String,
    pub prefix_name: String,
    pub is_wine: bool,
    pub has_cjk_fonts: bool,
    pub has_webview2: bool,
    pub is_sdk_registered: bool,
}

pub fn get_wine_version_info() -> Option<(String, Option<String>)> {
    unsafe {
        let ntdll = GetModuleHandleA(s!("ntdll.dll")).ok()?;
        let wine_get_version = GetProcAddress(ntdll, s!("wine_get_version"))?;
        let func_ver: extern "C" fn() -> *const i8 = std::mem::transmute(wine_get_version);
        let version_ptr = func_ver();
        if version_ptr.is_null() {
            return None;
        }
        let ver_str = CStr::from_ptr(version_ptr).to_string_lossy().into_owned();

        let build_id = if let Some(proc) = GetProcAddress(ntdll, s!("wine_get_build_id")) {
            let func_build: extern "C" fn() -> *const i8 = std::mem::transmute(proc);
            let build_ptr = func_build();
            if !build_ptr.is_null() {
                Some(CStr::from_ptr(build_ptr).to_string_lossy().into_owned())
            } else {
                None
            }
        } else {
            None
        };

        Some((ver_str, build_id))
    }
}

pub fn check_cjk_fonts() -> bool {
    let font_dir = Path::new(r"C:\windows\Fonts");
    let font_candidates = [
        "msgothic.ttc",
        "msgoth.ttc",
        "meiryo.ttc",
        "meiryob.ttc",
        "YuGothM.ttc",
        "NotoSansCJK-Regular.ttc",
    ];

    for candidate in font_candidates {
        if font_dir.join(candidate).exists() {
            return true;
        }
    }
    false
}

pub fn check_webview2() -> bool {
    const CLIENT_KEY: &str =
        r"Software\Microsoft\EdgeUpdate\Clients\{56EB18F8-B008-4CBD-B6D2-8C97FE7E9062}";
    
    if LOCAL_MACHINE.open(CLIENT_KEY).is_ok() {
        return true;
    }
    if CURRENT_USER.open(CLIENT_KEY).is_ok() {
        return true;
    }
    false
}

pub fn check_sdk_registration() -> bool {
    const SDK_KEY: &str = r"Software\DMM GAMES\Sdk\Settings";
    if let Ok(key) = LOCAL_MACHINE.open(SDK_KEY) {
        if let Ok(val) = key.get_string("server_port") {
            return val == "14603";
        }
    }
    false
}

fn inspect_proc_environment() -> (Option<String>, Option<String>) {
    let mut prefix_path: Option<String> = None;
    let mut runner_path: Option<String> = None;

    // 1. Check Win32 environment variables first
    if let Ok(val) = std::env::var("WINEPREFIX") {
        if !val.trim().is_empty() {
            prefix_path = Some(val);
        }
    } else if let Ok(val) = std::env::var("STEAM_COMPAT_DATA_PATH") {
        if !val.trim().is_empty() {
            prefix_path = Some(val);
        }
    }

    // 2. Read Linux /proc/self/environ via Z: drive if available
    if let Ok(environ) = std::fs::read(r"Z:\proc\self\environ") {
        for entry in environ.split(|&b| b == 0) {
            let s = String::from_utf8_lossy(entry);
            if prefix_path.is_none() {
                if let Some(stripped) = s.strip_prefix("WINEPREFIX=") {
                    prefix_path = Some(stripped.to_string());
                } else if let Some(stripped) = s.strip_prefix("STEAM_COMPAT_DATA_PATH=") {
                    prefix_path = Some(stripped.to_string());
                }
            }
            if runner_path.is_none() {
                if let Some(stripped) = s.strip_prefix("STEAM_COMPAT_TOOL_PATHS=") {
                    runner_path = Some(stripped.to_string());
                } else if let Some(stripped) = s.strip_prefix("WINE=") {
                    runner_path = Some(stripped.to_string());
                }
            }
        }
    }

    // 3. Inspect Linux /proc/self/cmdline to discover the exact runner binary
    if runner_path.is_none() {
        if let Ok(cmdline) = std::fs::read(r"Z:\proc\self\cmdline") {
            let s = String::from_utf8_lossy(&cmdline);
            for arg in s.split('\0') {
                if arg.contains("/runners/")
                    || arg.contains("/compatibilitytools.d/")
                    || arg.contains("/steamapps/common/Proton")
                    || arg.contains("/usr/bin/wine")
                    || arg.contains("/usr/lib/wine")
                {
                    runner_path = Some(arg.to_string());
                    break;
                }
            }
        }
    }

    (prefix_path, runner_path)
}

fn extract_folder_after<'a>(path: &'a str, marker: &str) -> Option<&'a str> {
    let idx = path.find(marker)?;
    let remainder = &path[idx + marker.len()..];
    let trimmed = remainder.trim_start_matches('/');
    let end_idx = trimmed.find('/').unwrap_or(trimmed.len());
    let folder = &trimmed[..end_idx];
    if folder.is_empty() {
        None
    } else {
        Some(folder)
    }
}

pub fn run_diagnostics() -> EnvironmentDiagnostics {
    let wine_info = get_wine_version_info();
    if wine_info.is_none() {
        return EnvironmentDiagnostics {
            runner_name: "Native Windows (NT x86_64)".to_string(),
            runner_flavor: "Native Windows".to_string(),
            prefix_name: "Native Windows System".to_string(),
            is_wine: false,
            has_cjk_fonts: check_cjk_fonts(),
            has_webview2: check_webview2(),
            is_sdk_registered: check_sdk_registration(),
        };
    }

    let (wine_ver, build_id) = wine_info.unwrap();
    let (prefix_path, runner_path) = inspect_proc_environment();

    // 1. Robust Runner Resolution
    let (runner_flavor, runner_name) = {
        let runner_str = runner_path.as_deref().unwrap_or("");
        let build_str = build_id.as_deref().unwrap_or("");

        if let Some(bottle_runner) = extract_folder_after(runner_str, "/bottles/runners/") {
            (
                "Bottles Runner".to_string(),
                format!("Bottles: {} (Wine {})", bottle_runner, wine_ver),
            )
        } else if let Some(steam_tool) = extract_folder_after(runner_str, "/compatibilitytools.d/") {
            (
                "Steam Proton (Custom)".to_string(),
                format!("Steam Proton: {} (Wine {})", steam_tool, wine_ver),
            )
        } else if let Some(proton_ver) = extract_folder_after(runner_str, "/steamapps/common/") {
            (
                "Valve Proton".to_string(),
                format!("{}: Wine {}", proton_ver, wine_ver),
            )
        } else if let Some(lutris_runner) = extract_folder_after(runner_str, "/lutris/runners/wine/") {
            (
                "Lutris Runner".to_string(),
                format!("Lutris: {} (Wine {})", lutris_runner, wine_ver),
            )
        } else if build_str.contains("CachyOS") {
            (
                "Proton-CachyOS".to_string(),
                format!("Proton-CachyOS (Wine {})", wine_ver),
            )
        } else if build_str.contains("TkG") || build_str.contains("bleeding.edge") {
            (
                "Wine-TkG / Soda".to_string(),
                format!("Wine-TkG (Version {})", wine_ver),
            )
        } else if build_str.contains("Staging") {
            (
                "Wine-Staging".to_string(),
                format!("Wine-Staging {}", wine_ver),
            )
        } else if build_str.to_lowercase().contains("proton") {
            (
                "Proton".to_string(),
                format!("Proton: {} (Wine {})", build_str, wine_ver),
            )
        } else if runner_str.contains("/usr/bin/wine") || runner_str.contains("/usr/lib/wine") {
            (
                "System Wine".to_string(),
                format!("System Wine {}", wine_ver),
            )
        } else if !build_str.is_empty() {
            (
                "Wine".to_string(),
                format!("{} ({})", build_str, wine_ver),
            )
        } else {
            (
                "Wine".to_string(),
                format!("Wine {}", wine_ver),
            )
        }
    };

    // 2. Robust Prefix Resolution
    let prefix_name = {
        let p_str = prefix_path.as_deref().unwrap_or("");
        
        let (base_desc, host_path) = if let Some(bottle_name) = extract_folder_after(p_str, "/bottles/bottles/") {
            (format!("Bottles: \"{}\"", bottle_name), Some(p_str))
        } else if let Some(appid) = extract_folder_after(p_str, "/compatdata/") {
            let appid_clean = appid.trim_end_matches("/pfx");
            (format!("Steam Proton: AppID {}", appid_clean), Some(p_str))
        } else if let Some(lutris_game) = extract_folder_after(p_str, "/Games/") {
            (format!("Lutris: \"{}\"", lutris_game), Some(p_str))
        } else if p_str.ends_with(".wine") || p_str.is_empty() {
            ("System Default (~/.wine)".to_string(), Some("~/.wine"))
        } else {
            (format!("Custom Prefix ({})", p_str), Some(p_str))
        };

        // Try reading version stamp from prefix root if Z: drive is accessible
        let mut version_tag: Option<String> = None;
        if let Some(host_p) = host_path {
            if host_p.starts_with('/') {
                let z_prefix = format!("Z:{}", host_p.replace('/', "\\"));
                let candidates = [
                    format!(r"{}\version", z_prefix),
                    format!(r"{}\..\version", z_prefix),
                    format!(r"{}\..\..\version", z_prefix),
                ];
                for c in candidates {
                    if let Ok(content) = std::fs::read_to_string(&c) {
                        let first = content.lines().next().unwrap_or("").trim();
                        if !first.is_empty() {
                            let clean_tag = if let Some((_, tag)) = first.split_once(' ') {
                                tag
                            } else {
                                first
                            };
                            version_tag = Some(clean_tag.to_string());
                            break;
                        }
                    }
                }
            }
        }

        if let Some(tag) = version_tag {
            format!("{} [{}]", base_desc, tag)
        } else {
            base_desc
        }
    };

    EnvironmentDiagnostics {
        runner_name,
        runner_flavor,
        prefix_name,
        is_wine: true,
        has_cjk_fonts: check_cjk_fonts(),
        has_webview2: check_webview2(),
        is_sdk_registered: check_sdk_registration(),
    }
}
