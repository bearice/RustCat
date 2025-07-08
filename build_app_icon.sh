#!/bin/bash

# Script to create a macOS app icon from existing assets
# This is optional and will be run during CI if iconutil is available

if command -v iconutil &> /dev/null; then
    echo "Creating app icon..."

    # Create iconset directory
    mkdir -p RustCat.iconset

    # We'll use the main app icon
    # Convert ICO to PNG at required sizes using sips (macOS built-in tool)
    if command -v sips &> /dev/null; then
        sips -s format png -Z 16 assets/appIcon.ico --out RustCat.iconset/icon_16x16.png
        sips -s format png -Z 32 assets/appIcon.ico --out RustCat.iconset/icon_32x32.png
        sips -s format png -Z 128 assets/appIcon.ico --out RustCat.iconset/icon_128x128.png
        sips -s format png -Z 256 assets/appIcon.ico --out RustCat.iconset/icon_256x256.png
        sips -s format png -Z 512 assets/appIcon.ico --out RustCat.iconset/icon_512x512.png
    else
        echo "sips not found"
        exit 1
    fi

    # Create the .icns file
    iconutil -c icns RustCat.iconset

    # Clean up
    rm -rf RustCat.iconset

    echo "App icon created: RustCat.icns"
else
    echo "iconutil not found, skipping app icon creation"
fi
