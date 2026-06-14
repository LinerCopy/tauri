cmake_minimum_required(VERSION 3.22)
project(gci_core LANGUAGES C CXX VERSION 1.0.0)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

# -----------------------------------------------------------------------------
# OpenSSL
# -----------------------------------------------------------------------------
# Поведение по умолчанию: ищем OpenSSL через find_package. Для Android/iOS
# рекомендуется передавать -DOPENSSL_ROOT_DIR=... с путём до статической сборки
# из third_party/openssl/install/<abi> (см. docs/build.md).
set(OPENSSL_USE_STATIC_LIBS TRUE)
find_package(OpenSSL 3.0 REQUIRED)

# -----------------------------------------------------------------------------
# GOST engine (опционально — для поддержки ГОСТ TLS на гос-сайтах)
# -----------------------------------------------------------------------------
# Если задан GOST_ENGINE_ROOT — линкуем gost-engine как static provider.
# Пример: -DGOST_ENGINE_ROOT=third_party/gost-engine/install/android-arm64
option(GCI_ENABLE_GOST "Link gost-engine for GOST cipher support" OFF)
if(GCI_ENABLE_GOST AND DEFINED GOST_ENGINE_ROOT)
    set(GOST_LIB_DIR "${GOST_ENGINE_ROOT}/lib")
    set(GOST_INCLUDE_DIR "${GOST_ENGINE_ROOT}/include")
    # gost-engine может собираться в несколько .a — ищем все
    file(GLOB GOST_STATIC_LIBS "${GOST_LIB_DIR}/*.a")
    if(GOST_STATIC_LIBS)
        message(STATUS "GOST engine: ${GOST_STATIC_LIBS}")
        add_library(gost_engine INTERFACE)
        target_link_libraries(gost_engine INTERFACE ${GOST_STATIC_LIBS})
        target_include_directories(gost_engine INTERFACE "${GOST_INCLUDE_DIR}")
        target_compile_definitions(gost_engine INTERFACE GCI_GOST_ENABLED=1)
    else()
        message(WARNING "GOST engine: no .a files found in ${GOST_LIB_DIR}")
        set(GCI_ENABLE_GOST OFF)
    endif()
endif()

# -----------------------------------------------------------------------------
# nlohmann/json (header-only). Пытаемся найти системно, иначе скачиваем.
# -----------------------------------------------------------------------------
find_package(nlohmann_json 3.10 QUIET)
if(NOT nlohmann_json_FOUND)
    include(FetchContent)
    FetchContent_Declare(
        nlohmann_json
        GIT_REPOSITORY https://github.com/nlohmann/json.git
        GIT_TAG v3.11.3
    )
    set(JSON_BuildTests OFF CACHE INTERNAL "")
    FetchContent_MakeAvailable(nlohmann_json)
endif()

# -----------------------------------------------------------------------------
# Целевая библиотека
# -----------------------------------------------------------------------------
set(GCI_SOURCES
    src/tls_client.cpp
    src/x509_parser.cpp
    src/http_client.cpp
    src/inspector.cpp
)

set(GCI_HEADERS
    include/inspector.h
    src/tls_client.h
    src/x509_parser.h
    src/http_client.h
)

# Статическая библиотека удобна для Tauri/Rust сборки (cc-rs / cmake-rs).
add_library(gci_core_static STATIC ${GCI_SOURCES} ${GCI_HEADERS})
set_target_properties(gci_core_static PROPERTIES OUTPUT_NAME gci_core)
target_include_directories(gci_core_static
    PUBLIC  ${CMAKE_CURRENT_SOURCE_DIR}/include
    PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/src
)
target_link_libraries(gci_core_static
    PUBLIC OpenSSL::SSL OpenSSL::Crypto
    PRIVATE nlohmann_json::nlohmann_json
    $<$<BOOL:${GCI_ENABLE_GOST}>:gost_engine>
)
target_compile_options(gci_core_static PRIVATE
    -Wall -Wextra -Wpedantic
    $<$<CONFIG:Release>:-O2 -fvisibility=hidden>
)

# Динамическая версия — для desktop preview и Android (если предпочтительнее .so).
option(GCI_BUILD_SHARED "Build shared library variant" ON)
if(GCI_BUILD_SHARED)
    add_library(gci_core_shared SHARED ${GCI_SOURCES} ${GCI_HEADERS})
    set_target_properties(gci_core_shared PROPERTIES OUTPUT_NAME gci_core)
    target_include_directories(gci_core_shared
        PUBLIC  ${CMAKE_CURRENT_SOURCE_DIR}/include
        PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/src
    )
    target_link_libraries(gci_core_shared
        PUBLIC OpenSSL::SSL OpenSSL::Crypto
        PRIVATE nlohmann_json::nlohmann_json
        $<$<BOOL:${GCI_ENABLE_GOST}>:gost_engine>
    )
    target_compile_options(gci_core_shared PRIVATE
        -Wall -Wextra -Wpedantic
        $<$<CONFIG:Release>:-O2 -fvisibility=hidden>
    )
endif()

# -----------------------------------------------------------------------------
# Тесты
# -----------------------------------------------------------------------------
# По умолчанию тесты выключены. При сборке через cmake-rs (Tauri/Android/iOS)
# тесты не нужны и могут вызвать линковочные ошибки (Catch2 тянет
# __android_log_write). CI-скрипты для хостовых тестов передают
# -DGCI_BUILD_TESTS=ON явно.
option(GCI_BUILD_TESTS "Build unit tests" OFF)
if(GCI_BUILD_TESTS)
    enable_testing()
    add_subdirectory(tests)
endif()

# -----------------------------------------------------------------------------
# Установка (для XCFramework / Android sysroot)
# -----------------------------------------------------------------------------
include(GNUInstallDirs)
install(TARGETS gci_core_static
        ARCHIVE DESTINATION ${CMAKE_INSTALL_LIBDIR}
        LIBRARY DESTINATION ${CMAKE_INSTALL_LIBDIR}
        RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR})
install(FILES include/inspector.h
        DESTINATION ${CMAKE_INSTALL_INCLUDEDIR})
