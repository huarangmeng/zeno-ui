#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist/windows"
PROFILE="${PROFILE:-release}"
FEATURES="${FEATURES:-windows}"
PACKAGE_NAME="${PACKAGE_NAME:-zeno-ui-windows}"
TARGETS_CSV="${ZENO_WINDOWS_TARGETS:-}"
HOST_TARGET="$(rustc -vV | awk '/host: / { print $2 }')"

build_args=()
if [[ "${PROFILE}" == "release" ]]; then
  build_args+=(--release)
else
  build_args+=(--profile "${PROFILE}")
fi

targets=()
if [[ -n "${TARGETS_CSV}" ]]; then
  IFS=',' read -r -a targets <<< "${TARGETS_CSV}"
elif [[ "${HOST_TARGET}" == *"pc-windows"* ]]; then
  targets=("${HOST_TARGET}")
else
  targets=("x86_64-pc-windows-msvc")
fi

mkdir -p "${DIST_DIR}"

copy_if_exists() {
  local source="$1"
  local destination="$2"
  if [[ -f "${source}" ]]; then
    cp "${source}" "${destination}"
  fi
}

available_targets=()
for target in "${targets[@]}"; do
  if ! rustup target list --installed | grep -qx "${target}"; then
    continue
  fi
  cargo build -p zeno-ui --lib --target "${target}" --features "${FEATURES}" "${build_args[@]}"
  target_out="${DIST_DIR}/${target}"
  mkdir -p "${target_out}"
  copy_if_exists "${ROOT_DIR}/target/${target}/${PROFILE}/zeno_ui.dll" "${target_out}/zeno_ui.dll"
  copy_if_exists "${ROOT_DIR}/target/${target}/${PROFILE}/zeno_ui.dll.lib" "${target_out}/zeno_ui.dll.lib"
  copy_if_exists "${ROOT_DIR}/target/${target}/${PROFILE}/zeno_ui.lib" "${target_out}/zeno_ui.lib"
  copy_if_exists "${ROOT_DIR}/target/${target}/${PROFILE}/libzeno_ui.a" "${target_out}/libzeno_ui.a"
  printf "package=%s\ntarget=%s\nfeatures=%s\nprofile=%s\n" "${PACKAGE_NAME}" "${target}" "${FEATURES}" "${PROFILE}" > "${target_out}/package-info.txt"
  available_targets+=("${target}")
done

if [[ "${#available_targets[@]}" -eq 0 ]]; then
  echo "no windows target available; install at least one rust target or set ZENO_WINDOWS_TARGETS" >&2
  exit 1
fi

echo "${DIST_DIR}"
