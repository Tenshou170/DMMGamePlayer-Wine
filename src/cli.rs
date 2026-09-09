use std::path::PathBuf;
use crate::{env_check, installer::Installer};

pub fn print_help() {
    println!("DMM Game Player Setup (Wine/Proton Compatible)");
    println!("Usage: DMMGamePlayer-Setup-Wine.exe [SUBCOMMAND] [OPTIONS]\n");
    println!("Subcommands:");
    println!("  install      Install application payload and configure registry");
    println!("  repair-reg   Re-apply registry keys (SDK port 14603 & URI handler)");
    println!("  uninstall    Remove application files and delete registry keys");
    println!("  diagnostics  Print Wine/Proton environment and health checks\n");
    println!("Options:");
    println!("  --install-dir <PATH>   Custom installation target directory");
    println!("  --silent               Run without prompting or showing message boxes");
    println!("  -h, --help             Show this help message");
}

pub fn run_cli(args: &[String]) -> Result<(), String> {
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return Ok(());
    }

    let mut subcommand = args[0].to_lowercase();
    let mut install_dir: Option<PathBuf> = None;
    let mut silent = false;

    if subcommand == "/s" {
        subcommand = "uninstall".to_string();
        silent = true;
    }

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "/S" | "/s" => {
                silent = true;
            }
            "--install-dir" => {
                if i + 1 < args.len() {
                    install_dir = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "--silent" => {
                silent = true;
            }
            _ => {}
        }
        i += 1;
    }

    let installer = Installer::new(install_dir);

    match subcommand.as_str() {
        "install" => {
            if !silent {
                println!("==> Installing DMM Game Player to: {}", installer.install_dir.display());
            }
            installer.install()?;
            if !silent {
                println!("[+] Installation and registry configuration completed successfully.");
            }
        }
        "repair" | "repair-reg" => {
            if !silent {
                println!("==> Repairing registry configuration...");
            }
            installer.repair_registry()?;
            if !silent {
                println!("[+] Registry configuration repaired successfully.");
            }
        }
        "uninstall" => {
            if !silent {
                println!("==> Uninstalling DMM Game Player from: {}", installer.install_dir.display());
            }
            installer.uninstall()?;
            if !silent {
                println!("[+] Uninstallation completed successfully.");
            }
        }
        "diagnostics" => {
            let diag = env_check::run_diagnostics();
            println!("Environment Diagnostics:");
            println!("  Runner:      {}", diag.runner_name);
            println!("  Prefix:      {}", diag.prefix_name);
            println!("  CJK Fonts:   {}", if diag.has_cjk_fonts { "Installed" } else { "Missing (cjkfonts)" });
            println!("  WebView2:    {}", if diag.has_webview2 { "Installed" } else { "Optional (Missing)" });
            println!("  SDK State:   {}", if diag.is_sdk_registered { "Registered" } else { "Not Registered" });
        }
        _ => {
            return Err(format!("Unknown subcommand: {}. Use --help for usage.", subcommand));
        }
    }

    Ok(())
}
