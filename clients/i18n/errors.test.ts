import { expect, test } from "bun:test";
import { presentError } from "./errors";

test("formats problem JSON and retains full raw body", () => {
  const body =
    '{"status":400,"title":"Sharing unavailable","detail":"Configure a public bucket","extra":123}';
  const result = presentError(
    Object.assign(new Error(`HTTP 400: ${body}`), { status: 400, body }),
  );
  expect(result.message).toBe("Sharing unavailable: Configure a public bucket");
  expect(result.raw).toContain('"extra": 123');
});
test("understands native SDK envelopes and HTTP message errors", () => {
  const envelope = {
    message: "HTTP 403: denied",
    status: 403,
    body: "denied",
    request_id: "abc",
  };
  expect(presentError(new Error(JSON.stringify(envelope))).message).toBe(
    "denied",
  );
  expect(presentError(new Error(JSON.stringify(envelope))).raw).toContain(
    "abc",
  );
  expect(presentError("HTTP 400: plain response").message).toBe(
    "plain response",
  );
});
test("empty bodies use status copy, including unknown status", () => {
  expect(presentError({ status: 400, body: "" }).message).toBe(
    "Something went wrong while sending the request.",
  );
  expect(presentError({ status: 500, body: "  " }).message).toBe(
    "The server couldn’t process the request.",
  );
  expect(presentError("HTTP 418: ").message).toContain("418");
});
test("renders arbitrary JSON, local errors and nested causes without losing data", () => {
  expect(
    presentError({ status: 400, body: '{"message":"Invalid filename"}' })
      .message,
  ).toBe("Invalid filename");
  expect(presentError({ status: 400, body: '["invalid"]' }).message).toBe(
    '[\n  "invalid"\n]',
  );
  const result = presentError(
    new Error("Sharing failed; retry in Manage uploads.", {
      cause: new Error("HTTP 500: backend unavailable"),
    }),
  );
  expect(result.message).toContain("backend unavailable");
  expect(result.raw).toContain("Sharing failed");
  expect(presentError(null).message).toBeTruthy();
  const cyclic: any = { size: 18446744073709551615n };
  cyclic.cause = cyclic;
  expect(presentError(cyclic).raw).toContain("18446744073709551615");
});

test("opaque local causes stay in raw details and structured HTTP causes stay readable", () => {
  const opaque = presentError(
    new Error("Check local setup", { cause: new Error("internal diagnostic") }),
  );
  expect(opaque.message).toBe("Check local setup");
  expect(opaque.raw).toContain("internal diagnostic");
  const http = presentError(
    new Error("Sharing failed", {
      cause: Object.assign(
        new Error(
          'HTTP 400: {"title":"Sharing unavailable","detail":"Configure storage"}',
        ),
        {
          status: 400,
          body: '{"title":"Sharing unavailable","detail":"Configure storage"}',
        },
      ),
    }),
  );
  expect(http.message).toContain("Sharing unavailable: Configure storage");
  expect(presentError({ status: 500, body: "{}" }).message).toBe(
    "The server couldn’t process the request.",
  );
});

test("direct JSON problem objects prefer their title and detail over status copy", () => {
  const problem = {
    status: 400,
    title: "Invalid request",
    detail: "Choose a filename",
  };
  expect(presentError(problem).message).toBe(
    "Invalid request: Choose a filename",
  );
  const result = presentError(JSON.stringify(problem));
  expect(result.message).toBe("Invalid request: Choose a filename");
  expect(result.raw).toContain('"title": "Invalid request"');
});
