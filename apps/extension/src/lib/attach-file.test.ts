/**
 * `lib/attach-file.ts` tests.
 *
 * jsdom implements neither `DataTransfer` nor a settable
 * `HTMLInputElement.files` (both are real-browser-only) — this file installs
 * the MINIMAL polyfill {@link attachResumeFile} needs to run: a fake
 * `DataTransfer` with a `.items.add`/`.files` pair, and a settable `files`
 * property on `HTMLInputElement.prototype`. This proves the module's OWN
 * logic — candidate-finding, the disabled/ambiguous refusals, the
 * fail-closed re-read verification, the synthetic drop — not that jsdom's
 * (nonexistent) native assignment works. The load-bearing claim that a REAL
 * browser's `input.files = dataTransfer.files` assignment sticks is verified
 * separately, against real Chromium, in `attach-file.chromium.test.ts`
 * (sibling file — see its doc for the resolvable/installed skip guards).
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { attachResumeFile } from './attach-file';

class FakeFileList extends Array<File> {
  item(i: number): File | null {
    return this[i] ?? null;
  }
}

class FakeDataTransfer {
  private readonly stored: File[] = [];
  readonly items = {
    add: (file: File): void => {
      this.stored.push(file);
    },
  };
  get files(): FileList {
    return new FakeFileList(...this.stored) as unknown as FileList;
  }
}

let filesDescriptor: PropertyDescriptor | undefined;

function installFilesPolyfill(): void {
  const backing = new WeakMap<HTMLInputElement, FileList>();
  Object.defineProperty(HTMLInputElement.prototype, 'files', {
    configurable: true,
    get(this: HTMLInputElement) {
      return backing.get(this) ?? (new FakeFileList() as unknown as FileList);
    },
    set(this: HTMLInputElement, value: FileList) {
      backing.set(this, value);
    },
  });
}

beforeEach(() => {
  filesDescriptor = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'files');
  installFilesPolyfill();
  vi.stubGlobal('DataTransfer', FakeDataTransfer);
});

afterEach(() => {
  if (filesDescriptor) {
    Object.defineProperty(HTMLInputElement.prototype, 'files', filesDescriptor);
  }
  vi.unstubAllGlobals();
  document.body.innerHTML = '';
});

function bytesOf(text: string): Uint8Array {
  return new TextEncoder().encode(text);
}

describe('attachResumeFile', () => {
  it('attaches to a résumé file input, dispatches input+change, and confirms via re-read', () => {
    document.body.innerHTML = '<input type="file" name="resume" accept=".pdf,.docx">';
    const input = document.querySelector('input')!;
    const inputSpy = vi.fn();
    const changeSpy = vi.fn();
    input.addEventListener('input', inputSpy);
    input.addEventListener('change', changeSpy);

    const bytes = bytesOf('%PDF-1.4 fake');
    const result = attachResumeFile(document, bytes, 'resume.pdf', 'application/pdf');

    expect(result).toEqual({
      attached: true,
      filename: 'resume.pdf',
      byteLength: bytes.byteLength,
    });
    expect(inputSpy).toHaveBeenCalledTimes(1);
    expect(changeSpy).toHaveBeenCalledTimes(1);
  });

  it('fails closed when no résumé file input exists on the page', () => {
    document.body.innerHTML = '<input type="text" name="email">';
    const result = attachResumeFile(document, bytesOf('x'), 'resume.pdf', 'application/pdf');
    expect(result.attached).toBe(false);
    expect(result.reason).toMatch(/no résumé/i);
  });

  it('fails closed (never guesses) when more than one résumé file input is found', () => {
    document.body.innerHTML =
      '<input type="file" name="resume1" accept=".pdf">' +
      '<input type="file" name="resume2" accept=".pdf">';
    const result = attachResumeFile(document, bytesOf('x'), 'resume.pdf', 'application/pdf');
    expect(result.attached).toBe(false);
    expect(result.reason).toMatch(/more than one/i);
  });

  it('fails closed on a disabled résumé field', () => {
    document.body.innerHTML = '<input type="file" name="resume" accept=".pdf" disabled>';
    const result = attachResumeFile(document, bytesOf('x'), 'resume.pdf', 'application/pdf');
    expect(result.attached).toBe(false);
    expect(result.reason).toMatch(/disabled/i);
  });

  it('fails closed when the re-read does not confirm the assignment (a custom widget silently dropped it)', () => {
    document.body.innerHTML = '<input type="file" name="resume" accept=".pdf">';
    // Simulate a widget/browser quirk that intercepts the setter and never
    // actually stores the FileList — the assignment "succeeds" with no error
    // but a re-read finds nothing.
    Object.defineProperty(HTMLInputElement.prototype, 'files', {
      configurable: true,
      get: () => new FakeFileList() as unknown as FileList,
      set: () => {
        /* silently dropped */
      },
    });

    const result = attachResumeFile(document, bytesOf('x'), 'resume.pdf', 'application/pdf');
    expect(result.attached).toBe(false);
    expect(result.reason).toMatch(/could not confirm/i);
  });

  it('dispatches a synthetic drop on a detectable drop-zone ancestor, carrying the same DataTransfer', () => {
    document.body.innerHTML =
      '<div class="upload-dropzone"><input type="file" name="resume" accept=".pdf"></div>';
    const zone = document.querySelector('.upload-dropzone')!;
    let seenDataTransfer: unknown;
    zone.addEventListener('drop', (e) => {
      seenDataTransfer = (e as Event & { dataTransfer?: unknown }).dataTransfer;
    });

    attachResumeFile(document, bytesOf('x'), 'resume.pdf', 'application/pdf');

    expect(seenDataTransfer).toBeInstanceOf(FakeDataTransfer);
  });

  it('never dispatches a drop when no drop-zone ancestor is detectable', () => {
    document.body.innerHTML = '<div><input type="file" name="resume" accept=".pdf"></div>';
    const dropSpy = vi.fn();
    document.addEventListener('drop', dropSpy);

    attachResumeFile(document, bytesOf('x'), 'resume.pdf', 'application/pdf');

    expect(dropSpy).not.toHaveBeenCalled();
  });
});
