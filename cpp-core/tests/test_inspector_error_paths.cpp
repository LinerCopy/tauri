#include <catch2/catch_test_macros.hpp>

#include "inspector.h"

#include <nlohmann/json.hpp>
#include <string>

using json = nlohmann::json;

namespace {

json run(const std::string& req) {
    const char* raw = inspect_url(req.c_str());
    REQUIRE(raw != nullptr);
    std::string s(raw);
    inspector_free_string(raw);
    return json::parse(s);
}

}  // namespace

TEST_CASE("inspect_url returns JSON even on null", "[inspector]") {
    const char* raw = inspect_url(nullptr);
    REQUIRE(raw != nullptr);
    std::string s(raw);
    inspector_free_string(raw);
    auto j = json::parse(s);
    REQUIRE(j["errors"].is_array());
    REQUIRE_FALSE(j["errors"].empty());
}

TEST_CASE("inspect_url rejects empty url", "[inspector]") {
    auto j = run(R"({"trustStorePath":"/tmp/does-not-matter"})");
    REQUIRE(j["errors"][0]["code"] == "EMPTY_URL");
    REQUIRE(j["validation"]["chain_ok"] == false);
    REQUIRE(j["validation"]["hostname_ok"] == false);
}

TEST_CASE("inspect_url rejects empty trust store path", "[inspector]") {
    auto j = run(R"({"url":"https://example.com/"})");
    REQUIRE(j["errors"][0]["code"] == "EMPTY_TRUST");
}

TEST_CASE("inspect_url rejects malformed json", "[inspector]") {
    auto j = run("{not json");
    REQUIRE(j["errors"][0]["code"] == "BAD_JSON");
}

TEST_CASE("inspect_url rejects non-https scheme", "[inspector]") {
    auto j = run(R"({"url":"ftp://example.com/","trustStorePath":"/tmp/a"})");
    REQUIRE(j["errors"][0]["code"] == "URL_PARSE");
}

TEST_CASE("inspector_version is non-empty", "[inspector]") {
    REQUIRE(std::string(inspector_version()).size() > 0);
}
