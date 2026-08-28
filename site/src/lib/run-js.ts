export type RunResult =
  | { ok: true; output: string }
  | { ok: false; error: string }

/**
 * Run a program's emitted JavaScript in the page.
 *
 * The emitted code is written for node: an `input` program reads stdin whole through
 * `require("fs").readFileSync` and decodes it itself with `JSON.parse`; a `lines` program reads
 * it incrementally through `fs.readSync` on fd 0, in fixed-size `Buffer` chunks. Both are faked
 * here rather than the emitted code being rewritten to suit a browser, so what you read in the
 * JavaScript tab is exactly what ran.
 *
 * `stdinText` is raw text either way -- JSON text for an `input` program, or the lines
 * themselves for a `lines` program -- and it is up to the emitted code to decide what to do
 * with it, exactly as node would.
 *
 * `console.log` appends a newline, and the Rust harness captures the process's stdout, so
 * joining with newlines is what makes this comparable to the recorded output.
 */
export function runJs(code: string, stdinText: string | null): RunResult {
  const lines: string[] = []
  const console = { log: (value: unknown) => lines.push(String(value)) }

  // One instance, however often it is asked for: node caches modules, and a `lines` program's
  // read loop calls `require("fs")` once per line. A fresh `readSync` per call would reset the
  // stdin cursor each time, and the loop would re-read the same input forever.
  const fs = { readFileSync: () => stdinText ?? "", readSync: makeReadSync(stdinText ?? "") }
  const require = (name: string) => {
    if (name !== "fs") throw new Error(`the emitted code asked for an unexpected module: ${name}`)
    return fs
  }

  try {
    new Function("require", "console", "Buffer", code)(require, console, FakeBuffer)
    return { ok: true, output: lines.map((line) => `${line}\n`).join("") }
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) }
  }
}

/**
 * A byte-oriented, cursor-tracking `fs.readSync(fd, buffer, offset, length, position)`, reading
 * from `text` encoded as UTF-8 -- the same encoding a real file on disk would be read as bytes
 * from, which is what makes chunk boundaries behave the same way here as they would for node.
 *
 * Known gap, not fixed here: a multi-byte UTF-8 character that falls exactly on a chunk boundary
 * decodes incorrectly, in both this shim and the emitted code's own chunk-by-chunk decoding --
 * neither carries a partial character over to the next chunk. Every corpus fixture is small
 * enough that no chunk boundary is ever reached, so this is latent rather than observed.
 */
function makeReadSync(text: string) {
  const bytes = new TextEncoder().encode(text)
  let pos = 0
  return (
    _fd: number,
    buffer: FakeBuffer,
    offset: number,
    length: number,
    _position: number | null,
  ) => {
    const n = Math.min(length, bytes.length - pos)
    if (n <= 0) return 0
    buffer.bytes.set(bytes.subarray(pos, pos + n), offset)
    pos += n
    return n
  }
}

/**
 * A minimal stand-in for node's `Buffer`, covering only what the emitted code calls: `alloc`
 * to create one, and `toString("utf8", start, end)` to decode a range back to text. Backed by a
 * plain `Uint8Array`, which exists in a browser with no polyfill needed.
 */
class FakeBuffer {
  bytes: Uint8Array

  constructor(size: number) {
    this.bytes = new Uint8Array(size)
  }

  static alloc(size: number): FakeBuffer {
    return new FakeBuffer(size)
  }

  get length(): number {
    return this.bytes.length
  }

  toString(_encoding: string, start: number, end: number): string {
    return new TextDecoder().decode(this.bytes.subarray(start, end))
  }
}
