#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
SOURCE_DIR=${SEERR_SOURCE_DIR:-/home/bear/TRUTH/seerr}
BUILD_DIR=${SEERR_BUILD_DIR:-"${ROOT}/.cache/seerr-m2"}
IMAGE=${SEERR_IMAGE:-rawrz-seerr:m2}
SOURCE_TAG=${SEERR_SOURCE_TAG:-v3.4.1}

if [[ ! -f "${SOURCE_DIR}/package.json" ]] || ! git -C "${SOURCE_DIR}" rev-parse --verify "${SOURCE_TAG}^{commit}" >/dev/null 2>&1; then
  echo "Pinned Seerr source tag not found: ${SOURCE_DIR} ${SOURCE_TAG}" >&2
  exit 1
fi

rm -rf "${BUILD_DIR}"
mkdir -p "${BUILD_DIR}"
git -C "${SOURCE_DIR}" archive "${SOURCE_TAG}" | tar -xf - -C "${BUILD_DIR}"
patch -d "${BUILD_DIR}" -p1 < "${ROOT}/services/seerr/patches/rawrz-redis-cache.patch"
docker build --tag "${IMAGE}" --build-arg COMMIT_TAG=m2 -f "${BUILD_DIR}/Dockerfile" "${BUILD_DIR}"
