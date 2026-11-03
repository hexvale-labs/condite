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
    assert.throws(() => allowedDomains(origin), /DASHBOARD_ORIGIN/);
  }
});
