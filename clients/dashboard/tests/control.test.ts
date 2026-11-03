import { test } from "node:test";
import assert from "node:assert/strict";
import { parseControlResponse } from "../src/lib/control";
test("administration responses preserve full u64 quantities", () => {
  const value = parseControlResponse(
    '{"type":"keys","keys":[{"id":"key","label":null,"created_at":"date","revoked":false,"limits":{"per_file_bytes":18446744073709551615,"storage_bytes":null},"usage":{"confirmed_bytes":18446744073709551615,"reserved_bytes":0,"charged_bytes":18446744073709551615}}],"next_after":null}',
  );
  assert.equal(value.type, "keys");
  if (value.type === "keys")
    assert.equal(value.keys[0]!.usage.charged_bytes, 18446744073709551615n);
});
