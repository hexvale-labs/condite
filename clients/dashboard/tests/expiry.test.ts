import { test } from "node:test";
import assert from "node:assert/strict";
import { resolveExpiry } from "../src/lib/expiry";

test("retention preserves default and Never without sentinel dates", () => {
  assert.equal(
    resolveExpiry({ mode: "default", date: "", days: "14", subtract: false }),
    undefined,
  );
  assert.equal(
    resolveExpiry({ mode: "never", date: "", days: "14", subtract: false }),
    null,
  );
});
test("interval updates use the existing deadline and dates are UTC", () => {
  const base = "2030-01-14T10:00:00Z";
  assert.equal(
    resolveExpiry(
      { mode: "interval", date: "", days: "7", subtract: true },
      base,
    ),
    "2030-01-07T10:00:00.000Z",
  );
  assert.equal(
    resolveExpiry({
      mode: "date",
      date: "2030-02-01T12:30",
      days: "14",
      subtract: false,
    }),
    "2030-02-01T12:30:00.000Z",
  );
  assert.throws(() =>
    resolveExpiry({ mode: "interval", date: "", days: "NaN", subtract: false }),
  );
});

test("expiry rejects normalized impossible calendar dates and fractional day controls", () => {
  assert.throws(() =>
    resolveExpiry({
      mode: "date",
      date: "2030-02-31T12:00",
      days: "14",
      subtract: false,
    }),
  );
  assert.throws(() =>
    resolveExpiry({ mode: "interval", date: "", days: "1.5", subtract: false }),
  );
});
