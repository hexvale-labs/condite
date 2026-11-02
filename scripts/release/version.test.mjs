import { test } from 'node:test';
import assert from 'node:assert/strict';
import { parseTag, validVersion, checkVersions } from './version.mjs';
test('strict semver tags select independent or umbrella releases', () => {
  assert.deepEqual(parseTag('core/v1.2.3'), { component: 'core', version: '1.2.3' });
  assert.deepEqual(parseTag('v1.2.3-rc.1'), { component: 'umbrella', version: '1.2.3-rc.1' });
  for (const tag of ['main', 'core:v1.2.3', 'core/v01.2.3', 'v1.2', 'other/v1.2.3', 'v1.2.3-01']) assert.throws(() => parseTag(tag));
  assert.ok(validVersion('1.2.3+build.1'));
});
test('checked-in package and generated versions agree with manifest', () => checkVersions());

import { mkdtempSync, mkdirSync, copyFileSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { root } from './version.mjs';
test('version checks reject manifest and lockfile drift in an independent fixture', () => {
  const fixture = mkdtempSync(join(tmpdir(), 'condite-versions-'));
  try {
    for (const file of ['versions.json', 'Cargo.toml', 'Cargo.lock', 'examples/sdk-consumer/Cargo.lock',
      ...['condite-core', 'condite-ctl', 'condite-sdk', 'condite-sdk-node', 'condite-sdk-wasm', 'condite-shared', 'condite-metadata'].map(name => `crates/${name}/Cargo.toml`),
      'crates/condite-sdk-node/package.json', 'crates/condite-sdk-wasm/package.json', 'clients/dashboard/package.json', 'clients/vencord/package.json', 'clients/vencord-extension/package.json']) {
      mkdirSync(dirname(join(fixture, file)), { recursive: true }); copyFileSync(join(root, file), join(fixture, file));
    }
    checkVersions(fixture);
    const json = join(fixture, 'clients/dashboard/package.json');
    writeFileSync(json, readFileSync(json, 'utf8').replaceAll('\n', '\r\n'));
    checkVersions(fixture);
    const file = join(fixture, 'crates/condite-ctl/Cargo.toml');
    const original = readFileSync(file, 'utf8');
    writeFileSync(file, original.replace('version = "0.1.0"', 'version = "9.0.0"'));
    assert.throws(() => checkVersions(fixture), /drift/);
    writeFileSync(file, original);
    const lock = join(fixture, 'Cargo.lock');
    writeFileSync(lock, readFileSync(lock, 'utf8').replace('name = "condite-ctl"\nversion = "0.1.0"', 'name = "condite-ctl"\nversion = "9.0.0"'));
    assert.throws(() => checkVersions(fixture), /drift/);
  } finally { rmSync(fixture, { recursive: true, force: true }); }
});
