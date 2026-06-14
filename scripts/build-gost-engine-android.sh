#!/usr/bin/env bash
# build-gost-engine-android.sh — собирает GOST-провайдер (gost-engine) для
# OpenSSL 3.x как статическую библиотеку для Android.
#
# Результат: third_party/gost-engine/install/android-<abi>/lib/libgostprov.a
#            third_party/gost-engine/install/android-<abi>/include/gost_provider_init.h
#
# Использование:
#   ./scripts/build-gost-engine-android.sh          # arm64 (по умолчанию)
#   ./scripts/build-gost-engine-android.sh arm64
#
# Переменные окружения:
#   ANDROID_NDK_ROOT   путь до NDK
#   OPENSSL_ROOT       путь до собранного OpenSSL (third_party/openssl/install/android-arm64)
#   MIN_SDK            минимальный Android API (по умолчанию 26)
#   JOBS               кол-во потоков make

set -euo pipefail

MIN_SDK="${MIN_SDK:-26}"
JOBS="${JOBS:-$(getconf _NPROCESSORS_ONLN 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
THIRD_PARTY="${REPO_DIR}/third_party"
SRC_DIR="${THIRD_PARTY}/gost-engine-src"
INSTALL_ROOT="${THIRD_PARTY}/gost-engine/install"

mkdir -p "${THIRD_PARTY}" "${INSTALL_ROOT}"

# ---------- NDK --------------------------------------------------------------
if [ -z "${ANDROID_NDK_ROOT:-}" ]; then
  if [ -n "${ANDROID_NDK_HOME:-}" ]; then
    ANDROID_NDK_ROOT="${ANDROID_NDK_HOME}"
  elif [ -n "${ANDROID_HOME:-}" ] && [ -d "${ANDROID_HOME}/ndk" ]; then
    ANDROID_NDK_ROOT="$(ls -d "${ANDROID_HOME}/ndk/"*/ 2>/dev/null | sort -V | tail -n1)"
    ANDROID_NDK_ROOT="${ANDROID_NDK_ROOT%/}"
  fi
fi
if [ -z "${ANDROID_NDK_ROOT:-}" ] || [ ! -d "${ANDROID_NDK_ROOT}" ]; then
  echo "ERROR: ANDROID_NDK_ROOT is not set or invalid" >&2
  exit 1
fi
export ANDROID_NDK_ROOT
echo "Using NDK: ${ANDROID_NDK_ROOT}"

# ---------- Host tag ---------------------------------------------------------
case "$(uname -s)" in
  Linux*)   HOST_TAG="linux-x86_64" ;;
  Darwin*)  HOST_TAG="darwin-x86_64" ;;
  *)        echo "ERROR: unsupported host OS" >&2; exit 1 ;;
esac
TOOLCHAIN="${ANDROID_NDK_ROOT}/toolchains/llvm/prebuilt/${HOST_TAG}"

# ---------- Исходники gost-engine -------------------------------------------
GOST_BRANCH="openssl_3.0"
if [ ! -d "${SRC_DIR}" ]; then
  echo "Cloning gost-engine (${GOST_BRANCH}) ..."
  git clone --depth 1 --branch "${GOST_BRANCH}" \
    https://github.com/gost-engine/engine.git "${SRC_DIR}"
else
  echo "Using existing gost-engine source at ${SRC_DIR}"
fi

# ---------- Сборка -----------------------------------------------------------
build_one() {
  local abi="$1"
  local target_triple=""
  local openssl_prefix=""

  case "${abi}" in
    arm64)
      target_triple="aarch64-linux-android"
      openssl_prefix="${OPENSSL_ROOT:-${THIRD_PARTY}/openssl/install/android-arm64}"
      ;;
    x86_64)
      target_triple="x86_64-linux-android"
      openssl_prefix="${OPENSSL_ROOT:-${THIRD_PARTY}/openssl/install/android-x86_64}"
      ;;
    *)
      echo "ERROR: unsupported ABI '${abi}'" >&2; exit 1 ;;
  esac

  local prefix="${INSTALL_ROOT}/android-${abi}"

  if [ -f "${prefix}/lib/libgost_core.a" ]; then
    echo "── [${abi}] already built, skipping"
    return 0
  fi

  echo "── [${abi}] building gost-engine → ${prefix}"

  local build_dir="${THIRD_PARTY}/gost-engine-build-${abi}"
  rm -rf "${build_dir}"
  mkdir -p "${build_dir}" "${prefix}/lib" "${prefix}/include"

  local CC="${TOOLCHAIN}/bin/${target_triple}${MIN_SDK}-clang"
  local CXX="${TOOLCHAIN}/bin/${target_triple}${MIN_SDK}-clang++"
  local AR="${TOOLCHAIN}/bin/llvm-ar"
  local RANLIB="${TOOLCHAIN}/bin/llvm-ranlib"

  cmake -S "${SRC_DIR}" -B "${build_dir}" \
    -DCMAKE_SYSTEM_NAME=Android \
    -DCMAKE_ANDROID_NDK="${ANDROID_NDK_ROOT}" \
    -DCMAKE_ANDROID_ARCH_ABI="$([ "${abi}" = "arm64" ] && echo "arm64-v8a" || echo "x86_64")" \
    -DCMAKE_ANDROID_NDK_TOOLCHAIN_VERSION=clang \
    -DCMAKE_SYSTEM_VERSION="${MIN_SDK}" \
    -DCMAKE_C_COMPILER="${CC}" \
    -DCMAKE_CXX_COMPILER="${CXX}" \
    -DCMAKE_AR="${AR}" \
    -DCMAKE_RANLIB="${RANLIB}" \
    -DCMAKE_BUILD_TYPE=Release \
    -DOPENSSL_ROOT_DIR="${openssl_prefix}" \
    -DOPENSSL_INCLUDE_DIR="${openssl_prefix}/include" \
    -DOPENSSL_CRYPTO_LIBRARY="${openssl_prefix}/lib/libcrypto.a" \
    -DBUILD_SHARED_LIBS=OFF \
    -DCMAKE_INSTALL_PREFIX="${prefix}" \
    -DCMAKE_POSITION_INDEPENDENT_CODE=ON

  cmake --build "${build_dir}" -j "${JOBS}" --target gost_core 2>/dev/null || \
  cmake --build "${build_dir}" -j "${JOBS}" --target gost_prov 2>/dev/null || \
  cmake --build "${build_dir}" -j "${JOBS}" 2>&1 | tail -30

  # Копируем нужные артефакты (.a и .o)
  find "${build_dir}" -name "*.a" -exec cp {} "${prefix}/lib/" \;
  
  # Если .a не нашлись — попробуем собрать из .o
  if ! ls "${prefix}/lib/"*.a 1>/dev/null 2>&1; then
    echo "── [${abi}] no .a found, trying to build from .o files"
    find "${build_dir}" -name "*.o" | head -100 | xargs "${AR}" rcs "${prefix}/lib/libgost_prov.a" 2>/dev/null || true
  fi

  # Ищем правильное имя init-функции в собранных объектах
  local INIT_FUNC="ossl_gost_provider_init"
  if command -v nm >/dev/null 2>&1; then
    local found_func
    found_func=$(find "${prefix}/lib" -name "*.a" -exec nm {} \; 2>/dev/null | grep -o '[A-Za-z_]*provider_init' | head -1 || true)
    if [ -n "${found_func}" ]; then
      INIT_FUNC="${found_func}"
    fi
  fi

  # Создаём заголовок для вызова init
  cat > "${prefix}/include/gost_provider_init.h" <<EOF
#ifndef GOST_PROVIDER_INIT_H
#define GOST_PROVIDER_INIT_H
#include <openssl/core.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Entry point for gost-engine provider (OpenSSL 3.x provider init function).
 * Used with OSSL_PROVIDER_add_builtin() for static linking.
 * Detected function name: ${INIT_FUNC} */
int ${INIT_FUNC}(const OSSL_CORE_HANDLE *handle,
                 const OSSL_DISPATCH *in,
                 const OSSL_DISPATCH **out,
                 void **provctx);

/* Alias for code that uses the canonical name */
#define ossl_gost_provider_init ${INIT_FUNC}

#ifdef __cplusplus
}
#endif

#endif /* GOST_PROVIDER_INIT_H */
EOF

  echo "── [${abi}] done"
  ls -la "${prefix}/lib/"*.a 2>/dev/null || true
}

# ---------- Запуск -----------------------------------------------------------
ABI="${1:-arm64}"
build_one "${ABI}"

echo
echo "GOST engine build complete."
find "${INSTALL_ROOT}" -name "*.a" -print | sort
