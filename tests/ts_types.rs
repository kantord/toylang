//! The `.d.ts` the JS backend emits beside its `.js`, and the TypeScript consumer that uses it.
//!
//! `emit_dts` is a pure function, pinned directly below with no toolchain in the way; the
//! `tsc` check at the bottom is the real gate, against an actual compiler, and requires a
//! TypeScript compiler to be installed -- on `PATH` as `tsc`, in `site/node_modules` (which
//! `pnpm install` in site/ provides), or named by `$TSC`. A missing one is an error rather
//! than a skipped check, the same rule the subprocess backends follow for node, go, and cc.

/// Exercises every shape the .d.ts has to name: a typed function signature for int, str, bool,
/// a Vec, a record, and a generic enum, all called from the body so the checker keeps them.
const PROGRAM: &str = r#"fn add(x: Int) -> Int = x + 1
fn greet(x: Str) -> Str = "hi " + x
fn is_pos(x: Int) -> Bool = x >  0
fn total(v: Vec<Int>) -> Int = sum(v)
fn area(r: {w: Int, h: Int}) -> Int = r.w * r.h
fn bump(x: Opt<Int>) -> Opt<Int> = x

{
  a: add(1),
  g: greet("bob"),
  p: is_pos(2),
  t: total([1,  2,  3]),
  r: area({w:  2, h:  3}),
  b: bump(some(5))
}
"#;

/// A consumer that imports the emitted module and uses every export in a type-correct way: this is
/// what the done-gate's `tsc --noEmit` runs against.
const CONSUMER: &str = r#"import { v_add, v_greet, v_is_pos, v_total, v_area, v_bump, Opt_Int } from "./prog";

const a: number = v_add(1);
const b: string = v_greet("bob");
const c: boolean = v_is_pos(2);
const d: number = v_total([1,  2,  3]);
const e: number = v_area({ w:  2, h:  3 });
const f: Opt_Int = v_bump({ some:  5 });
"#;

/// `strict: true` is the point: it is what makes the consumer's usage actually type-checked
/// rather than being accepted as `any`.
const TSCONFIG: &str = r#"{
  "compilerOptions": {
    "target": "es2022",
    "module": "esnext",
    "moduleResolution": "bundler",
    "strict": true,
    "noEmit": true
  },
  "files": ["consumer.ts"]
}
"#;

#[test]
fn emitted_dts_declares_the_module_shape() {
    let program = toylang::compile(PROGRAM).expect("compiles");
    insta::assert_snapshot!(toylang::emit_js::emit_dts(&program));
}

#[test]
fn tsc_accepts_the_declaration_and_consumer() {
    let program = toylang::compile(PROGRAM).expect("compiles");
    let js = toylang::emit_js::emit(&program, toylang::emit_js::JsTarget::Node)
        .expect("the js backend emits");
    let dts = toylang::emit_js::emit_dts(&program);

    let dir = tempfile::tempdir().expect("temp dir");
    let stem = dir.path().join("prog");
    std::fs::write(stem.with_extension("js"), js).expect("write js");
    std::fs::write(stem.with_extension("d.ts"), dts).expect("write d.ts");
    std::fs::write(dir.path().join("consumer.ts"), CONSUMER).expect("write consumer");
    std::fs::write(dir.path().join("tsconfig.json"), TSCONFIG).expect("write tsconfig");

    let out = std::process::Command::new(find_tsc())
        .arg("-p")
        .arg(dir.path().join("tsconfig.json"))
        .output()
        .expect("could not run tsc");
    assert!(
        out.status.success(),
        "tsc rejected the emitted declarations:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

/// Locate a TypeScript compiler: `$TSC`,the site's installed one (what a `pnpm install`
/// in site/ leaves in the gitignored node_modules there), or a `tsc` on PATH. The site's copy is
/// preferred: its version is pinned by pnpm-lock, where a global one is whoever installed it.
fn find_tsc() -> String {
    if let Ok(path) = std::env::var("TSC") {
        return path;
    }
    let site_tsc =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("site/node_modules/.bin/tsc");
    if site_tsc.is_file() {
        return site_tsc.display().to_string();
    }
    // A bare name resolves through PATH, so the only reliable presence test is running it: an
    // `is_file` on the name would look for a relative file rather than an executable.

    if std::process::Command::new("tsc")
        .arg("--version")
        .output()
        .is_ok()
    {
        return "tsc".to_string();
    }
    panic!(
        "tsc is not installed, and the .d.ts gate needs a real TypeScript compiler: install one \
         (e.g. `npm install -g typescript`, or `pnpm install` in site/), or point $TSC at one"
    );
}
