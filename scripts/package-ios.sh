#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist/ios"
HEADERS_DIR="${DIST_DIR}/headers"
PROFILE="${PROFILE:-release}"
FEATURES="${FEATURES:-ios}"
FRAMEWORK_NAME="${FRAMEWORK_NAME:-ZenoUI}"
LIB_FILE="libzeno_ui.a"
TARGETS_CSV="${ZENO_IOS_TARGETS:-aarch64-apple-ios,aarch64-apple-ios-sim,x86_64-apple-ios}"

if ! command -v xcodebuild >/dev/null 2>&1; then
  echo "xcodebuild is required" >&2
  exit 1
fi

build_args=()
if [[ "${PROFILE}" == "release" ]]; then
  build_args+=(--release)
else
  build_args+=(--profile "${PROFILE}")
fi

mkdir -p "${DIST_DIR}"
rm -rf "${DIST_DIR:?}/${FRAMEWORK_NAME}.xcframework"
mkdir -p "${HEADERS_DIR}"

cat > "${HEADERS_DIR}/zeno_ui.h" <<'EOF'
#pragma once
EOF

library_args=()
available_targets=()
IFS=',' read -r -a targets <<< "${TARGETS_CSV}"
for target in "${targets[@]}"; do
  if ! rustup target list --installed | grep -qx "${target}"; then
    continue
  fi
  cargo build -p zeno-ui --lib --target "${target}" --features "${FEATURES}" "${build_args[@]}"
  lib_path="${ROOT_DIR}/target/${target}/${PROFILE}/${LIB_FILE}"
  if [[ -f "${lib_path}" ]]; then
    library_args+=(-library "${lib_path}" -headers "${HEADERS_DIR}")
    available_targets+=("${target}")
  fi
done

if [[ "${#library_args[@]}" -eq 0 ]]; then
  echo "no ios target available; install at least one rust target or set ZENO_IOS_TARGETS" >&2
  exit 1
fi

xcodebuild -create-xcframework "${library_args[@]}" -output "${DIST_DIR}/${FRAMEWORK_NAME}.xcframework"
printf "framework=%s\nfeatures=%s\nprofile=%s\ntargets=%s\n" "${FRAMEWORK_NAME}" "${FEATURES}" "${PROFILE}" "${available_targets[*]}" > "${DIST_DIR}/package-info.txt"
echo "${DIST_DIR}/${FRAMEWORK_NAME}.xcframework"
