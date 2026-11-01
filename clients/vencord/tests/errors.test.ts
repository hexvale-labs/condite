import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { presentError } from "../condite.desktop/errors";

test("standalone formatter stays aligned with the shared MIT formatter", () => {
  expect(readFileSync(new URL('../condite.desktop/errors.ts', import.meta.url), 'utf8')).toBe(readFileSync(new URL('../../i18n/errors.ts', import.meta.url), 'utf8'));
  expect(presentError(new Error('HTTP 500: ')).message).toContain('server');
});
