# 🐈‍⬛ RustCat

Your CPU’s new emotional support animal. Forget blinking graphs and sterile charts. RustCat is here to make your system monitoring adorable.

## 🚀 What is it?
RustCat is a lightweight, cross-platform taskbar companion that turns your CPU activity into a running cat animation. The faster the cat, the busier your CPU — no extra runtimes, no bloated dependencies, just pixel-perfect feline feedback.

![screen_win](/assets/screen_win.webp)

![screen_mac](/assets/screen_mac.webp)

Inspired by [Kyome22/RunCat_for_windows](https://github.com/Kyome22/RunCat_for_windows), thanks for the cute cat.

## 🪶 Features
Speedy Cat Visuals: Watch your system load as a tiny cat dashes across your taskbar.

No Runtime Baggage: Written in Rust, so it’s leaner than a whisker. (In human language: It's small and uses less memory.)

Platform Flair: Supports Windows, macOS, and Linux/KDE with native theme detection.

Auto Theme Matching: Your cat’s colors shift with your system's light/dark mode — drama-free style.

## 🧩 Installation
Visit the [Releases page](https://github.com/bearice/RustCat/releases) and grab the file. Double-click, and let the cat out.

### Linux / KDE

On Linux the tray icon uses the freedesktop StatusNotifierItem (SNI) protocol over D-Bus, so it integrates natively with KDE Plasma's system tray.

```bash
# from source (needs a Rust toolchain)
cargo build --release
# the binary is at target/release/rust_cat
```

### Nix / NixOS

A `flake.nix` is provided:

```bash
# run directly
nix run github:bearice/RustCat

# build into a profile / your config
nix build .#default
# -> result/bin/rust_cat, plus result/share/applications/rustcat.desktop

# dev shell
nix develop
```

Runtime helper tools (`kdialog`, `plasma-systemmonitor`) are wrapped onto `PATH` automatically; the app degrades gracefully if a tool is missing.

> Build note for packagers: the repo's `.cargo/config.toml` only enables
> `crt-static` on Windows. On Linux the build links dynamically against glibc,
> which is what most distros (and Nix) expect.

## 💬 Quote from the Dev
“RustCat doesn’t monitor your CPU. It vibes with it.” — Bearice
