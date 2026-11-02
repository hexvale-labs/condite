import { test } from 'node:test';
import assert from 'node:assert/strict';
import { releaseTargets, releaseAllowed } from './vencord.mjs';
test('Vencord ZIPs require a published release and matching release family', () => {
  assert.equal(releaseAllowed('published', 'v1.0.0'), true);
  assert.equal(releaseAllowed('published', 'vencord/v1.0.0'), true);
  assert.equal(releaseAllowed('created', 'v1.0.0'), false);
  assert.equal(releaseAllowed('published', 'core/v1.0.0'), false);
  assert.equal(releaseAllowed('published', 'v1.00.0'), false);
});
test('desktop and standalone native packages cover six host platforms', () => {
  assert.equal(releaseTargets.length, 6);
  for (const target of releaseTargets) assert.ok(target.rustHost.startsWith(target.arch === 'x64' ? 'x86_64-' : 'aarch64-'));
  for (const os of ['linux', 'win32', 'darwin']) for (const arch of ['x64', 'arm64']) assert.ok(releaseTargets.some(t => t.os === os && t.arch === arch));
});

import { buildArguments } from '../../clients/vencord-extension/build-options.mjs';
test('packaged custom builds cannot update themselves to official Vencord', () => {
  assert.deepEqual(buildArguments(false), ['build', '--disable-updater']);
  assert.deepEqual(buildArguments(true), ['build', '--standalone', '--disable-updater']);
});
