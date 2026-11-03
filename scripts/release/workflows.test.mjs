import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { root, versions } from './version.mjs';

for (const workflow of ['images', 'vencord-release']) {
  test(`${workflow} metadata propagates validation failures and emits valid matrices`, () => {
    const yaml = readFileSync(join(root, `.github/workflows/${workflow}.yml`), 'utf8');
    const metadata = yaml.split('  build:')[0];
    const run = metadata.match(/^        run: (.+)(?:\n|$)/m);
    assert.ok(run, 'metadata must contain a run step');
    const script = run[1] === '|'
      ? metadata.slice(run.index + run[0].length).replace(/^          /gm, '')
      : run[1];
    const directory = mkdtempSync(join(tmpdir(), 'condite-matrix-'));
    const output = join(directory, 'output');
    try {
      for (const tag of ['v999999.0.0', 'v01.0.0', `v${versions().umbrella}`]) {
        writeFileSync(output, '');
        const result = spawnSync('bash', ['-e', '-c', script], {
          cwd: root, encoding: 'utf8', env: { ...process.env, RELEASE_TAG: tag, GITHUB_OUTPUT: output }
        });
        if (tag === `v${versions().umbrella}`) {
          assert.equal(result.status, 0, result.stderr);
          const value = readFileSync(output, 'utf8').trim();
          assert.ok(value.startsWith('value='));
          assert.equal(JSON.parse(value.slice(6)).include.length, workflow === 'images' ? 4 : 6);
        } else {
          assert.notEqual(result.status, 0, `${tag} must fail metadata`);
          assert.match(result.stderr, /does not match|Invalid release tag|Not a Vencord/);
          assert.equal(readFileSync(output, 'utf8'), '');
        }
      }
    } finally { rmSync(directory, { recursive: true, force: true }); }
  });
}
