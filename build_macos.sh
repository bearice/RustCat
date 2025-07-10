#!/bin/bash

# macOS build script for RustCat
# This script handles all macOS-specific build tasks including:
# - Building for both Intel and Apple Silicon
# - Creating universal binaries
# - Building app bundles
# - Creating DMG images

set -e

echo "🍎 Starting macOS build process..."

# Configuration
APP_NAME="RustCat"
BUNDLE_ID="com.bearice.rustcat"
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')

# Build targets
INTEL_TARGET="x86_64-apple-darwin"
ARM_TARGET="aarch64-apple-darwin"

# Ensure targets are installed
echo "📦 Installing build targets..."
rustup target add $INTEL_TARGET
rustup target add $ARM_TARGET

# Build for both architectures
echo "🔨 Building for Intel (x86_64)..."
cargo build --release --target $INTEL_TARGET

echo "🔨 Building for Apple Silicon (aarch64)..."
cargo build --release --target $ARM_TARGET

# Create universal binary
echo "🔗 Creating universal binary..."
lipo -create -output rust_cat_universal \
    target/$INTEL_TARGET/release/rust_cat \
    target/$ARM_TARGET/release/rust_cat

# Verify universal binary
echo "✅ Verifying universal binary..."
lipo -info rust_cat_universal
file rust_cat_universal

# Create app icon
echo "🎨 Creating app icon..."
if command -v iconutil &> /dev/null; then
    # Create iconset directory
    mkdir -p RustCat.iconset

    # Convert ICO to PNG at required sizes using sips (macOS built-in tool)
    if command -v sips &> /dev/null; then
        sips -s format png -Z 16 assets/appIcon.ico --out RustCat.iconset/icon_16x16.png
        sips -s format png -Z 32 assets/appIcon.ico --out RustCat.iconset/icon_32x32.png
        sips -s format png -Z 128 assets/appIcon.ico --out RustCat.iconset/icon_128x128.png
        sips -s format png -Z 256 assets/appIcon.ico --out RustCat.iconset/icon_256x256.png
        sips -s format png -Z 512 assets/appIcon.ico --out RustCat.iconset/icon_512x512.png
    else
        echo "❌ sips not found - cannot create app icon"
        exit 1
    fi

    # Create the .icns file
    iconutil -c icns RustCat.iconset

    echo "✅ App icon created: RustCat.icns"
else
    echo "❌ iconutil not found - cannot create app icon"
    exit 1
fi

# Create app bundle
echo "📱 Creating app bundle..."
APP_BUNDLE="${APP_NAME}.app"
mkdir -p "$APP_BUNDLE/Contents/MacOS"
mkdir -p "$APP_BUNDLE/Contents/Resources"

# Copy binary
cp rust_cat_universal "$APP_BUNDLE/Contents/MacOS/rust_cat"

# Update and copy Info.plist with current version
echo "📝 Updating Info.plist with version $VERSION..."
sed "s/<string>2\.2\.0<\/string>/<string>$VERSION<\/string>/g" Info.plist > "$APP_BUNDLE/Contents/Info.plist"

# Copy app icon
cp RustCat.icns "$APP_BUNDLE/Contents/Resources/AppIcon.icns"
echo "✅ App icon added"

# Make binary executable
chmod +x "$APP_BUNDLE/Contents/MacOS/rust_cat"

# Create ZIP archive
echo "📦 Creating ZIP archive..."
zip -r "${APP_NAME}-universal.app.zip" "$APP_BUNDLE"

# Create DMG
echo "💿 Creating DMG..."
DMG_NAME="${APP_NAME}-universal.dmg"

# Create a temporary directory for DMG contents
TEMP_DIR=$(mktemp -d)
DMG_DIR="$TEMP_DIR/dmg_contents"
mkdir -p "$DMG_DIR"

# Copy the app bundle to the DMG directory
cp -R "$APP_BUNDLE" "$DMG_DIR/"

# Create a symbolic link to Applications folder for easy installation
ln -s /Applications "$DMG_DIR/Applications"

# Create the DMG
hdiutil create -volname "RustCat" -srcfolder "$DMG_DIR" -ov -format UDZO "$DMG_NAME"

# Clean up temporary directory
rm -rf "$TEMP_DIR"

echo "✅ DMG created: $DMG_NAME"

# Clean up temporary files
echo "🧹 Cleaning up..."
rm -f rust_cat_universal
rm -f RustCat.icns
rm -rf RustCat.iconset

echo "✅ macOS build complete\!"
echo ""
echo "📦 Created files:"
echo "   - ${APP_NAME}-universal.app.zip (Universal app bundle)"
echo "   - ${APP_NAME}-universal.dmg (DMG installer)"
echo ""
echo "🔍 Universal binary info:"
lipo -info "$APP_BUNDLE/Contents/MacOS/rust_cat"
