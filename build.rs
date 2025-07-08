use flate2::{write::GzEncoder, Compression};
use std::{collections::HashMap, io, path::Path};

// only include winres if compiling for Windows
#[cfg(target_os = "windows")]
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

    // Generate Windows resource file if compiling for Windows
    #[cfg(target_os = "windows")]
    {
        let mut res = WindowsResource::new();
        res.set_icon("assets/appIcon.ico");
        res.compile()?;
    }

    generate_icon_resources()?;
    Ok(())
}

fn generate_icon_resources() -> io::Result<()> {
    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("icon_data.rs");

    // Define icon configurations that match the icon manager structure
    let icon_configs = [
        ("cat", [("light", 5), ("dark", 5)]),
        ("parrot", [("light", 10), ("dark", 10)]),
    ];

    // Concatenate ALL icons into one big chunk for maximum compression
    let (compressed_data, icon_metadata) = generate_all_icons_compressed(&icon_configs)?;

    // Generate code with single compressed chunk
    let code = generate_single_chunk_module(&compressed_data, &icon_metadata);

    std::fs::write(&dest_path, code.as_bytes())
}

// Icon metadata for the single compressed chunk
#[derive(Debug)]
struct IconGroupMetadata {
    icon_name: String,
    theme: String,
    offset: usize,
    sizes: Vec<usize>,
}

fn generate_all_icons_compressed(
    icon_configs: &[(&str, [(&str, usize); 2])],
) -> io::Result<(String, Vec<IconGroupMetadata>)> {
    let mut all_icons_data = Vec::new();
    let mut metadata = Vec::new();

    // Collect all icon data
    for (icon_name, themes) in icon_configs.iter() {
        for (theme, count) in themes.iter() {
            let base = std::fs::canonicalize(Path::new("assets").join(icon_name))?;
            let icon_file_names = (0..*count)
                .map(|i| format!("{}_{}_{}", theme, icon_name, i))
                .collect::<Vec<_>>();

            let mut group_sizes = Vec::new();
            let group_offset = all_icons_data.len();

            // Read all icons for this group
            for name in &icon_file_names {
                let icon_path = base.join(format!("{}.ico", name));
                let icon_data = std::fs::read(&icon_path)?;
                group_sizes.push(icon_data.len());
                all_icons_data.extend_from_slice(&icon_data);
            }

            metadata.push(IconGroupMetadata {
                icon_name: icon_name.to_string(),
                theme: theme.to_string(),
                offset: group_offset,
                sizes: group_sizes,
            });
        }
    }

    // Compress all icons together
    let mut gz_encoder = GzEncoder::new(Vec::new(), Compression::best());
    std::io::copy(&mut &all_icons_data[..], &mut gz_encoder)?;
    let compressed = gz_encoder.finish()?;

    // Write compressed data to OUT_DIR
    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    let compressed_path = Path::new(&out_dir).join("all_icons.gz");
    std::fs::write(&compressed_path, &compressed)?;

    println!(
        "cargo:info=All icons compressed: {} bytes -> {} bytes ({:.1}% reduction)",
        all_icons_data.len(),
        compressed.len(),
        100.0 * (1.0 - compressed.len() as f64 / all_icons_data.len() as f64)
    );

    Ok((compressed_path.display().to_string(), metadata))
}

fn generate_single_chunk_module(compressed_path: &str, metadata: &[IconGroupMetadata]) -> String {
    let mut code = String::new();

    code.push_str("// All icons compressed into a single chunk for maximum compression\n");
    code.push_str(&format!(
        "pub const ALL_ICONS_COMPRESSED: &[u8] = include_bytes!(r\"{}\");\n\n",
        compressed_path
    ));

    code.push_str("use std::collections::HashMap;\n\n");

    // IconGroupInfo is now defined in main.rs

    // Generate size arrays for each group
    for meta in metadata {
        let sizes_array = meta
            .sizes
            .iter()
            .map(|size| size.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        code.push_str(&format!(
            "const {}_{}_{}_SIZES: &[u32] = &[{}];\n",
            meta.theme.to_uppercase(),
            meta.icon_name.to_uppercase(),
            "INDIVIDUAL",
            sizes_array
        ));
    }

    code.push('\n');

    // Generate function to get icon metadata
    code.push_str("pub fn get_icon_metadata() -> IconData {\n");
    code.push_str("    let mut icons = HashMap::new();\n\n");

    // Group metadata by icon name
    let mut grouped_metadata: HashMap<String, Vec<&IconGroupMetadata>> = HashMap::new();
    for meta in metadata {
        grouped_metadata
            .entry(meta.icon_name.clone())
            .or_default()
            .push(meta);
    }

    for (icon_name, themes) in grouped_metadata {
        code.push_str(&format!("    // {} icons\n", icon_name));
        code.push_str(&format!("    let mut {} = HashMap::new();\n", icon_name));

        for meta in themes {
            code.push_str(&format!(
                "    {}.insert(\"{}\", IconGroupInfo {{ offset: {}, sizes: {}_{}_INDIVIDUAL_SIZES }});\n",
                icon_name,
                meta.theme,
                meta.offset,
                meta.theme.to_uppercase(),
                meta.icon_name.to_uppercase()
            ));
        }

        code.push_str(&format!(
            "    icons.insert(\"{}\", {});\n\n",
            icon_name, icon_name
        ));
    }

    code.push_str("    icons\n");
    code.push_str("}\n");

    code
}
