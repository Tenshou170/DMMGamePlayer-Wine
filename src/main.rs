#![windows_subsystem = "windows"]

mod cli;
mod env_check;
mod gui;
mod installer;
mod registry;
mod resource;
mod utils;

use std::env;

fn main() {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }

    let args: Vec<String> = env::args().collect();

    // If arguments are provided (CLI subcommands or help flags), run in CLI mode
    if args.len() > 1 {
        if let Err(err) = cli::run_cli(&args[1..]) {
            eprintln!("[-] Error: {}", err);
            std::process::exit(1);
        }
    } else {
        let exe_name = env::current_exe()
            .ok()
            .and_then(|p| p.file_name().map(|f| f.to_string_lossy().to_lowercase()))
            .unwrap_or_default();

        if exe_name.contains("uninstall") {
            if let Err(err) = cli::run_cli(&["uninstall".to_string()]) {
                eprintln!("[-] Error: {}", err);
                std::process::exit(1);
            }
            return;
        }

        // No arguments: start GUI wizard
        if let Err(err) = gui::run() {
            eprintln!("[-] GUI Error: {:?}", err);
            std::process::exit(1);
        }
    }
}
