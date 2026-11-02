import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';

test('ctl install, build and checks dispatch without starting servers', () => {
  const dir = mkdtempSync(join(tmpdir(), 'condite-task-'));
  try {
    const log = join(dir, 'calls');
    writeFileSync(join(dir, 'just'), '#!/bin/sh\nprintf "%s\\n" "$*" >> "$TASK_LOG"\n', { mode: 0o755 });
    for (const action of ['install', 'build', 'check']) {
      const p = spawnSync('bash', ['scripts/dev/task.sh', action, 'ctl'], {
        env: { ...process.env, PATH: `${dir}:${process.env.PATH}`, TASK_LOG: log }, encoding: 'utf8'
      });
      assert.equal(p.status, 0, p.stderr);
    }
    assert.match(readFileSync(log, 'utf8'), /crates\/condite-ctl\/justfile install/);
    assert.match(readFileSync(log, 'utf8'), /crates\/condite-ctl\/justfile build/);
    assert.match(readFileSync(log, 'utf8'), /crates\/condite-ctl\/justfile check/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('ctl shell resolves configured cargo target directory and leaves parent PATH intact', () => {
  const dir = mkdtempSync(join(tmpdir(), 'condite-shell-'));
  try {
    writeFileSync(join(dir, 'cargo'), '#!/bin/sh\nif [ "$1" = metadata ]; then printf \'{"target_directory":"%s"}\\n\' "$TASK_TARGET"; fi\n', { mode: 0o755 });
    writeFileSync(join(dir, 'shell'), '#!/bin/sh\nprintf "%s" "$PATH"\n', { mode: 0o755 });
    const before = process.env.PATH;
    const p = spawnSync('bash', ['scripts/dev/ctl-shell.sh'], { encoding: 'utf8', env: {
      ...process.env, PATH: `${dir}:${before}`, SHELL: join(dir, 'shell'), TASK_TARGET: join(dir, 'custom target')
    } });
    assert.equal(p.status, 0, p.stderr);
    assert.ok(p.stdout.startsWith(`${dir}/custom target/debug:`), p.stdout);
    assert.equal(process.env.PATH, before);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
