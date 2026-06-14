#include <catch2/catch_test_macros.hpp>

#include "tls_client.h"

using namespace gci;

TEST_CASE("parse_url accepts https with default port", "[url]") {
    ParsedUrl out;
    std::string err;
    REQUIRE(parse_url("https://gosuslugi.ru/path?x=1", out, err));
    REQUIRE(out.scheme == "https");
    REQUIRE(out.host   == "gosuslugi.ru");
    REQUIRE(out.port   == 443);
    REQUIRE(out.path   == "/path?x=1");
}

TEST_CASE("parse_url accepts custom port", "[url]") {
    ParsedUrl out;
    std::string err;
    REQUIRE(parse_url("https://example.gov.ru:8443/", out, err));
    REQUIRE(out.port == 8443);
    REQUIRE(out.path == "/");
}

TEST_CASE("parse_url rejects http scheme", "[url]") {
    ParsedUrl out;
    std::string err;
    REQUIRE_FALSE(parse_url("http://example.com/", out, err));
    REQUIRE(err.find("https") != std::string::npos);
}

TEST_CASE("parse_url rejects missing host", "[url]") {
    ParsedUrl out;
    std::string err;
    REQUIRE_FALSE(parse_url("https:///path", out, err));
}

TEST_CASE("parse_url assigns root path when absent", "[url]") {
    ParsedUrl out;
    std::string err;
    REQUIRE(parse_url("https://nalog.gov.ru", out, err));
    REQUIRE(out.path == "/");
}
