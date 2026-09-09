use std::env;
use std::fs;
use std::path::Path;

fn compile_resources() {
    println!("cargo:rerun-if-changed=assets");
    println!("cargo:rerun-if-changed=Cargo.toml");

    let version = env!("CARGO_PKG_VERSION");
    let parts: Vec<&str> = version.split('.').collect();
    let mut v = ["0", "0", "0", "0"];
    for (i, p) in parts.iter().enumerate().take(4) {
        v[i] = p;
    }
    let version_comma = v.join(",");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_dir_path = Path::new(&out_dir);
    let target_rc = out_dir_path.join("installer.rc");

    let content = fs::read_to_string("assets/installer.rc").expect("Failed to read assets/installer.rc");

    let mut new_content = String::with_capacity(content.len() + 64);
    for line in content.lines() {
        if line.starts_with("FILEVERSION") {
            new_content.push_str(&format!("FILEVERSION {}\n", version_comma));
        } else if line.starts_with("PRODUCTVERSION") {
            new_content.push_str(&format!("PRODUCTVERSION {}\n", version_comma));
        } else if line.contains("VALUE \"FileVersion\"") {
            new_content.push_str(&format!("            VALUE \"FileVersion\", \"{}\"\n", version));
        } else if line.contains("VALUE \"ProductVersion\"") {
            new_content.push_str(&format!("            VALUE \"ProductVersion\", \"{}\"\n", version));
        } else if line.contains("1001, \"STATIC\"") {
            new_content.push_str(&format!("    CONTROL \"Package: v{}\", 1001, \"STATIC\", SS_RIGHT | WS_CHILD | WS_VISIBLE | WS_GROUP, 190, 8, 100, 10\n", version));
        } else {
            new_content.push_str(line);
            new_content.push('\n');
        }
    }

    let _ = fs::copy("assets/dmm.ico", out_dir_path.join("dmm.ico"));
    let _ = fs::copy("assets/installer.manifest", out_dir_path.join("installer.manifest"));

    fs::write(&target_rc, new_content).expect("Failed to write generated installer.rc");

    embed_resource::compile(target_rc, &[] as &[&str]);
}

fn main() {
    compile_resources();
}
