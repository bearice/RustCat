use std::{io, path::Path};
use winres::WindowsResource;

fn main() -> io::Result<()> {
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

    let profile = std::env::var("PROFILE").unwrap();
    if profile == "release" {
        println!("cargo:rustc-cfg=release");
    }
    let mut res = WindowsResource::new();
    // This path can be absolute, or relative to your crate root.
    res.set_icon("assets/appIcon.ico");

    res.compile()?;
    generate_icon_resources()?;
    Ok(())
}

fn generate_icon_resources() -> io::Result<()> {
    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("icons.rs");
    let themes = ["light", "dark"];
    let names = [("cat", 5), ("parrot", 10usize)];
    let mut code = vec![];
    for theme in themes.iter() {
        for (name, count) in names.iter() {
            code.push(generate_icon_resources_array(theme, name, *count));
        }
    }
    std::fs::write(&dest_path, code.join("\n").as_bytes())
}

fn generate_icon_resources_array(theme: &str, name: &str, cnt: usize) -> String {
    let base = std::fs::canonicalize(Path::new("assets").join(name)).unwrap();
    let names = (0..cnt)
        .map(|i| format!("{}_{}_{}", theme, name, i))
        .collect::<Vec<_>>();
    let res = names
        .iter()
        .map(|name| {
            format!(
                r#"pub const {name}: &[u8] = include_bytes!(r"{fname}.ico");"#,
                fname = base.join(name).display(),
                name = name.to_uppercase(),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"
{res}
pub const {theme}_{name}: &[&[u8]] = &[
    {names}
];
        "#,
        res = res,
        theme = theme.to_uppercase(),
        name = name.to_uppercase(),
        names = names.join(",").to_uppercase(),
    )
}
