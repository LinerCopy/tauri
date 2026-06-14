#!/usr/bin/env bash
# build-gost-engine-android.sh — собирает gost-engine (provider + engine)
# для OpenSSL 3.x как статические библиотеки для Android.
#
# Особенности:
# 1. Сборка через CMake с принудительными путями к Android OpenSSL.
# 2. Переименование символа `OSSL_provider_init` → `gost_provider_init`
#    через `llvm-objcopy --redefine-sym`, чтобы избежать конфликта при
#    статической линковке нескольких провайдеров.
# 3. Объединение всех .o в один большой архив libgost_provider_static.a.
#
# Результат:
#   third_party/gost-engine/install/android-<abi>/
#     ├── lib/libgost_provider_static.a   (provider, переименованный init)
#     └── include/gost_provider_init.h    (declaration)
#
# Использование:
#   ./scripts/build-gost-engine-android.sh             # arm64 по умолчанию
#   ./scripts/build-gost-engine-android.sh arm64
#
# Переменные окружения:
#   ANDROID_NDK_ROOT   путь до NDK r26+
#   OPENSSL_ROOT       путь до собранного OpenSSL (third_party/openssl/install/android-arm64)
#   MIN_SDK            минимальный Android API (по умолчанию 26)
#   GOST_BRANCH        ветка gost-engine (по умолчанию openssl_3_0)
#   JOBS               параллелизм make

set -euo pipefail

MIN_SDK="${MIN_SDK:-26}"
JOBS="${JOBS:-$(getconf _NPROCESSORS_ONLN 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)}"
GOST_BRANCH="${GOST_BRANCH:-master}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
THIRD_PARTY="${REPO_DIR}/third_party"
SRC_DIR="${THIRD_PARTY}/gost-engine-src"
INSTALL_ROOT="${THIRD_PARTY}/gost-engine/install"

mkdir -p "${THIRD_PARTY}" "${INSTALL_ROOT}"

# ---------- NDK ----------
if [ -z "${ANDROID_NDK_ROOT:-}" ]; then
  if [ -n "${ANDROID_NDK_HOME:-}" ]; then
    ANDROID_NDK_ROOT="${ANDROID_NDK_HOME}"
  elif [ -n "${ANDROID_HOME:-}" ] && [ -d "${ANDROID_HOME}/ndk" ]; then
    ANDROID_NDK_ROOT="$(ls -d "${ANDROID_HOME}/ndk/"*/ 2>/dev/null | sort -V | tail -n1)"
    ANDROID_NDK_ROOT="${ANDROID_NDK_ROOT%/}"
  fi
fi
[ -d "${ANDROID_NDK_ROOT:-}" ] || { echo "ERROR: ANDROID_NDK_ROOT not set"; exit 1; }
export ANDROID_NDK_ROOT
echo "Using NDK: ${ANDROID_NDK_ROOT}"

case "$(uname -s)" in
  Linux*)   HOST_TAG="linux-x86_64" ;;
  Darwin*)  HOST_TAG="darwin-x86_64" ;;
  *)        echo "ERROR: unsupported host OS"; exit 1 ;;
esac
TOOLCHAIN="${ANDROID_NDK_ROOT}/toolchains/llvm/prebuilt/${HOST_TAG}"
[ -d "${TOOLCHAIN}" ] || { echo "ERROR: toolchain not found: ${TOOLCHAIN}"; exit 1; }

# ---------- Источники gost-engine ----------
if [ ! -d "${SRC_DIR}" ]; then
  echo "Cloning gost-engine (${GOST_BRANCH}) ..."
  # --recurse-submodules is required: libprov is a submodule in master branch
  git clone --depth 1 --recurse-submodules --shallow-submodules \
    --branch "${GOST_BRANCH}" \
    https://github.com/gost-engine/engine.git "${SRC_DIR}" || {
      echo "WARN: branch '${GOST_BRANCH}' not found, falling back to master"
      git clone --depth 1 --recurse-submodules --shallow-submodules \
        https://github.com/gost-engine/engine.git "${SRC_DIR}"
    }
else
  # Ensure submodules are initialised even if clone was done without them
  git -C "${SRC_DIR}" submodule update --init --recursive --depth 1 2>/dev/null || true
fi

# ---------- Сборка одной ABI ----------
build_one() {
  local abi="$1"
  local target_triple=""
  local cmake_abi=""
  local openssl_prefix=""

  case "${abi}" in
    arm64)
      target_triple="aarch64-linux-android"
      cmake_abi="arm64-v8a"
      openssl_prefix="${OPENSSL_ROOT:-${THIRD_PARTY}/openssl/install/android-arm64}"
      ;;
    x86_64)
      target_triple="x86_64-linux-android"
      cmake_abi="x86_64"
      openssl_prefix="${OPENSSL_ROOT:-${THIRD_PARTY}/openssl/install/android-x86_64}"
      ;;
    *)
      echo "ERROR: unsupported ABI '${abi}'"; exit 1 ;;
  esac

  local prefix="${INSTALL_ROOT}/android-${abi}"
  local build_dir="${THIRD_PARTY}/gost-engine-build-${abi}"

  # Проверяем зависимости
  [ -f "${openssl_prefix}/lib/libcrypto.a" ] || {
    echo "ERROR: OpenSSL not built at ${openssl_prefix}"
    echo "Run scripts/build-openssl-android.sh ${abi} first"
    exit 1
  }

  # Кеш: уже собран?
  if [ -f "${prefix}/lib/libgost_provider_static.a" ]; then
    echo "── [${abi}] already built, skipping"
    return 0
  fi

  echo "── [${abi}] building gost-engine for OpenSSL @ ${openssl_prefix}"
  rm -rf "${build_dir}"
  mkdir -p "${build_dir}" "${prefix}/lib" "${prefix}/include"

  local CC="${TOOLCHAIN}/bin/${target_triple}${MIN_SDK}-clang"
  local CXX="${TOOLCHAIN}/bin/${target_triple}${MIN_SDK}-clang++"
  local AR="${TOOLCHAIN}/bin/llvm-ar"
  local RANLIB="${TOOLCHAIN}/bin/llvm-ranlib"
  local OBJCOPY="${TOOLCHAIN}/bin/llvm-objcopy"
  local NM="${TOOLCHAIN}/bin/llvm-nm"

  # CMake configure
  cmake -S "${SRC_DIR}" -B "${build_dir}" \
    -DCMAKE_SYSTEM_NAME=Android \
    -DCMAKE_ANDROID_NDK="${ANDROID_NDK_ROOT}" \
    -DCMAKE_ANDROID_ARCH_ABI="${cmake_abi}" \
    -DCMAKE_SYSTEM_VERSION="${MIN_SDK}" \
    -DCMAKE_ANDROID_STL_TYPE=c++_static \
    -DCMAKE_C_COMPILER="${CC}" \
    -DCMAKE_CXX_COMPILER="${CXX}" \
    -DCMAKE_AR="${AR}" \
    -DCMAKE_RANLIB="${RANLIB}" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
    -DOPENSSL_ROOT_DIR="${openssl_prefix}" \
    -DOPENSSL_INCLUDE_DIR="${openssl_prefix}/include" \
    -DOPENSSL_CRYPTO_LIBRARY="${openssl_prefix}/lib/libcrypto.a" \
    -DOPENSSL_SSL_LIBRARY="${openssl_prefix}/lib/libssl.a" \
    -DOPENSSL_USE_STATIC_LIBS=ON \
    -DBUILD_SHARED_LIBS=OFF \
    -DSKIP_PERL_TESTS=ON \
    -DENABLE_PROGRAMS=OFF \
    -DCMAKE_INSTALL_PREFIX="${prefix}" \
    -DRELAXED_ALIGNMENT_EXITCODE=0 \
    -DRELAXED_ALIGNMENT_EXITCODE__TRYRUN_OUTPUT="" \
    -DADDCARRY_U64_EXITCODE=1 \
    -DADDCARRY_U64_EXITCODE__TRYRUN_OUTPUT="" \
    -Wno-dev \
    2>&1 | tee "${build_dir}/cmake_configure.log"

  # Билдим всё что получится (provider + engine)
  echo "── [${abi}] cmake --build (this may take a while) ..."
  cmake --build "${build_dir}" -j "${JOBS}" 2>&1 | tee "${build_dir}/cmake_build.log" | tail -50 || {
    echo "WARN: full build had errors; will try to salvage .o files"
  }

  # ── Собираем все .o из CMakeFiles ──
  local objs_list="${build_dir}/all_objs.txt"
  find "${build_dir}/CMakeFiles" -name '*.o' \
    ! -path '*/test*' \
    ! -path '*/CMakeTmp*' \
    ! -name 'test_*' \
    > "${objs_list}" 2>/dev/null || true

  local total_objs
  total_objs=$(wc -l < "${objs_list}" | tr -d ' ')
  echo "── [${abi}] found ${total_objs} object files"

  if [ "${total_objs}" -lt 5 ]; then
    echo "ERROR: too few object files (${total_objs}); build likely failed"
    echo "── cmake configure log (last 40 lines) ──"
    tail -40 "${build_dir}/cmake_configure.log" 2>/dev/null || true
    echo "── cmake build log (last 60 lines) ──"
    tail -60 "${build_dir}/cmake_build.log" 2>/dev/null || true
    return 1
  fi

  # ── Переименовываем символ OSSL_provider_init → gost_provider_init ──
  # Нужно чтобы избежать конфликта при статической линковке с другими
  # провайдерами OpenSSL (default, legacy).
  echo "── [${abi}] renaming OSSL_provider_init → gost_provider_init"
  local renamed=0
  while IFS= read -r objfile; do
    [ -f "${objfile}" ] || continue
    if "${NM}" "${objfile}" 2>/dev/null | grep -q " T OSSL_provider_init"; then
      "${OBJCOPY}" --redefine-sym OSSL_provider_init=gost_provider_init "${objfile}"
      echo "    renamed in: $(basename "${objfile}")"
      renamed=$((renamed + 1))
    fi
  done < "${objs_list}"

  if [ "${renamed}" -eq 0 ]; then
    echo "WARN: no OSSL_provider_init found in any .o file"
    echo "Searching for any *provider_init* symbols:"
    while IFS= read -r objfile; do
      "${NM}" "${objfile}" 2>/dev/null | grep -i "provider_init" || true
    done < "${objs_list}" | sort -u | head -20
  fi

  # ── Создаём финальный архив ──
  local final_lib="${prefix}/lib/libgost_provider_static.a"
  rm -f "${final_lib}"
  echo "── [${abi}] creating archive ${final_lib}"

  # ar @file syntax — читает имена .o из файла
  if ! "${AR}" rcs "${final_lib}" @"${objs_list}" 2>/dev/null; then
    echo "    ar @file failed, using xargs"
    < "${objs_list}" xargs "${AR}" rcs "${final_lib}"
  fi
  "${RANLIB}" "${final_lib}" || true

  # Заголовок
  cat > "${prefix}/include/gost_provider_init.h" <<'EOF'
/* Auto-generated by build-gost-engine-android.sh */
#ifndef GOST_PROVIDER_INIT_H
#define GOST_PROVIDER_INIT_H

#include <openssl/core.h>

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Entry point for the statically-linked gost-engine provider.
 *
 * Symbol was renamed at build time from `OSSL_provider_init` to
 * `gost_provider_init` via `llvm-objcopy --redefine-sym` to avoid clashes
 * with other statically-linked OpenSSL providers (default, legacy).
 *
 * Used with OSSL_PROVIDER_add_builtin(libctx, "gost", gost_provider_init).
 */
int gost_provider_init(const OSSL_CORE_HANDLE *handle,
                       const OSSL_DISPATCH *in,
                       const OSSL_DISPATCH **out,
                       void **provctx);

#ifdef __cplusplus
}
#endif

#endif /* GOST_PROVIDER_INIT_H */
EOF

  # ── Проверка ──
  echo "── [${abi}] verifying archive"
  if "${NM}" "${final_lib}" 2>/dev/null | grep -q " T gost_provider_init"; then
    echo "    ✓ gost_provider_init found"
  else
    echo "    ✗ WARN: gost_provider_init NOT found in archive"
    echo "    Available T symbols (first 30):"
    "${NM}" "${final_lib}" 2>/dev/null | grep " T " | head -30 || true
  fi

  ls -la "${prefix}/lib/" || true
  echo "── [${abi}] done"
}

# ---------- Main ----------
ABI="${1:-arm64}"
build_one "${ABI}"

echo
echo "=== GOST engine build complete ==="
find "${INSTALL_ROOT}" -name "*.a" -print
