import { test } from "node:test";
import assert from "node:assert/strict";
import { allowedDomains } from "../server/origin";

test("runtime origin adds an exact host, protocol and port to loopback defaults", () => {
  assert.deepEqual(allowedDomains("https://dashboard.example:8443"), [
    { hostname: "localhost" }, { hostname: "127.0.0.1" },
    { hostname: "dashboard.example", protocol: "https", port: "8443" },
  ]);
  assert.equal(allowedDomains(undefined).length, 2);
  assert.deepEqual(allowedDomains("https://other.example")[2], {
    hostname: "other.example", protocol: "https", port: "",
  });
});

test("invalid runtime origins fail closed", () => {
  for (const origin of ["", "bad", "ftp://dashboard.example", "https://user:pass@example.com", "https://example.com/path", "https://example.com?x=1", "https://example.com/#x", "https://*.example.com"]) {
    assert.throws(() => allowedDomains(origin), /DASHBOARD_ORIGINS/);
  }
});

test("comma-separated runtime origins trim whitespace and preserve exact ports", () => {
  assert.deepEqual(allowedDomains(" https://dashboard.example:8443 , http://other.example:8080 ").slice(2), [
    { hostname: "dashboard.example", protocol: "https", port: "8443" },
    { hostname: "other.example", protocol: "http", port: "8080" },
  ]);
});

test("invalid or empty entries reject the entire origins list", () => {
  for (const value of ["https://good.example,invalid", "https://good.example,", ",https://good.example", "https://good.example, ,https://other.example"]) {
    assert.throws(() => allowedDomains(value), /DASHBOARD_ORIGINS/);
  }
});
