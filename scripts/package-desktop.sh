#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist/desktop"
PROFILE="${PROFILE:-release}"
FEATURES="${FEATURES:-macos}"
PACKAGE_NAME="${PACKAGE_NAME:-zeno-ui-desktop}"
TARGETS_CSV="${ZENO_DESKTOP_TARGETS:-}"
HOST_TARGET="$(rustc -vV | awk '/host: / { print $2 }')"
LIB_PREFIX="libzeno_ui"

build_args=()
if [[ "${PROFILE}" == "release" ]]; then
  build_args+=(--release)
else
  build_args+=(--profile "${PROFILE}")
fi

targets=()
if [[ -n "${TARGETS_CSV}" ]]; then
  IFS=',' read -r -a targets <<< "${TARGETS_CSV}"
elif [[ "$(uname -s)" == "Darwin" ]]; then
  targets=("aarch64-apple-darwin" "x86_64-apple-darwin")
else
  targets=("${HOST_TARGET}")
fi

mkdir -p "${DIST_DIR}"

available_targets=()
for target in "${targets[@]}"; do
  if ! rustup target list --installed | grep -qx "${target}"; then
    continue
  fi
  cargo build -p zeno-ui --lib --target "${target}" --features "${FEATURES}" "${build_args[@]}"
  target_out="${DIST_DIR}/${target}"
  mkdir -p "${target_out}"
  cp "${ROOT_DIR}/target/${target}/${PROFILE}/${LIB_PREFIX}.a" "${target_out}/${LIB_PREFIX}.a"
  if [[ -f "${ROOT_DIR}/target/${target}/${PROFILE}/${LIB_PREFIX}.dylib" ]]; then
    cp "${ROOT_DIR}/target/${target}/${PROFILE}/${LIB_PREFIX}.dylib" "${target_out}/${LIB_PREFIX}.dylib"
  fi
  printf "target=%s\nfeatures=%s\nprofile=%s\n" "${target}" "${FEATURES}" "${PROFILE}" > "${target_out}/package-info.txt"
  available_targets+=("${target}")
done

if [[ "${#available_targets[@]}" -eq 0 ]]; then
  echo "no desktop target available; install at least one rust target or set ZENO_DESKTOP_TARGETS" >&2
  exit 1
fi

if [[ "$(uname -s)" == "Darwin" ]] && command -v lipo >/dev/null 2>&1; then
  has_arm64=0
  has_x64=0
  for target in "${available_targets[@]}"; do
    [[ "${target}" == "aarch64-apple-darwin" ]] && has_arm64=1
    [[ "${target}" == "x86_64-apple-darwin" ]] && has_x64=1
  done
  if [[ "${has_arm64}" -eq 1 && "${has_x64}" -eq 1 ]]; then
    universal_dir="${DIST_DIR}/universal-macos"
    mkdir -p "${universal_dir}"
    lipo -create \
      "${DIST_DIR}/aarch64-apple-darwin/${LIB_PREFIX}.a" \
      "${DIST_DIR}/x86_64-apple-darwin/${LIB_PREFIX}.a" \
      -output "${universal_dir}/${LIB_PREFIX}.a"
    if [[ -f "${DIST_DIR}/aarch64-apple-darwin/${LIB_PREFIX}.dylib" && -f "${DIST_DIR}/x86_64-apple-darwin/${LIB_PREFIX}.dylib" ]]; then
      lipo -create \
        "${DIST_DIR}/aarch64-apple-darwin/${LIB_PREFIX}.dylib" \
        "${DIST_DIR}/x86_64-apple-darwin/${LIB_PREFIX}.dylib" \
        -output "${universal_dir}/${LIB_PREFIX}.dylib"
    fi
    printf "package=%s\nfeatures=%s\nprofile=%s\n" "${PACKAGE_NAME}" "${FEATURES}" "${PROFILE}" > "${universal_dir}/package-info.txt"
  fi
fi

echo "${DIST_DIR}"
