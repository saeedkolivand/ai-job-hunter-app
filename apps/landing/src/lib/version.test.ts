import { describe, expect, it } from 'vitest';

import { buildInstallers, isNewer } from './version';

describe('isNewer', () => {
  it('detects a higher patch, minor, or major', () => {
    expect(isNewer('0.127.1', '0.127.0')).toBe(true);
    expect(isNewer('0.128.0', '0.127.9')).toBe(true);
    expect(isNewer('1.0.0', '0.999.999')).toBe(true);
  });

  it('is false for an equal or older candidate', () => {
    expect(isNewer('0.127.0', '0.127.0')).toBe(false);
    expect(isNewer('0.126.9', '0.127.0')).toBe(false);
    expect(isNewer('0.127.0', '0.127.1')).toBe(false);
  });

  it('tolerates a leading v and a pre-release suffix', () => {
    expect(isNewer('v0.128.0', '0.127.0')).toBe(true);
    expect(isNewer('0.127.0-rc.1', '0.127.0')).toBe(false);
    expect(isNewer('0.128.0-rc.1', 'v0.127.0')).toBe(true);
  });
});

describe('buildInstallers', () => {
  it('pins every per-OS asset URL to the version', () => {
    const i = buildInstallers('0.127.0');
    expect(i.macArm).toBe(
      'https://github.com/saeedkolivand/ai-job-hunter-app/releases/download/v0.127.0/macos-AI-Job-Hunter_0.127.0_aarch64-apple-silicon.dmg'
    );
    expect(i.macIntel).toContain('macos-AI-Job-Hunter_0.127.0_x64-intel.dmg');
    expect(i.winExe).toContain('windows-AI-Job-Hunter_0.127.0_x64-setup.exe');
    expect(i.winMsi).toContain('windows-AI-Job-Hunter_0.127.0_x64_en-US.msi');
    expect(i.linuxAppImage).toContain('linux-AI-Job-Hunter_0.127.0_amd64.AppImage');
    expect(i.linuxDeb).toContain('linux-AI-Job-Hunter_0.127.0_amd64.deb');
    expect(i.linuxRpm).toContain('linux-AI-Job-Hunter-0.127.0-1.x86_64.rpm');
  });
});
