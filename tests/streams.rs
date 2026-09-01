//! Streaming input, in its most minimal cut: `lines`, `collect`, and the rules that keep a
//! single-use stream from ending up somewhere it cannot be honoured.
//!
//! What every backend agrees on when it runs lives in the corpus, as `lines_cat.yaml` and its
//! siblings. This file holds the claims a corpus case cannot express: that a program is
//! refused, and why.

#[track_caller]
fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

/// The containment bans: a stream cannot end up stored inside a Vec, a record, an enum payload,
/// or another stream, because nothing can get a single-use value back out of one once it is in.
mod containment {
    use super::err;

    /// Nothing can get a `Lines` value back out of a Vec once it is in one, and there is only
    /// ever one real stdin to hold in the first place.
    #[test]
    fn lines_cannot_enter_a_vec() {
        insta::assert_snapshot!(err("str(1) | [lines]"));
    }

    /// Same reasoning as the Vec case, for a record field.
    #[test]
    fn lines_cannot_enter_a_record() {
        insta::assert_snapshot!(err("{a: lines}"));
    }

    /// The containment bans hold in the type grammar itself, not just at value construction
    /// sites: an annotation cannot describe a stream as stored in a Vec.
    #[test]
    fn a_signature_cannot_put_a_stream_in_a_vec() {
        insta::assert_snapshot!(err("fn f(v: Vec<Stream<Str>>) -> Int = 0\n\n1"));
    }

    /// Same ban for a record field, spelled in a signature.
    #[test]
    fn a_signature_cannot_put_a_stream_in_a_record() {
        insta::assert_snapshot!(err("fn f(r: {s: Stream<Str>}) -> Int = 0\n\n1"));
    }

    /// And for an enum variant's payload, the one other annotation a value constructor reads.
    #[test]
    fn an_enum_payload_cannot_hold_a_stream() {
        insta::assert_snapshot!(err("enum E { v{s: Stream<Str>} }\n\n1"));
    }

    /// The parens spelling puts a type directly in payload position, so the ban has to hold
    /// there too, not only inside a record.
    #[test]
    fn a_scalar_enum_payload_cannot_be_a_stream() {
        insta::assert_snapshot!(err("enum E { v(Stream<Str>) }\n\n1"));
    }

    /// A stream of streams has nothing it could yield: its entries would not be values.
    #[test]
    fn a_stream_cannot_hold_another_stream() {
        insta::assert_snapshot!(err("fn f(s: Stream<Stream<Str>>) -> Int = 0\n\n1"));
    }
}

/// The exactly-once rule: a stream-typed binding must be consumed exactly once, on every path,
/// outside a mapper body, so that fusion always knows its pipeline's shape at compile time.
mod linearity {
    use super::err;

    /// A stream has nothing to print: only `collect` turns it into a value. Caught for the
    /// program's own result, which has no annotation to check against the way a function's
    /// return type does.
    #[test]
    fn lines_cannot_be_the_programs_result() {
        insta::assert_snapshot!(err("lines"));
    }

    /// `Stream` is spellable in a signature now -- the one thing the `Lines` design deliberately
    /// withheld -- so a user function can take the stream and consume it itself.
    #[test]
    fn a_stream_signature_checks_end_to_end() {
        assert!(
            toylang::compile("fn f(s: Stream<Str>) -> Vec<Str> = collect(s)\n\nf(lines)").is_ok()
        );
    }

    /// Zero uses is an error: linear, not affine. Exactly-once can relax to at-most-once later
    /// without breaking a program, while the reverse tightening would break every program that
    /// dropped a stream.
    #[test]
    fn a_stream_parameter_must_be_consumed() {
        insta::assert_snapshot!(err("fn f(s: Stream<Str>) -> Int = 0\n\n1"));
    }

    /// Two uses is the Python-generator mistake the single-use rule exists to prevent: the
    /// second pass over an already-consumed iterator is silently empty.
    #[test]
    fn a_stream_parameter_cannot_be_consumed_twice() {
        insta::assert_snapshot!(err(
            "fn f(s: Stream<Str>) -> Int = length(collect(s)) + length(collect(s))\n\n1"
        ));
    }

    /// `|` is the one construct that can silently drop its left side, so a stream piped into an
    /// expression gets the same exactly-once rule a stream-typed parameter does.
    #[test]
    fn a_piped_stream_must_be_consumed() {
        insta::assert_snapshot!(err("lines | 0"));
    }

    /// The generalization of "contains `lines`, nothing to print": any bare unconsumed stream as
    /// the program's result, here one arriving through a user-written stream signature.
    #[test]
    fn a_program_cannot_result_in_a_bare_stream() {
        insta::assert_snapshot!(err(
            "fn noisy(s: Stream<Str>) -> Stream<Str> = s | map(. + \"!\")\n\nnoisy(lines)"
        ));
    }

    /// The ternary was retired (kantord/toylang#155) in favor of guard arms; the stream rule
    /// it would have exercised lives on as `a_match_cannot_yield_a_stream` below. The old
    /// spelling no longer parses -- `if` is an ordinary identifier, so `s if ...` reads as a
    /// cross-line call.
    #[test]
    fn a_conditional_cannot_yield_a_stream() {
        insta::assert_snapshot!(err(
            "fn f(s: Stream<Str>) -> Stream<Str> = s if 1 == 1 else s\n\n1"
        ));
    }

    /// The same rule for a match's arms.
    #[test]
    fn a_match_cannot_yield_a_stream() {
        insta::assert_snapshot!(err(
            "enum E { a, b }\n\nfn f(s: Stream<Str>) -> Stream<Str> = a | (a -> s or b -> s)\n\n1"
        ));
    }

    /// The elements of a map's result are stored, and a stream is not storable: the same
    /// containment ban a Vec literal enforces, met before the Vec of streams could exist.
    #[test]
    fn a_map_body_cannot_be_a_stream() {
        insta::assert_snapshot!(err(
            "fn f(s: Stream<Str>) -> Int = length([1] | map(s))\n\n1"
        ));
    }

    /// One spelled consumption, many runtime ones: a mapper's body runs once per element, so a
    /// stream consumed there would be drained by the first element and empty for every later
    /// one.
    #[test]
    fn a_stream_cannot_be_consumed_inside_a_mapper() {
        insta::assert_snapshot!(err(
            "fn f(s: Stream<Str>) -> Vec<Int> = [1] | map(length(collect(s)))\n\n1"
        ));
    }

    /// A source read beside an unrelated piped value is not one chain either.
    #[test]
    fn a_pipes_stream_must_flow_in_from_its_left() {
        insta::assert_snapshot!(err("1 | (lines | map(. + \"!\"))"));
    }
}

/// The source rules: there is only one real stdin, so `lines`, `input`, and `inputs` cannot
/// coexist or repeat, and a source is legal only in the program's own body -- never inside a
/// mapper or a `fn`, both of which could run it more than once.
mod sources {
    use super::err;

    /// There is only one real stdin, so a second read is refused at compile time rather than
    /// silently handed nothing back, the way Python's own generators are.
    #[test]
    fn lines_cannot_be_read_twice() {
        insta::assert_snapshot!(err("[collect(lines), collect(lines)]"));
    }

    /// Forced by jq specifically: raw-input mode, needed for `collect` to read lines rather than
    /// JSON, changes what the whole invocation means, so it cannot coexist with `input` being
    /// parsed as a JSON document in the same run. Verified against real jq before this rule was
    /// added: `jq -Rn '[inputs]'` and ordinary `.`-is-the-document mode are mutually exclusive.
    #[test]
    fn input_and_lines_cannot_both_be_used() {
        insta::assert_snapshot!(err(
            "fn f(x: Int) -> Int = x\n\nf(input) + (collect(lines) | 0)"
        ));
    }

    /// `inputs` reads the same real stdin `input` does, eagerly, so the two cannot coexist any
    /// more than `input` and `lines` can.
    #[test]
    fn input_and_inputs_cannot_both_be_used() {
        insta::assert_snapshot!(err(
            "fn f(x: Int) -> Int = x\nfn g(x: Vec<Int>) -> Int = length(x)\n\nf(input) + g(collect(inputs))"
        ));
    }

    /// Forced by jq specifically, the same way `input`/`lines` is: raw-input mode for `lines`
    /// and parsed-JSON mode for `inputs` are different invocation-wide flags, so one jq process
    /// cannot run a program that asks for both.
    #[test]
    fn lines_and_inputs_cannot_both_be_used() {
        insta::assert_snapshot!(err(
            "fn g(x: Vec<Int>) -> Int = length(x)\n\n(collect(lines) | 0) + g(collect(inputs))"
        ));
    }

    /// Like `input`, `inputs` has no type of its own until it is checked against one.
    #[test]
    fn inputs_needs_a_position_to_check_against() {
        insta::assert_snapshot!(err("inputs"));
    }

    /// A second `inputs` would be a second stream claiming the same real stdin, refused exactly
    /// the way a second `lines` is. This also retires the old element-type-agreement rule: with
    /// one use, there is nothing left to disagree.
    #[test]
    fn inputs_cannot_be_read_twice() {
        insta::assert_snapshot!(err(
            "fn f(s: Stream<Int>) -> Vec<Int> = collect(s)\n\nlength(f(inputs)) + length(f(inputs))"
        ));
    }

    /// `collect` is an ordinary function, so the bare-application rule reaches it like any
    /// other: `collect lines`, with no parens at all, is one more way to spell the acceptance
    /// program alongside `collect(lines)`.
    #[test]
    fn collect_takes_lines_with_or_without_parens() {
        assert!(toylang::compile("collect(lines)").is_ok());
        assert!(toylang::compile("collect lines").is_ok());
    }

    /// A Vec is already a value; `collect` is the exit from the effect layer, not a copy.
    #[test]
    fn collect_of_a_vec_is_refused() {
        insta::assert_snapshot!(err("collect([1, 2])"));
    }

    /// `inputs` is born a stream now, so a Vec-wanted position names the eager spelling instead
    /// of silently materializing.
    #[test]
    fn inputs_wanted_as_a_vec_names_the_eager_spelling() {
        insta::assert_snapshot!(err("fn g(x: Vec<Int>) -> Int = length(x)\n\ng(inputs)"));
    }

    /// `input` is one whole value already in hand, which is exactly what a stream is not, so a
    /// stream-typed position cannot ask for it.
    #[test]
    fn input_cannot_be_a_stream() {
        insta::assert_snapshot!(err(
            "fn f(s: Stream<Str>) -> Vec<Str> = collect(s)\n\nf(input)"
        ));
    }

    /// The same once-per-element problem for the sources themselves.
    #[test]
    fn lines_cannot_be_read_inside_a_mapper() {
        insta::assert_snapshot!(err("length([1] | map(length(collect(lines))))"));
    }

    #[test]
    fn inputs_cannot_be_read_inside_a_mapper() {
        insta::assert_snapshot!(err(
            "fn g(x: Vec<Int>) -> Int = length(x)\n\nlength([1] | map(g(collect(inputs))))"
        ));
    }

    /// A source is legal only in the program's own body, so a `fn` body cannot read one however
    /// it is called. The rule is this blunt because anything finer leaked: see the reproducer
    /// below.
    #[test]
    fn lines_cannot_be_read_inside_a_fn_body() {
        insta::assert_snapshot!(err("fn f(x: Int) -> Vec<Str> = collect(lines)\n\n1"));
    }

    #[test]
    fn inputs_cannot_be_read_inside_a_fn_body() {
        insta::assert_snapshot!(err("fn f(x: Int) -> Vec<Int> = collect(inputs)\n\n1"));
    }

    /// The reproducer that forced the rule: the source is read in `f`'s body, the mapper only
    /// sees an innocent `f(.)`, and before the fn-body ban this compiled and re-read stdin once
    /// per element. The mapper-body check alone cannot catch it, because nothing at the call
    /// site says a source is behind it.
    #[test]
    fn a_function_reading_a_source_cannot_be_called_from_a_mapper() {
        insta::assert_snapshot!(err(
            "fn f(x: Int) -> Int = length(collect(lines)) + x\n\nlength([1, 2] | map(f(.)))"
        ));
    }

    /// A stream is born only at a source, so a function cannot conjure one: a stream result
    /// flows in through a stream parameter, keeping every pipeline one chain from source to
    /// sink.
    #[test]
    fn a_function_cannot_conjure_a_stream() {
        insta::assert_snapshot!(err("fn f(x: Int) -> Stream<Str> = lines\n\n1"));
    }
}
