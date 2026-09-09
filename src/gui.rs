use std::path::PathBuf;
use windows::{
    core::{w, HSTRING},
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        System::LibraryLoader::{FindResourceExW, GetModuleHandleW, LoadResource, LockResource},
        UI::{
            Controls::{PBM_SETPOS, PBM_SETRANGE32},
            Input::KeyboardAndMouse::EnableWindow,
            WindowsAndMessaging::{
                CreateDialogIndirectParamW, CreateDialogParamW, DestroyWindow, DispatchMessageW,
                GetDlgItem, GetMessageW, GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW,
                LoadIconW, MessageBoxW, PostQuitMessage, SendMessageW, SetWindowLongPtrW,
                SetWindowTextW, ShowWindow, TranslateMessage, GWLP_USERDATA, ICON_BIG, ICON_SMALL,
                IDYES, MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MB_YESNO, MSG, RT_DIALOG,
                SW_SHOW, WM_CLOSE, WM_COMMAND, WM_DESTROY, WM_INITDIALOG, WM_SETICON,
            },
        },
    },
};

use crate::{
    env_check,
    installer::Installer,
    resource::*,
    utils::{center_window, open_select_folder_dialog},
};

struct GuiState {
    installer: Installer,
}

pub fn run() -> Result<(), windows::core::Error> {
    let state = Box::new(GuiState {
        installer: Installer::default(),
    });
    let state_ptr = Box::into_raw(state) as isize;

    let instance = unsafe { GetModuleHandleW(None)? };
    let dialog = unsafe {
        CreateDialogParamW(
            instance,
            IDD_MAIN,
            None,
            Some(dlg_proc),
            LPARAM(state_ptr),
        )
    }.or_else(|_| unsafe {
        let mut hrsrc = FindResourceExW(instance, RT_DIALOG, IDD_MAIN, 0);
        if hrsrc.is_invalid() {
            hrsrc = FindResourceExW(instance, RT_DIALOG, IDD_MAIN, 0x409);
        }
        if hrsrc.is_invalid() {
            hrsrc = FindResourceExW(instance, RT_DIALOG, IDD_MAIN, 0x411);
        }
        if hrsrc.is_invalid() {
            return Err(windows::core::Error::from_win32());
        }
        let hglobal = LoadResource(instance, hrsrc)?;
        let template = LockResource(hglobal);
        if template.is_null() {
            return Err(windows::core::Error::from_win32());
        }
        CreateDialogIndirectParamW(
            instance,
            template as *const _,
            None,
            Some(dlg_proc),
            LPARAM(state_ptr),
        )
    })?;

    center_window(dialog)?;
    unsafe {
        let _ = ShowWindow(dialog, SW_SHOW);
    }

    let mut message = MSG::default();
    unsafe {
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }

    Ok(())
}

fn get_state(dialog: HWND) -> Option<&'static mut GuiState> {
    unsafe {
        let ptr = GetWindowLongPtrW(dialog, GWLP_USERDATA) as *mut GuiState;
        if ptr.is_null() {
            None
        } else {
            Some(&mut *ptr)
        }
    }
}

fn get_text_from_edit(control: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(control);
        if len == 0 {
            return String::new();
        }
        let mut buffer: Vec<u16> = vec![0; (len + 1) as usize];
        let copied = GetWindowTextW(control, &mut buffer);
        buffer.truncate(copied as usize);
        String::from_utf16_lossy(&buffer)
    }
}

fn update_installed_status(dialog: HWND, state: &GuiState) {
    if let Ok(ctrl) = unsafe { GetDlgItem(dialog, IDC_INSTALLED_LABEL) } {
        let text = match state.installer.get_installed_version() {
            Some(ver) => format!("Target Status: Installed ({})", ver),
            None => "Target Status: Not Installed".to_string(),
        };
        unsafe { let _ = SetWindowTextW(ctrl, &HSTRING::from(&text)); }
    }
}

fn set_buttons_enabled(dialog: HWND, enabled: bool) {
    let buttons = [
        IDC_INSTALL,
        IDC_REPAIR,
        IDC_UNINSTALL,
        IDC_INSTALL_PATH_BROWSE,
    ];
    for &id in &buttons {
        if let Ok(ctrl) = unsafe { GetDlgItem(dialog, id) } {
            unsafe { let _ = EnableWindow(ctrl, enabled); }
        }
    }
}

fn set_progress(dialog: HWND, percent: u32) {
    if let Ok(ctrl) = unsafe { GetDlgItem(dialog, IDC_PROGRESS) } {
        unsafe {
            SendMessageW(ctrl, PBM_SETPOS, WPARAM(percent as usize), None);
        }
    }
}

fn set_status(dialog: HWND, text: &str) {
    if let Ok(ctrl) = unsafe { GetDlgItem(dialog, IDC_STATUS) } {
        unsafe {
            let _ = SetWindowTextW(ctrl, &HSTRING::from(text));
        }
    }
}

extern "system" fn dlg_proc(
    dialog: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    match msg {
        WM_INITDIALOG => {
            let state_ptr = lparam.0 as *mut GuiState;
            unsafe {
                SetWindowLongPtrW(dialog, GWLP_USERDATA, state_ptr as isize);

                // Set Icon
                if let Ok(instance) = GetModuleHandleW(None) {
                    if let Ok(icon) = LoadIconW(instance, IDI_DMM) {
                        SendMessageW(dialog, WM_SETICON, WPARAM(ICON_BIG as usize), LPARAM(icon.0 as isize));
                        SendMessageW(dialog, WM_SETICON, WPARAM(ICON_SMALL as usize), LPARAM(icon.0 as isize));
                    }
                }

                // Initialize progress bar range (0 to 100)
                if let Ok(prog) = GetDlgItem(dialog, IDC_PROGRESS) {
                    SendMessageW(prog, PBM_SETRANGE32, WPARAM(0), LPARAM(100));
                    SendMessageW(prog, PBM_SETPOS, WPARAM(0), None);
                }

                // Run environment diagnostics
                let diag = env_check::run_diagnostics();

                if let Ok(ctrl) = GetDlgItem(dialog, IDC_RUNNER_STATUS) {
                    let text = format!("Runner: {}", diag.runner_name);
                    let _ = SetWindowTextW(ctrl, &HSTRING::from(&text));
                }

                if let Ok(ctrl) = GetDlgItem(dialog, IDC_PREFIX_STATUS) {
                    let text = format!("Prefix: {}", diag.prefix_name);
                    let _ = SetWindowTextW(ctrl, &HSTRING::from(&text));
                }

                if let Ok(ctrl) = GetDlgItem(dialog, IDC_DIAGNOSTICS_STATUS) {
                    let cjk_str = if diag.has_cjk_fonts { "OK" } else { "Missing (cjkfonts)" };
                    let wv2_str = if diag.has_webview2 { "OK" } else { "Optional" };
                    let sdk_str = if diag.is_sdk_registered { "Registered" } else { "Not Registered" };
                    let diag_text = format!("Fonts: {}  |  WebView2: {}  |  SDK Port: {}", cjk_str, wv2_str, sdk_str);
                    let _ = SetWindowTextW(ctrl, &HSTRING::from(&diag_text));
                }

                if let Ok(ctrl) = GetDlgItem(dialog, IDC_INSTALL_PATH) {
                    let default_path = r"C:\Program Files\DMMGamePlayer";
                    let _ = SetWindowTextW(ctrl, &HSTRING::from(default_path));
                }

                if let Ok(ctrl) = GetDlgItem(dialog, IDC_VERSION_LABEL) {
                    let ver_text = format!("Package: v{}", env!("CARGO_PKG_VERSION"));
                    let _ = SetWindowTextW(ctrl, &HSTRING::from(&ver_text));
                }
            }

            if let Some(state) = get_state(dialog) {
                update_installed_status(dialog, state);
            }

            1
        }
        WM_COMMAND => {
            let control_id = (wparam.0 & 0xffff) as i32;
            let Some(state) = get_state(dialog) else {
                return 0;
            };

            // Update install directory from edit control
            if let Ok(path_edit) = unsafe { GetDlgItem(dialog, IDC_INSTALL_PATH) } {
                let current_str = get_text_from_edit(path_edit);
                if !current_str.is_empty() {
                    state.installer.install_dir = PathBuf::from(current_str);
                }
            }

            match control_id {
                IDC_INSTALL_PATH_BROWSE => {
                    if let Some(selected) = open_select_folder_dialog(dialog, Some(&state.installer.install_dir)) {
                        state.installer.install_dir = selected.clone();
                        if let Ok(path_edit) = unsafe { GetDlgItem(dialog, IDC_INSTALL_PATH) } {
                            if let Some(path_str) = selected.to_str() {
                                unsafe { let _ = SetWindowTextW(path_edit, &HSTRING::from(path_str)); }
                            }
                        }
                        update_installed_status(dialog, state);
                    }
                }
                IDC_INSTALL => {
                    set_buttons_enabled(dialog, false);
                    set_progress(dialog, 15);
                    set_status(dialog, "Installing payload files...");

                    set_progress(dialog, 40);
                    match state.installer.install() {
                        Ok(_) => {
                            set_progress(dialog, 100);
                            set_status(dialog, "Installation complete.");
                            update_installed_status(dialog, state);
                            unsafe {
                                MessageBoxW(
                                    dialog,
                                    w!("DMM Game Player has been successfully installed and configured!"),
                                    w!("Installation Success"),
                                    MB_ICONINFORMATION | MB_OK,
                                );
                            }
                        }
                        Err(err) => {
                            set_progress(dialog, 0);
                            set_status(dialog, "Installation failed.");
                            unsafe {
                                MessageBoxW(
                                    dialog,
                                    &HSTRING::from(&err),
                                    w!("Installation Error"),
                                    MB_ICONERROR | MB_OK,
                                );
                            }
                        }
                    }
                    set_buttons_enabled(dialog, true);
                }
                IDC_REPAIR => {
                    set_buttons_enabled(dialog, false);
                    set_progress(dialog, 30);
                    set_status(dialog, "Configuring registry keys and SDK port...");

                    match state.installer.repair_registry() {
                        Ok(_) => {
                            set_progress(dialog, 100);
                            set_status(dialog, "Registry repaired successfully.");
                            update_installed_status(dialog, state);
                            unsafe {
                                MessageBoxW(
                                    dialog,
                                    w!("Registry keys, scheme handler, and SDK port (14603) successfully configured!"),
                                    w!("Registry Repaired"),
                                    MB_ICONINFORMATION | MB_OK,
                                );
                            }
                        }
                        Err(err) => {
                            set_progress(dialog, 0);
                            set_status(dialog, "Registry repair failed.");
                            unsafe {
                                MessageBoxW(
                                    dialog,
                                    &HSTRING::from(&err),
                                    w!("Repair Error"),
                                    MB_ICONERROR | MB_OK,
                                );
                            }
                        }
                    }
                    set_buttons_enabled(dialog, true);
                }
                IDC_UNINSTALL => {
                    let confirm = unsafe {
                        MessageBoxW(
                            dialog,
                            w!("Are you sure you want to uninstall DMM Game Player and remove its registry keys?"),
                            w!("Confirm Uninstall"),
                            MB_ICONINFORMATION | MB_YESNO,
                        )
                    };
                    if confirm == IDYES {
                        set_buttons_enabled(dialog, false);
                        set_progress(dialog, 30);
                        set_status(dialog, "Removing application files and registry keys...");

                        match state.installer.uninstall() {
                            Ok(_) => {
                                set_progress(dialog, 100);
                                set_status(dialog, "Uninstallation complete.");
                                update_installed_status(dialog, state);
                                unsafe {
                                    MessageBoxW(
                                        dialog,
                                        w!("DMM Game Player has been uninstalled."),
                                        w!("Uninstalled"),
                                        MB_ICONINFORMATION | MB_OK,
                                    );
                                }
                            }
                            Err(err) => {
                                set_progress(dialog, 0);
                                set_status(dialog, "Uninstallation failed.");
                                unsafe {
                                    MessageBoxW(
                                        dialog,
                                        &HSTRING::from(&err),
                                        w!("Uninstall Error"),
                                        MB_ICONERROR | MB_OK,
                                    );
                                }
                            }
                        }
                        set_buttons_enabled(dialog, true);
                    }
                }
                _ => {}
            }
            0
        }
        WM_CLOSE => {
            unsafe { let _ = DestroyWindow(dialog); }
            0
        }
        WM_DESTROY => {
            let state_ptr = unsafe { GetWindowLongPtrW(dialog, GWLP_USERDATA) } as *mut GuiState;
            if !state_ptr.is_null() {
                unsafe { drop(Box::from_raw(state_ptr)); }
            }
            unsafe { PostQuitMessage(0); }
            0
        }
        _ => 0,
    }
}

