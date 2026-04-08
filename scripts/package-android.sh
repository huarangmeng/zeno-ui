#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist/android"
STAGE_DIR="${DIST_DIR}/aar-stage"
PROFILE="${PROFILE:-release}"
FEATURES="${FEATURES:-android}"
AAR_NAME="${AAR_NAME:-zeno-ui-android.aar}"
ABIS_CSV="${ZENO_ANDROID_ABIS:-arm64-v8a,armeabi-v7a,x86_64}"

if ! command -v cargo-ndk >/dev/null 2>&1; then
  echo "cargo-ndk is required" >&2
  exit 1
fi

if [[ -z "${ANDROID_NDK_HOME:-}" ]]; then
  echo "ANDROID_NDK_HOME is required" >&2
  exit 1
fi

if ! command -v jar >/dev/null 2>&1; then
  echo "jar is required to build classes.jar" >&2
  exit 1
fi

if ! command -v zip >/dev/null 2>&1; then
  echo "zip is required to package the aar" >&2
  exit 1
fi

build_args=()
if [[ "${PROFILE}" == "release" ]]; then
  build_args+=(--release)
else
  build_args+=(--profile "${PROFILE}")
fi

mkdir -p "${DIST_DIR}"
rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}/jni"

ndk_args=()
IFS=',' read -r -a abi_list <<< "${ABIS_CSV}"
for abi in "${abi_list[@]}"; do
  ndk_args+=(-t "${abi}")
done

cargo ndk "${ndk_args[@]}" -o "${STAGE_DIR}/jni" build -p zeno-ui --lib --features "${FEATURES}" "${build_args[@]}"

cat > "${STAGE_DIR}/AndroidManifest.xml" <<'EOF'
<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="dev.zeno.sdk" />
EOF

mkdir -p "${STAGE_DIR}/empty"
jar --create --file "${STAGE_DIR}/classes.jar" -C "${STAGE_DIR}/empty" .
rm -rf "${STAGE_DIR}/empty"

(
  cd "${STAGE_DIR}"
  rm -f "${DIST_DIR}/${AAR_NAME}"
  zip -qr "${DIST_DIR}/${AAR_NAME}" AndroidManifest.xml classes.jar jni
)

printf "aar=%s\nfeatures=%s\nprofile=%s\nabis=%s\n" "${AAR_NAME}" "${FEATURES}" "${PROFILE}" "${ABIS_CSV}" > "${DIST_DIR}/package-info.txt"
echo "${DIST_DIR}/${AAR_NAME}"
