import { test, expect } from "@playwright/test";

test("runtime origin permits proxy writes and rejects forged origins", async ({ request }) => {
  const headers = {
    "X-Forwarded-Host": "dashboard.example:8443",
    "X-Forwarded-Proto": "https",
    Origin: "https://dashboard.example:8443",
  };
  const trusted = await request.delete("/api/v1/uploads/missing", { headers });
  expect(trusted.status()).toBe(204);
  for (const origin of ["https://build-time.example", "https://evil.example", "https://dashboard.example", "http://dashboard.example:8443"]) {
    const response = await request.delete("/api/v1/uploads/missing", { headers: { ...headers, Origin: origin } });
    expect(response.status()).toBe(403);
  }
  const forged = await request.delete("/api/v1/uploads/missing", {
    headers: { ...headers, "X-Forwarded-Host": "evil.example:8443", Origin: "https://evil.example:8443" },
  });
  expect(forged.status()).toBe(403);
});
