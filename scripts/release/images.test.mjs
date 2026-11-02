import { test } from 'node:test';
import assert from 'node:assert/strict';
import { imageComponents, publication } from './images.mjs';
test('only image components and umbrella tags select images', () => {
  assert.deepEqual(imageComponents('core'), ['core']);
  assert.deepEqual(imageComponents('umbrella'), ['core', 'dashboard']);
  assert.deepEqual(imageComponents('sdk'), []);
  assert.deepEqual(imageComponents('vencord'), []);
});
test('immutable versions reuse only identical build inputs', () => {
  assert.equal(publication(null, 'abc').build, true);
  assert.equal(publication({ fingerprint: 'abc', digest: 'sha256:123' }, 'abc').build, false);
  assert.throws(() => publication({ fingerprint: 'old' }, 'new'), /bump/i);
});

import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
import { fingerprint } from './images.mjs';
test('fingerprints ignore unrelated sources and include shared build inputs', () => {
  const directory = mkdtempSync(join(tmpdir(), 'condite-inputs-'));
  const git = (...args) => execFileSync('git', args, { cwd: directory, stdio: 'pipe' });
  const commit = () => { git('add', '.'); git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.test', 'commit', '-qm', 'fixture'); };
  try {
    git('init', '-q');
    mkdirSync(join(directory, 'crates/condite-shared'), { recursive: true });
    mkdirSync(join(directory, 'clients/vencord'), { recursive: true });
    writeFileSync(join(directory, 'crates/condite-shared/lib.rs'), 'one');
    commit();
    const initial = fingerprint('core', 'HEAD', directory);
    writeFileSync(join(directory, 'clients/vencord/unrelated.ts'), 'two'); commit();
    assert.equal(fingerprint('core', 'HEAD', directory), initial);
    writeFileSync(join(directory, 'crates/condite-shared/lib.rs'), 'two'); commit();
    assert.notEqual(fingerprint('core', 'HEAD', directory), initial);
  } finally { rmSync(directory, { recursive: true, force: true }); }
});
