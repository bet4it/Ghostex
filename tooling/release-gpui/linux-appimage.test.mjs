import { spawnSync } from 'node:child_process';
import {
  chmodSync,
  closeSync,
  existsSync,
  fstatSync,
  mkdirSync,
  mkdtempSync,
  openSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, describe, expect, test } from 'vitest';

import { customerDownloadEntries } from './customer-downloads.mjs';
import { productDefinition } from './product-inputs.mjs';
import { PUBLISH_STAGES, stageArtifactNames } from './publish-stage.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const appRun = path.join(repoRoot, 'tooling/packaging/appimage/AppRun');

describe('the Linux AppImage release product', () => {
  test('ships in each architecture stage and the Linux line of the download block', () => {
    expect(PUBLISH_STAGES.linux).toContain('linux-appimage-x64');
    expect(PUBLISH_STAGES['linux-arm64']).toContain('linux-appimage-arm64');
    const plan = { expectedPlatforms: ['gxserver-linux-arm64', 'linux-appimage-arm64'] };
    /* The names the Linux job uploads (release-gpui-linux.yml). */
    expect(stageArtifactNames({ plan, stage: 'linux-arm64' })).toEqual([
      'release-plan',
      'release-gxserver-linux-arm64',
      'release-linux-appimage-arm64',
      'release-provenance-gxserver-linux-arm64',
      'release-provenance-linux-appimage-arm64',
    ]);

    const assets = ['linux-appimage-x64', 'linux-appimage-arm64'].flatMap((id) =>
      productDefinition(id).artifacts('7.8.0')
    );
    const groups = customerDownloadEntries('7.8.0', assets);
    expect(groups.map((group) => [group.title, group.line, group.downloads.map((d) => d.label)])).toEqual([
      ['Linux x64', 2, ['AppImage', 'AUR (ghostex-bin)']],
      ['Linux ARM64', 2, ['AppImage']],
    ]);
  });

  test('pins the type2 runtime by digest for both architectures', () => {
    const script = readFileSync(path.join(repoRoot, 'tooling/release-gpui/linux-appimage.sh'), 'utf8');
    expect(script).toMatch(/^APPIMAGE_RUNTIME_RELEASE="\d{8}"$/mu);
    for (const name of ['runtime-x86_64', 'runtime-aarch64']) {
      expect(script).toMatch(
        new RegExp(`APPIMAGE_RUNTIME_NAME="${name}"\\n\\s*APPIMAGE_RUNTIME_SHA256="[0-9a-f]{64}"`, 'u')
      );
    }
  });
});

/*
 * AppRun is a Linux launcher (GNU cp, /proc); the gates job runs on macOS, so
 * these run wherever the suite runs on Linux.
 */
describe.skipIf(process.platform !== 'linux')('the AppImage launcher', () => {
  let scratch = '';
  afterEach(() => {
    if (scratch) rmSync(scratch, { force: true, recursive: true });
    scratch = '';
  });

  /* A fake AppDir whose Ghostex records where it ran and with which arguments. */
  function appDir(name, payloadId, files) {
    const root = path.join(scratch, name);
    mkdirSync(path.join(root, 'opt/ghostex'), { recursive: true });
    writeFileSync(path.join(root, 'AppRun'), readFileSync(appRun));
    chmodSync(path.join(root, 'AppRun'), 0o755);
    writeFileSync(path.join(root, '.ghostex-appimage'), `${payloadId}\n`);
    writeFileSync(
      path.join(root, 'opt/ghostex/Ghostex'),
      '#!/usr/bin/env bash\nprintf "%s\\n" "$0" "$@" >"$GHOSTEX_TEST_LAUNCH"\nls /proc/$$/fd >"$GHOSTEX_TEST_FDS"\n'
    );
    chmodSync(path.join(root, 'opt/ghostex/Ghostex'), 0o755);
    for (const [relative, content] of Object.entries(files)) {
      mkdirSync(path.dirname(path.join(root, 'opt/ghostex', relative)), { recursive: true });
      writeFileSync(path.join(root, 'opt/ghostex', relative), content);
    }
    return root;
  }

  function launch(root, ...args) {
    const result = spawnSync(path.join(root, 'AppRun'), args, {
      encoding: 'utf8',
      env: {
        ...process.env,
        GHOSTEX_TEST_FDS: path.join(scratch, 'fds'),
        GHOSTEX_TEST_LAUNCH: path.join(scratch, 'launch'),
        HOME: path.join(scratch, 'home'),
        XDG_DATA_HOME: path.join(scratch, 'data'),
      },
    });
    expect(result.stderr).toBe('');
    expect(result.status).toBe(0);
    return readFileSync(path.join(scratch, 'launch'), 'utf8').trim().split('\n');
  }

  test('copies the payload into the data folder once and starts it from there', () => {
    scratch = mkdtempSync(path.join(tmpdir(), 'ghostex-apprun-'));
    const install = path.join(scratch, 'data/ghostex/appimage');
    const first = appDir('v1', '7.8.0-aaaa', { 'gxserver/bin/gxserver': 'v1', 'resources/old.json': 'old' });
    expect(launch(first, '--flag', 'two words')).toEqual([path.join(install, 'Ghostex'), '--flag', 'two words']);
    expect(readFileSync(path.join(install, 'gxserver/bin/gxserver'), 'utf8')).toBe('v1');
    expect(readFileSync(path.join(install, '.ghostex-appimage'), 'utf8')).toBe('7.8.0-aaaa\n');
    /* Only stdio reaches the app, so the AppImage's keep-alive pipe cannot hold the mount. */
    expect(readFileSync(path.join(scratch, 'fds'), 'utf8').trim().split('\n').sort()).toEqual(['0', '1', '2', '255']);

    const stampTime = statSync(path.join(install, '.ghostex-appimage')).mtimeMs;
    launch(first);
    expect(statSync(path.join(install, '.ghostex-appimage')).mtimeMs).toBe(stampTime);
  });

  test('a newer image replaces changed files by new inodes and drops removed ones', () => {
    scratch = mkdtempSync(path.join(tmpdir(), 'ghostex-apprun-'));
    const install = path.join(scratch, 'data/ghostex/appimage');
    launch(appDir('v1', '7.8.0-aaaa', { 'gxserver/bin/gxserver': 'v1', 'resources/old.json': 'old' }));
    /* Stands in for a gxserver still running the previous version. */
    const running = openSync(path.join(install, 'gxserver/bin/gxserver'), 'r');
    try {
      launch(appDir('v2', '7.9.0-bbbb', { 'gxserver/bin/gxserver': 'v2', 'resources/new.json': 'new' }));
      expect(readFileSync(path.join(install, 'gxserver/bin/gxserver'), 'utf8')).toBe('v2');
      /* It keeps reading its own bytes, as it does through a dpkg upgrade: the file was replaced, not rewritten. */
      expect(readFileSync(running, 'utf8')).toBe('v1');
      expect(fstatSync(running).ino).not.toBe(statSync(path.join(install, 'gxserver/bin/gxserver')).ino);
    } finally {
      closeSync(running);
    }
    expect(existsSync(path.join(install, 'resources/old.json'))).toBe(false);
    expect(readFileSync(path.join(install, 'resources/new.json'), 'utf8')).toBe('new');
    expect(readFileSync(path.join(install, '.ghostex-appimage'), 'utf8')).toBe('7.9.0-bbbb\n');
  });

  test('an interrupted copy is finished on the next launch', () => {
    scratch = mkdtempSync(path.join(tmpdir(), 'ghostex-apprun-'));
    const install = path.join(scratch, 'data/ghostex/appimage');
    mkdirSync(path.join(install, 'gxserver/bin'), { recursive: true });
    writeFileSync(path.join(install, 'gxserver/bin/gxserver'), 'half-copied');
    launch(appDir('v1', '7.8.0-aaaa', { 'gxserver/bin/gxserver': 'v1' }));
    expect(readFileSync(path.join(install, 'gxserver/bin/gxserver'), 'utf8')).toBe('v1');
  });
});
