include(FetchContent)
FetchContent_Declare(
    Catch2
    GIT_REPOSITORY https://github.com/catchorg/Catch2.git
    GIT_TAG v3.5.4
)
FetchContent_MakeAvailable(Catch2)

add_executable(gci_unit_tests
    test_url_parse.cpp
    test_inspector_error_paths.cpp
)
target_link_libraries(gci_unit_tests
    PRIVATE gci_core_static Catch2::Catch2WithMain nlohmann_json::nlohmann_json
)
target_include_directories(gci_unit_tests
    PRIVATE ${CMAKE_SOURCE_DIR}/src
            ${CMAKE_SOURCE_DIR}/include
)

include(Catch)
catch_discover_tests(gci_unit_tests)
