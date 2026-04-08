#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

"${ROOT_DIR}/scripts/package-desktop.sh"

if [[ "${ZENO_PACKAGE_LINUX:-0}" == "1" ]]; then
  "${ROOT_DIR}/scripts/package-linux.sh"
fi

if [[ "${ZENO_PACKAGE_WINDOWS:-0}" == "1" ]]; then
  "${ROOT_DIR}/scripts/package-windows.sh"
fi

if [[ "$(uname -s)" == "Darwin" ]] && command -v xcodebuild >/dev/null 2>&1; then
  "${ROOT_DIR}/scripts/package-ios.sh"
fi

if command -v cargo-ndk >/dev/null 2>&1 && [[ -n "${ANDROID_NDK_HOME:-}" ]] && command -v jar >/dev/null 2>&1 && command -v zip >/dev/null 2>&1; then
  "${ROOT_DIR}/scripts/package-android.sh"
fi
