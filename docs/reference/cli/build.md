# toylang build

`toylang build FILE` with no backend named compiles the program to LLVM IR, links it into a
native executable in the current directory named after the file's stem, and prints the path it
wrote. Spelling the backend, `toylang build FILE llvm`, does the same thing.

```
$ toylang build greet_fn.toy
greet_fn
$ ./greet_fn
hello world
```

(`greet_fn.toy` is `examples/greet_fn.toy`.) The binary is a dynamically linked ELF
executable of about 44 KB. Any backend other than `js` is refused with a message naming it:
`build only links native (or emits js); Lua has no build step`.

## `js`: the script and its declarations

`toylang build FILE js` writes `<stem>.js` and `<stem>.d.ts` side by side in the current
directory and prints both names. The `.js` is exactly what `toylang emit FILE js` prints, so
it honors [toylang.conf.yaml](config.md) the same way. The `.d.ts` is what `build` adds: the
program's functions, at the names and runtime shapes the JS backend gives them, as TypeScript
declarations. This program exercises every shape it can name:

```toylang
enum Shape { Point, Circle { r: Int } }

fn area_ish(s: Shape) -> Int =
  s | Circle { r } -> r * r or Point -> 0

fn greet(who: Str) -> Str = "hello " + who

fn total(v: Vec<Int>) -> Int = sum v

fn bump(x: Opt<Int>) -> Opt<Int> = x

fn area(r: { w: Int, h: Int }) -> Int = r.w * r.h

fn big(x: Int64) -> Int64 = x

fn positive(x: Int) -> Result<Int, Str> =
  x | . > 0 -> ok(.) or err "no"

fn half(x: Float) -> Float = x / 2.0

{
  a: area_ish(circle { r: 3 }),
  g: greet "bob",
  t: total([1, 2]),
  b: bump(some 5),
  r: area { w: 2, h: 3 },
  i: big(i64 7),
  o: positive 1,
  h: half 3.0
}
```

```output
{"a":9,"g":"hello bob","t":3,"b":5,"r":6,"i":7,"o":{"Ok":1},"h":1.5}
```

```
$ toylang build shapes.toy js
shapes.js
shapes.d.ts
$ node shapes.js
{"a":9,"g":"hello bob","t":3,"b":5,"r":6,"i":7,"o":{"Ok":1},"h":1.5}
$ cat shapes.d.ts
export type Shape = "Point" | { Circle: { r: number } };
export type Opt_Int = { Some: number } | "None";
export type Result_Int_Str = { Ok: number } | { Err: string };
export function v_area_ish(x: Shape): number;
export function v_greet(x: string): string;
export function v_total(x: Array<number>): number;
export function v_bump(x: Opt_Int): Opt_Int;
export function v_area(x: { w: number, h: number }): number;
export function v_big(x: bigint): bigint;
export function v_positive(x: number): Result_Int_Str;
export function v_half(x: number): number;
```

One `export function` per function the program's body reaches, at the name the `.js` defines
it under (`v_` in front of the source name) and with the one parameter always called `x`. A
function nothing calls is pruned by the checker before any backend runs, so it appears in
neither file. Before the functions, one `export type` alias per enum a signature mentions,
named as the JS backend names the type (`Opt_Int`, `Result_Int_Str`: the arguments are
embedded because `Opt<Int>` and `Opt<Str>` are distinct types) and defined as the union of the
runtime shapes an enum value takes
([ADR 0009](../../adr/0009-enums-are-json-native-single-key-wrappers.md)): a unit variant is
its name as a string, a payload variant a single-key object. The variant keeps its declared
spelling, `Circle`, not the `circle` the program constructs it with.

The scalar mappings follow what the emitted code holds at runtime: `Int`, `Float`, and `Char`
are all `number` (a `Char` is its codepoint), `Int64` is `bigint`, `Str` is `string`, `Bool`
is `boolean`. `Vec<T>` and `Stream<T>` are both `Array<T>`, since a stream reaching a function
boundary has been materialized. A record is a structural object type with its fields, and a
`Sink` is `void`. Neither target of the JS backend changes any of this, so the `.d.ts` is the
same file whichever one [toylang.conf.yaml](config.md) selects.

## Checking a consumer against it

A TypeScript file importing the module by its stem resolves the `.d.ts`, and under `strict`
every call is checked against the declared signatures. This consumer has two mistakes in it:

```
$ cat consumer.ts
import { v_greet, v_total, v_bump, v_area_ish, v_positive, Opt_Int, Shape } from "./shapes";

const g: string = v_greet("bob");
const t: number = v_total([1, 2, 3]);
const b: Opt_Int = v_bump({ some: 5 });
const s: Shape = { circle: { r: 3 } };
const a: number = v_area_ish(s);
const o = v_positive(1);
const wrong: string = v_total([1]);
$ cat tsconfig.json
{
  "compilerOptions": { "target": "es2022", "module": "esnext", "moduleResolution": "bundler", "strict": true, "noEmit": true },
  "files": ["consumer.ts"]
}
$ tsc -p tsconfig.json
consumer.ts(6,20): error TS2561: Object literal may only specify known properties, but 'circle' does not exist in type '{ Circle: { r: number; }; }'. Did you mean to write 'Circle'?
consumer.ts(9,7): error TS2322: Type 'number' is not assignable to type 'string'.
```

With those two lines corrected, `tsc` exits 0. That is the whole of what the declarations buy
today. The `.js` itself has no `export` statements: it defines the functions at top level and
then runs the program, so importing it under node 24 printed the program's output and handed
back nothing (an ESM `import * as m` saw only node's interop keys, `require` an empty object).
Calling the declared functions from another module at runtime is not something the emitted
file arranges yet; the `.d.ts` is the type surface for a checker and an editor.
