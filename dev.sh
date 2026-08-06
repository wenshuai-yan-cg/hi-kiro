#!/usr/bin/env zsh
# kiro-history launcher — sets WSLg/Wayland IME environment before starting

# Force Wayland backend for proper IME support via WSLg
export GDK_BACKEND=wayland

# Disable GPU compositing (fixes MESA/ZINK errors on WSL2)
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1

# Disable broken IM modules
unset GTK_IM_MODULE

# Start the app
cd "$(dirname "$0")/kiro-history"
npm run tauri dev
