#!/usr/bin/env bash
# build-openssl-android.sh — собирает статический OpenSSL для всех нужных
# Android ABI с помощью NDK r26+. Используется как локально, так и в CI
# (.github/workflows/android.yml → job `android-cpp`).
#
# Соответствует ТЗ §10:
#   * Android: CMake + NDK r26+
#   * OpenSSL — статическая сборка (libssl.a / libcrypto.a)
#
# Результат: third_party/openssl/install/android-<abi>/{include,lib}
#
# Использование:
#   ./scripts/build-openssl-android.sh                  # все три ABI
#   ./scripts/build-openssl-android.sh arm64            # только arm64-v8a
#   ./scripts/build-openssl-android.sh arm64 x86_64     # выбор подмножества
#
# Переменные окружения:
#   ANDROID_NDK_ROOT   путь до NDK (по умолчанию — $ANDROID_HOME/ndk/<latest>)
#   OPENSSL_VERSION    тег OpenSSL для клонирования (по умолчанию openssl-3.3.0)
#   MIN_SDK            минимальный Android API (по умолчанию 26)
#   JOBS               кол-во параллельных потоков make (по умолчанию nproc)

set -euo pipefail

# ---------- Параметры -------------------------------------------------------
OPENSSL_VERSION="${OPENSSL_VERSION:-openssl-3.3.0}"
MIN_SDK="${MIN_SDK:-26}"
JOBS="${JOBS:-$(getconf _NPROCESSORS_ONLN 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
THIRD_PARTY="${REPO_DIR}/third_party"
SRC_DIR="${THIRD_PARTY}/openssl-src"
INSTALL_ROOT="${THIRD_PARTY}/openssl/install"

mkdir -p "${THIRD_PARTY}" "${INSTALL_ROOT}"

# ---------- Поиск NDK -------------------------------------------------------
if [ -z "${ANDROID_NDK_ROOT:-}" ]; then
  if [ -n "${ANDROID_NDK_HOME:-}" ]; then
    ANDROID_NDK_ROOT="${ANDROID_NDK_HOME}"
  elif [ -n "${NDK_HOME:-}" ]; then
    ANDROID_NDK_ROOT="${NDK_HOME}"
  elif [ -n "${ANDROID_HOME:-}" ] && [ -d "${ANDROID_HOME}/ndk" ]; then
    # Берём самую свежую установленную версию NDK
    ANDROID_NDK_ROOT="$(ls -d "${ANDROID_HOME}/ndk/"*/ 2>/dev/null | sort -V | tail -n1)"
    ANDROID_NDK_ROOT="${ANDROID_NDK_ROOT%/}"
  fi
fi
if [ -z "${ANDROID_NDK_ROOT:-}" ] || [ ! -d "${ANDROID_NDK_ROOT}" ]; then
  echo "ERROR: ANDROID_NDK_ROOT is not set or invalid (got '${ANDROID_NDK_ROOT:-}')" >&2
  echo "Set ANDROID_NDK_ROOT to an NDK r26+ installation." >&2
  exit 1
fi
export ANDROID_NDK_ROOT
echo "Using NDK: ${ANDROID_NDK_ROOT}"

# ---------- Хостовая папка toolchain (Linux / macOS) -----------------------
case "$(uname -s)" in
  Linux*)   HOST_TAG="linux-x86_64" ;;
  Darwin*)  HOST_TAG="darwin-x86_64" ;;
  *)        echo "ERROR: unsupported host OS '$(uname -s)'" >&2; exit 1 ;;
esac
TOOLCHAIN_BIN="${ANDROID_NDK_ROOT}/toolchains/llvm/prebuilt/${HOST_TAG}/bin"
if [ ! -d "${TOOLCHAIN_BIN}" ]; then
  echo "ERROR: toolchain bin not found: ${TOOLCHAIN_BIN}" >&2
  exit 1
fi
export PATH="${TOOLCHAIN_BIN}:${PATH}"
echo "Toolchain in PATH: ${TOOLCHAIN_BIN}"

# ---------- Исходники OpenSSL ----------------------------------------------
if [ ! -d "${SRC_DIR}" ]; then
  echo "Cloning OpenSSL ${OPENSSL_VERSION} ..."
  git clone --depth 1 --branch "${OPENSSL_VERSION}" \
    https://github.com/openssl/openssl.git "${SRC_DIR}"
else
  echo "Using existing OpenSSL source at ${SRC_DIR}"
fi

# ---------- Сборка одной ABI -----------------------------------------------
# $1 — короткое имя (arm64|armv7|x86_64)
# $2 — OpenSSL Configure target
# $3 — имя каталога install (android-<abi>)
build_one() {
  local short="$1" ssl_target="$2" install_name="$3"
  local prefix="${INSTALL_ROOT}/${install_name}"

  if [ -f "${prefix}/lib/libssl.a" ] && [ -f "${prefix}/lib/libcrypto.a" ]; then
    echo "── [${short}] already built at ${prefix}, skipping"
    return 0
  fi

  echo "── [${short}] building → ${prefix}"
  pushd "${SRC_DIR}" >/dev/null

  # Полная очистка от прошлой ABI (OpenSSL Configure не любит грязное дерево).
  make distclean >/dev/null 2>&1 || true

  # Configure. Опции:
  #   no-shared    — только .a
  #   no-tests     — экономим время CI
  #   no-asm       — упрощаем кросс-сборку (теряем 5-10% перф, но переносимо)
  #   -D__ANDROID_API__=${MIN_SDK} — NDK clang выберет правильный sysroot
  ./Configure "${ssl_target}" \
    -D__ANDROID_API__="${MIN_SDK}" \
    no-shared no-tests no-asm \
    --prefix="${prefix}" \
    --openssldir="${prefix}/ssl"

  make -j"${JOBS}" build_libs
  # install_dev = только headers + .a + pkgconfig (без бинарей)
  make install_dev
  popd >/dev/null

  echo "── [${short}] done: $(ls -l "${prefix}/lib/"*.a 2>/dev/null | wc -l | tr -d ' ') static libs"
}

# ---------- Выбор ABI -------------------------------------------------------
ALL_ABIS=("arm64" "armv7" "x86_64")
if [ "$#" -eq 0 ]; then
  ABIS=("${ALL_ABIS[@]}")
else
  ABIS=("$@")
fi

echo "Target ABIs: ${ABIS[*]}"
echo "Install root: ${INSTALL_ROOT}"
echo

for abi in "${ABIS[@]}"; do
  case "${abi}" in
    arm64|arm64-v8a|aarch64)
      build_one "arm64"  "android-arm64"  "android-arm64" ;;
    armv7|armeabi-v7a|arm)
      build_one "armv7"  "android-arm"    "android-armv7" ;;
    x86_64|x64)
      build_one "x86_64" "android-x86_64" "android-x86_64" ;;
    *)
      echo "ERROR: unknown ABI '${abi}' (expected: arm64 | armv7 | x86_64)" >&2
      exit 1 ;;
  esac
done

echo
echo "Готово. Содержимое install-дерева:"
find "${INSTALL_ROOT}" -maxdepth 3 -name '*.a' -print | sort
