# cpp-core

C++17 ядро: TLS-клиент на OpenSSL 3, разбор X.509, HTTP GET и сборка JSON-DTO.

Экспортирует C-ABI:

```c
const char* inspect_url(const char* request_json);
void        inspector_free_string(const char* ptr);
const char* inspector_version(void);
```

## Сборка (desktop)

```bash
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build -j
ctest --test-dir build --output-on-failure
```

## Сборка под Android

```bash
export NDK=$ANDROID_HOME/ndk/26.2.11394342
cmake -S . -B build-android-arm64 \
  -DCMAKE_TOOLCHAIN_FILE=$NDK/build/cmake/android.toolchain.cmake \
  -DANDROID_ABI=arm64-v8a \
  -DANDROID_PLATFORM=android-26 \
  -DOPENSSL_ROOT_DIR=$PWD/../third_party/openssl/install/android-arm64 \
  -DCMAKE_BUILD_TYPE=Release
cmake --build build-android-arm64 -j
```

## Сборка под iOS

```bash
cmake -S . -B build-ios -G Xcode \
  -DCMAKE_SYSTEM_NAME=iOS \
  -DCMAKE_OSX_ARCHITECTURES=arm64 \
  -DCMAKE_OSX_DEPLOYMENT_TARGET=14.0 \
  -DOPENSSL_ROOT_DIR=$PWD/../third_party/openssl/install/ios-arm64
cmake --build build-ios --config Release
```

См. подробнее [`docs/build.md`](../docs/build.md).
