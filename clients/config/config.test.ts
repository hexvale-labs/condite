import { expect, test } from "bun:test";
import { parseConfig } from "./index";
test("interpolation is single pass, escaped and cannot inject structure", () => {
  expect(parseConfig("secret = '${SECRET}'\nliteral = '$${OTHER}'", { SECRET: "'\n[table]\nkey='${OTHER}'" })).toEqual({secret:"'\n[table]\nkey='${OTHER}'", literal:"${OTHER}"});
});
test("missing references and parser errors never expose source secrets", () => {
  expect(() => parseConfig("secret = '${MISSING}'", {})).toThrow("MISSING");
  try { parseConfig("secret = 'TOP-SECRET'\nbroken", {}); } catch (error) { expect(String(error)).not.toContain("TOP-SECRET"); }
});
test("unterminated environment references are rejected",()=>{
  expect(()=>parseConfig("secret = '${BROKEN'",{})).toThrow("unterminated");
});
import { clientConfig } from "./index";
test("client configuration rejects unknown fields and invalid limits",()=>{
  expect(()=>clientConfig("unrecognized = true")).toThrow("Unknown");
  expect(()=>clientConfig("retention_days = -1")).toThrow("retention_days");
  expect(()=>clientConfig("api_url = 'javascript:alert(1)'")).toThrow("HTTP");
});
