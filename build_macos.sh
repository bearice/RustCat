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
VERSION=$(grep '^version' Cargo.toml | sed 's/version = "\(.*\)"/\1/')

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
./build_app_icon.sh || echo "⚠️  Could not create app icon, continuing without it"

# Create app bundle
echo "📱 Creating app bundle..."
APP_BUNDLE="${APP_NAME}.app"
mkdir -p "$APP_BUNDLE/Contents/MacOS"
mkdir -p "$APP_BUNDLE/Contents/Resources"

# Copy binary
cp rust_cat_universal "$APP_BUNDLE/Contents/MacOS/rust_cat"

# Copy Info.plist
cp Info.plist "$APP_BUNDLE/Contents/"

# Copy app icon if it exists
if [ -f "RustCat.icns" ]; then
    cp RustCat.icns "$APP_BUNDLE/Contents/Resources/AppIcon.icns"
    echo "✅ App icon added"
else
    echo "⚠️  No app icon found"
fi

# Make binary executable
chmod +x "$APP_BUNDLE/Contents/MacOS/rust_cat"

# Create ZIP archive
echo "📦 Creating ZIP archive..."
zip -r "${APP_NAME}-universal.app.zip" "$APP_BUNDLE"

# Create DMG
echo "💿 Creating DMG..."
./create_dmg.sh "$APP_BUNDLE" "${APP_NAME}-universal.dmg"

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
