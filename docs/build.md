# Сборка

## Предварительные требования

| Платформа | Что нужно                                                                |
| --------- | ------------------------------------------------------------------------ |
| Любая     | Rust 1.77+, Node 20+, CMake 3.22+, Git, OpenSSL 3.x (для desktop)         |
| Android   | JDK 17, Android SDK + NDK r26+, переменные `ANDROID_HOME`, `NDK_HOME`     |
| iOS       | Xcode 15+ (только macOS), `xcode-select --install`, CocoaPods             |

Установка Tauri CLI:

```bash
cargo install tauri-cli --version "^2.0"
```

Установка `tauri-cli` cargo-плагина для мобильных команд (`tauri android`,
`tauri ios`) выполняется тем же crate.

## OpenSSL — статическая сборка

Создайте отдельный артефакт для каждой ABI/архитектуры. Пример каталога:

```
third_party/openssl/install/
  android-arm64/   include/ lib/libssl.a libcrypto.a
  android-armv7/
  android-x86_64/
  ios-arm64/
  ios-simulator-arm64/
  ios-simulator-x86_64/
```

### Android

Готовый скрипт (тот же, что выполняется в CI):

```bash
# собрать все три ABI разом
./scripts/build-openssl-android.sh
# или только arm64-v8a
./scripts/build-openssl-android.sh arm64
```

Скрипт сам определит NDK (`ANDROID_NDK_ROOT` / `$ANDROID_HOME/ndk/...`),
поднимет тулчейн `linux-x86_64` или `darwin-x86_64`, склонирует OpenSSL
`openssl-3.3.0` в `third_party/openssl-src/` и установит статические
библиотеки в `third_party/openssl/install/android-<abi>/{include,lib}`.

Ручной вариант (если нужно тонко контролировать сборку):

```bash
git clone https://github.com/openssl/openssl --branch openssl-3.3.0
cd openssl
export ANDROID_NDK_ROOT=$ANDROID_HOME/ndk/26.2.11394342
export PATH=$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/darwin-x86_64/bin:$PATH

# arm64-v8a
./Configure android-arm64 -D__ANDROID_API__=26 \
    --prefix=$PWD/../third_party/openssl/install/android-arm64 \
    no-shared no-tests
make -j && make install_sw

# armeabi-v7a
make clean
./Configure android-arm -D__ANDROID_API__=26 \
    --prefix=$PWD/../third_party/openssl/install/android-armv7 \
    no-shared no-tests
make -j && make install_sw
```

### iOS

```bash
# device (arm64)
./Configure ios64-xcrun \
    --prefix=$PWD/../third_party/openssl/install/ios-arm64 \
    no-shared no-tests
make -j && make install_sw

# simulator arm64 (Apple Silicon Mac)
./Configure iossimulator-xcrun -arch arm64 \
    --prefix=$PWD/../third_party/openssl/install/ios-simulator-arm64 \
    no-shared no-tests
make -j && make install_sw
```

Объединение в XCFramework:

```bash
xcodebuild -create-xcframework \
  -library ios-arm64/lib/libssl.a            -headers ios-arm64/include \
  -library ios-simulator-arm64/lib/libssl.a  -headers ios-simulator-arm64/include \
  -output OpenSSL.xcframework
```

## cpp-core (только desktop, для unit-тестов)

```bash
cd cpp-core
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build -j
ctest --test-dir build --output-on-failure
```

## Desktop preview (без мобильных тулчейнов)

```bash
cd frontend && npm install
cd ../src-tauri
cargo tauri dev
```

Если OpenSSL/CMake тулчейн недоступен — соберите Rust-команды с feature `mock-core`:

```bash
GCI_SKIP_NATIVE=1 cargo tauri dev --features mock-core
```

## Android

```bash
cd src-tauri
cargo tauri android init      # генерирует gen/android, требует JDK 17
# Передаём пути к OpenSSL через env. CMakeLists.txt сам подцепит OPENSSL_ROOT_DIR.
export OPENSSL_LIB_DIR=$PWD/../third_party/openssl/install/android-arm64/lib
export OPENSSL_INCLUDE_DIR=$PWD/../third_party/openssl/install/android-arm64/include
export OPENSSL_ROOT_DIR=$PWD/../third_party/openssl/install/android-arm64

cargo tauri android dev       # запуск на подключённом устройстве/эмуляторе
cargo tauri android build --apk
```

ABI выбираются через `tauri.conf.json` → `app.android.archs`.

## iOS

```bash
cd src-tauri
cargo tauri ios init          # требует Xcode + cocoapods
export OPENSSL_LIB_DIR=$PWD/../third_party/openssl/install/ios-arm64/lib
export OPENSSL_INCLUDE_DIR=$PWD/../third_party/openssl/install/ios-arm64/include
export OPENSSL_ROOT_DIR=$PWD/../third_party/openssl/install/ios-arm64

cargo tauri ios dev
cargo tauri ios build
```

## Запуск тестов

```bash
# Rust
cd src-tauri
GCI_SKIP_NATIVE=1 cargo test --features mock-core

# C++
cd ../cpp-core
cmake -S . -B build && cmake --build build -j
ctest --test-dir build --output-on-failure

# Vue
cd ../frontend
npm test
```
