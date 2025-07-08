#!/bin/bash

# Script to create a DMG image for macOS app distribution
# Usage: ./create_dmg.sh <app_bundle_path> <dmg_name>

APP_PATH="$1"
DMG_NAME="$2"

if [ -z "$APP_PATH" ] || [ -z "$DMG_NAME" ]; then
    echo "Usage: $0 <app_bundle_path> <dmg_name>"
    exit 1
fi

if [ ! -d "$APP_PATH" ]; then
    echo "Error: App bundle not found at $APP_PATH"
    exit 1
fi

# Create a temporary directory for DMG contents
TEMP_DIR=$(mktemp -d)
DMG_DIR="$TEMP_DIR/dmg_contents"
mkdir -p "$DMG_DIR"

# Copy the app bundle to the DMG directory
cp -R "$APP_PATH" "$DMG_DIR/"

# Create a symbolic link to Applications folder for easy installation
ln -s /Applications "$DMG_DIR/Applications"

# Create the DMG
hdiutil create -volname "RustCat" -srcfolder "$DMG_DIR" -ov -format UDZO "$DMG_NAME"

# Clean up
rm -rf "$TEMP_DIR"

echo "DMG created: $DMG_NAME"
