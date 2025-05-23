fn main() {
    // Get Git commit hash
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let git_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !git_hash.is_empty() {
                println!("cargo:rustc-env=GIT_HASH={}", git_hash);
            } else {
                println!("cargo:rustc-env=GIT_HASH=N/A");
            }
        }
        _ => {
            println!("cargo:rustc-env=GIT_HASH=N/A");
        }
    }

    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/appIcon.ico")
            // .set_manifest_file("assets/manifest.xml") // This line removed
            .compile()
            .unwrap();
    }
    // Generate icons.rs
    let mut file_content = String::new();
    file_content.push_str("pub const DARK_CAT: &[&[u8]] = &[\n");
    for i in 0..5 {
        file_content.push_str(&format!(
            "\tinclude_bytes!(\"../../assets/cat/dark_cat_{}.ico\"),\n",
            i
        ));
    }
    file_content.push_str("];\n");
    file_content.push_str("pub const LIGHT_CAT: &[&[u8]] = &[\n");
    for i in 0..5 {
        file_content.push_str(&format!(
            "\tinclude_bytes!(\"../../assets/cat/light_cat_{}.ico\"),\n",
            i
        ));
    }
    file_content.push_str("];\n");
    file_content.push_str("pub const DARK_PARROT: &[&[u8]] = &[\n");
    for i in 0..10 {
        file_content.push_str(&format!(
            "\tinclude_bytes!(\"../../assets/parrot/dark_parrot_{}.ico\"),\n",
            i
        ));
    }
    file_content.push_str("];\n");
    file_content.push_str("pub const LIGHT_PARROT: &[&[u8]] = &[\n");
    for i in 0..10 {
        file_content.push_str(&format!(
            "\tinclude_bytes!(\"../../assets/parrot/light_parrot_{}.ico\"),\n",
            i
        ));
    }
    file_content.push_str("];\n");

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest_path = std::path::Path::new(&out_dir).join("icons.rs");
    std::fs::write(&dest_path, file_content).unwrap();
}
