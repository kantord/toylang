# Function-signature matching syntax: how the three spellings hold up across type shapes

The deeper-research legwork for gh:152 (round answer, signature-matching-and-search-cut
round).. Round 3 stress-tested the three candidate spellings against the real parser on the enum
example and re-asked;the maintainer wanted the same across structs/records, tuples, scalars,
nested matches, and recursive/self-referential types. This run probed each spelling over every
shape the language actually has, and the verdicts below are what the real parser and checker said.

The probe programs live in `scratch/`, run through `toylang::fmt` + `toylang::compile` (parse
and check only, no codegen was attempted),both untracked in this worktree. One shape in the required list does not exist: **there is no tuple
type.** The language's product type is the record (`{x: Int, y: Int}`),and the record probes
below are what stand in for the tuple dimension. Probing an anonymous tuple would mean inventing a
type form first, which is a different question from the spelling one. 

## The baseline: every shape compiles today

All six baseline programs -- enum, record, scalar, vec, nested, recursive -- parse and check
clean. Each is a named param plus an explicit `|` subject;the subject is spelled out everywhere
the language currently lets you match:

```toylang
enum Shape { point, circle{r: Int} }
fn area(s: Shape) -> Int = s | circle{r} -> r * r or point ->  0

area(Shape.point)
```

The record one destructures through guard projections (`p |.x > .y -> .x or .y`),the vec
through `select`, the nested through a payload arm then a second match, and the recursive through a
`sum` over a `map` that calls the function again. All compile as written. They are the floor the
three spellings are measured against. A and B never reach type shape: their failures fire at
parse and at the subject check, before any shape-dependent reasoning, so their probes are mostly
enum-shaped. C is the only spelling whose probes parse as written, so it is the one the shape sweep
actually differentiates. 

## Candidate A: call-form hoist

`fn area = Shape(circle{r} -> r * r or point ->  0)` -- the function is a call-form match:type
name as head, arms as arguments, no param list, no return annotation. The maintainer's sketch
was `fn render = Msg(Ping -> "*ping*" or Quit -> "*quit*" or Text -> .body)`. 

**The signature shape is not parseable.** The fn grammar requires
`fn name(param: Type) -> Type = body` (parse.rs:585-622:the parens, the param name,
the return annotation, each mandatory). Every A probe dies at the same byte:

```toylang
enum Shape { point, circle{r: Int} }
fn area = Shape(circle{r} -> r * r or point ->  0)

area(Shape.point)
```

`a_enum`, `a_nested` (`Wrapped(w -> Shape(circle{r} -> ... or ...))` as head),and `a_vec`
(`Vec<Int>(any() -> . | select(. >= 2))` as head) all die identically: "expected `(`, found
`=`". The shape being matched -- enum, nested enum, vec -- never reaches the parser;the wall
is the fn declaration shape itself. So A is not a spelling of the existing fn signature;it is a
new fn-declaration form. And the maintainer sketch omits the return type too, so the form also
needs return-type inference (or a return position A does not spell)erto work.

**The call-form head means nothing as an expression either.** `a_inner` keeps a normal signature
and writes the call-form as a body expression, to test the head independently of the fn-grammar
question:

```toylang
enum Shape { point, circle{r: Int} }
fn area(s: Shape) -> Int = Shape(circle{r} -> r * r or point ->  0)

area(Shape.point)
```

`a_inner` parses,and the checker answers "`Shape` is not a function". Enum construction is
`Shape.circle` or the bare `circle{r: 1}` (enums.md),not `Shape(...)`;the application path
only resolves functions and variant constructors (check/mod.rs:1883-1891). So A's head is a new
expression form -- "a type name used as a match-call" -- not a reuse of anything. For the vec
shape the head would additionally be a generic type-name call (`Vec<Int>(...)`),even further from any
existing application, but it never gets far enough to test that. A's nested probe was written to
see whether arms compose head-in-head, and it dies at the same grammar wall, so nesting behavior
under A is unobserved. Record, scalar, and tuple-shaped heads would hit the same two walls:the
head is uniformly a type name, and both failures fire before type resolution. Nothing shape-
specific differentiates A.

## Candidate B: anonymous param + bare match

Two probes isolate the two halves, because each half fails on its own:

**Anonymous `: Type` is not parseable.**

```toylang
enum Shape { point, circle{r: Int} }
fn area(: Shape) -> Int = s | circle{r} -> r * r or point ->  0

area(Shape.point)
```

`b_param` (and `b_recursive`) die at the param: "expected a name, found `:`" (parse.rs:594).
The fn grammar demands an identifier before the colon. Dropping the name is a parse-level grammar
change, independent of everything downstream. 

**A bare match body has no subject.**

```toylang
enum Shape { point, circle{r: Int} }
fn area(s: Shape) -> Int = circle{r} -> r * r or point ->  0

area(Shape.point)
```

`b_body` parses fine,and the checker answers "a match needs a subject, so it must follow `|`"
(check/mod.rs:1300). A match chain's subject arrives through a pipe -- each arm body stops at `|`
and `or`, which is what lets the chain be one pipe stage (match.md:3; parse.rs:898). A bare body
presents no pipe stage, so the match has no subject to check against. B needs the checker to accept
a subject-less match at body top and treat the input as that subject -- the same body-top-binding idea
C needs, but without spelling it. The recursive case (`b_recursive_body`, `num -> . or arr ->
sum(...)`) hits the identical wall;recursion adds nothing shape-specific for B either. Both B
walls fire before the subject's type is examined, so record, scalar, vec, and nested shapes
would fail identically;no further probes were needed there. 

The bare-subject philosophy already has a precedent against it: **the language refuses bare function
calls in pipe stages.** `xs | sum` errors "`sum` is a function, not a value; write `sum(...)`
to call it", while `xs | sum(.)` compiles (`probe_sum_bare` vs `probe_sum_parens`). The subject
is explicit everywhere a value enters a stage;a bare match would be the first subject-less
construct. 

## Candidate C: additive `.`-binding

```toylang
enum Shape { point, circle{r: Int} }
fn area(s: Shape) -> Int = . | circle{r} -> r * r or point ->  0

area(Shape.point)
```

`. ` rebinds at each boundary that introduces a subject: a pipe stage, a `map` or `select` body,
a match arm over a bare payload (pipe.md:29). C's proposal is one more boundary:the fn body top
binds `.` to the input param, so the whole function is the match chain over its one input,with the
signature unchanged. 

All six C probes -- enum, record, scalar, vec, nested, recursive -- parse clean;every one
fails at the same single check: "`.` is not bound here" at the body's first `.` (check/mod.rs:1507).
The fn body top is not currently a `.`-binding boundary, so `Expr::Subject` finds no subject. Each
C program is the corresponding baseline program with exactly the `s |` / `p |` / `n |` / `xs |` /
`x |` / `j |` prefix replaced by `. |`;nothing downstream differs, and every baseline compiles.

So C is one additive change -- bind `.` to the input at body top -- with no grammar surface,and no
shape-specific interaction in any of the probed shapes. The inner `.`-uses (match arms, `select`,
`map`, `sum(.)`) already work today;each probed body's remainder is the compiling baseline's, so
nothing downstream of the leading `.` remains to fail once the binding exists. 

**The one wrinkle found: recursion wants the reduce inside the arm.** `probe_rec_explicit` moved the
recursion out of the arm chain, keeping the explicit `j |` subject so the `.`-uses inside lambdas
could be tested independently of C's top-level binding:

```toylang
enum Json { arr(Vec<Json>), num(Int) }
fn total(j: Json) -> Int = j | num -> . or arr -> . | map(. | total(.)) | sum(.)
```

It fails type-checking: "expected Int, found Vec<Json>" at the `arr` arm's body `.` -- match arms
each get checked against the declared `-> Int`, so an arm may not hand the payload onward for a later
stage to reduce. The recursion must stay inside the arm (`arr -> sum(. | map(. | total(.)))`),
exactly as the baseline spells it. That constraint applies to whichever spelling wins -- B's
bare-match recursion dies one step earlier at the subject wall,and C's recursion has nothing else
between it and the baseline. 

## What each candidate would cost

- **A**: a new fn-declaration shape (or optional parens and return annotation), return-type
  inference, and a type-name-as-match-call expression form. Three novel surfaces, all hit before
  type shape can differentiate anything.
- **B**: anonymous params (parse grammar) plus subject-less match chains at body top (checker
  rule). Two independent changes;neither shape-specific;and the bare subject fights the language's
  explicitness convention.

- **C**: one additive boundary in `.`-rebinding. No grammar change;no shape-specific interaction
  found;and it is the only spelling whose probed programs parse as written today.