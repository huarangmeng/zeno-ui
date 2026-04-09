#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARTIFACT_DIR="${ZENO_BENCH_ARTIFACT_DIR:-${ROOT_DIR}/artifacts/metrics}"

mkdir -p "${ARTIFACT_DIR}"

cd "${ROOT_DIR}"

ZENO_TEXT_PROBE_FORMAT=json \
ZENO_TEXT_PROBE_OUTPUT="${ARTIFACT_DIR}/text_probe.json" \
ZENO_TEXT_PROBE_MIN_PATCH_RATIO="${ZENO_TEXT_PROBE_MIN_PATCH_RATIO:-0.60}" \
ZENO_TEXT_PROBE_MIN_CACHE_HIT_RATE="${ZENO_TEXT_PROBE_MIN_CACHE_HIT_RATE:-0.45}" \
cargo run -p text_probe --quiet

ZENO_BENCH_GALLERY_FORMAT=json \
ZENO_BENCH_GALLERY_OUTPUT="${ARTIFACT_DIR}/bench_gallery.json" \
ZENO_BENCH_GALLERY_MIN_PATCH_RATIO="${ZENO_BENCH_GALLERY_MIN_PATCH_RATIO:-0.50}" \
ZENO_BENCH_GALLERY_MIN_CACHE_HIT_RATE="${ZENO_BENCH_GALLERY_MIN_CACHE_HIT_RATE:-0.30}" \
cargo run -p bench_gallery --quiet

printf 'bench suite artifacts written to %s\n' "${ARTIFACT_DIR}"
