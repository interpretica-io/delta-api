#!/usr/bin/env bash
# Copyright (C) 2025-2026 Interpretica Unipessoal Lda
# SPDX-License-Identifier: Apache-2.0
#
# Build the Delta API network server (delta-server) from the delta-api crate
# and place the resulting binary at the requested location.
#
# Usage: build_delta_server.sh <cargo> <delta-api-src> <output>
#
#   <cargo>           Path to the cargo executable.
#   <delta-api-src>   Fallback path to the delta-api crate (the directory
#                     that contains Cargo.toml). Overridden by the
#                     DELTA_API_SRC environment variable when it is set.
#   <output>          Path the delta-server binary must be copied to.

set -e

CARGO="$1"
SRC="${DELTA_API_SRC:-$2}"
OUT="$3"

if [ -z "${CARGO}" ] || [ -z "${SRC}" ] || [ -z "${OUT}" ] ; then
    echo "build_delta_server.sh: missing argument" >&2
    echo "Usage: $0 <cargo> <delta-api-src> <output>" >&2
    exit 1
fi

if [ ! -f "${SRC}/Cargo.toml" ] ; then
    echo "build_delta_server.sh: no Cargo.toml under '${SRC}'" >&2
    echo "Set DELTA_API_SRC to the delta-api crate directory." >&2
    exit 1
fi

SRC="$(cd "${SRC}" && pwd -P)"
OUT_DIR="$(cd "$(dirname "${OUT}")" && pwd -P)"
OUT_BASE="$(basename "${OUT}")"
TARGET_DIR="${OUT_DIR}/cargo-target"

echo "build_delta_server.sh: building delta-server from ${SRC}"

"${CARGO}" build \
    --release \
    --no-default-features \
    --features server \
    --bin delta-server \
    --manifest-path "${SRC}/Cargo.toml" \
    --target-dir "${TARGET_DIR}"

cp "${TARGET_DIR}/release/delta-server" "${OUT_DIR}/${OUT_BASE}"
chmod +x "${OUT_DIR}/${OUT_BASE}"

echo "build_delta_server.sh: delta-server ready at ${OUT_DIR}/${OUT_BASE}"
