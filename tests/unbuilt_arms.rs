//! A builtin that has landed on some backends and not others is refused, not panicked on,
//! everywhere it has not landed -- and the table that says where it has landed
//! (`backend_support::LANDINGS`) is held to the emitters in both directions: every backend
//! the table names must emit the program, and every backend it leaves out must refuse it
//! with the same message the CLI prints. A landing row that adds an arm updates the table,
//! and this test is what tells it so.

use toylang::Backend;
use toylang::backend_support::LANDINGS;

const JQ_REFUSAL: &str =
    "`pipe_through` has no jq backend, and never will: jq cannot spawn a process";

/// One program per landing builtin, using nothing else that is backend-specific.
fn program_using(name: &str) -> &'static str {
    match name {
        "sort_by" => "[3, 1, 2] | sort_by(.)\n",
        "max_by" => "[3, 1, 2] | max_by(.)\n",
        "transpose" => "transpose([[1, 2], [3, 4]])\n",
        "pipe_through" => "collect(pipe_through({cmd: \"cat\", args: [], lines: stdin}))\n",
        "sqrt" => "sqrt(2.0)\n",
        "float" => "float(1) + 0.5\n",
        "closure" => {
            "fn f({items, pred}: {items: Vec<Int>, pred: Int -> Bool}) -> Int = \
             items | select(pred) | length(.);\n\n\
             f({items: [1, 2], pred: $ > 0})\n"
        }
        other => panic!("no program for landing builtin `{other}`; add one here"),
    }
}

#[test]
fn every_landing_builtin_emits_where_built_and_refuses_elsewhere() {
    let mut failures = Vec::new();
    for landing in LANDINGS {
        let program = toylang::compile(program_using(landing.name))
            .unwrap_or_else(|e| panic!("`{}` program does not compile: {e}", landing.name));
        for backend in Backend::ALL {
            let result = backend.emit(&program);
            let built = landing.built_on.contains(&backend);
            match (built, result) {
                (true, Ok(_)) => {}
                (true, Err(e)) => failures.push(format!(
                    "`{}` is listed as built on {} but emitting fails: {e}",
                    landing.name,
                    backend.name()
                )),
                (false, Err(e)) => {
                    let runs_on: Vec<&str> = landing.built_on.iter().map(|b| b.name()).collect();
                    let expected = match landing.never_on.iter().find(|(b, _)| *b == backend) {
                        Some((_, why)) => format!(
                            "`{}` has no {} backend, and never will: {why}",
                            landing.name,
                            backend.name()
                        ),
                        None => format!(
                            "`{}` has no {} backend yet; today it runs on {}",
                            landing.name,
                            backend.name(),
                            runs_on.join(" and ")
                        ),
                    };
                    if e != expected {
                        failures.push(format!(
                            "`{}` on {}: refused with {e:?}, expected {expected:?}",
                            landing.name,
                            backend.name()
                        ));
                    }
                }
                (false, Ok(_)) => failures.push(format!(
                    "`{}` emits on {} but the table says it is not built there; add it to LANDINGS",
                    landing.name,
                    backend.name()
                )),
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A backend cannot be both built and permanently refused; the table would contradict itself.
#[test]
fn a_permanently_refused_backend_is_not_also_built() {
    for landing in LANDINGS {
        for (backend, why) in landing.never_on {
            assert!(
                !landing.built_on.contains(backend),
                "`{}` lists {} as built and as never ({why})",
                landing.name,
                backend.name()
            );
        }
    }
}

/// Ruling 2026-09-20's check-level claims: `sqrt` is `Float -> Float` and `float` is
/// `Int -> Float` (or `Float -> Float`, unchanged), independent of which backends have
/// landed an arm for either.
#[test]
fn sqrt_and_float_type_check_where_the_corpus_cannot() {
    // `sqrt` is `Float -> Float`.
    toylang::compile("sqrt(2.0)\n").expect("sqrt(2.0) type-checks");
    // The other edge, a negative argument, is the backend's business (NaN): on the front end
    // it type-checks like any `Float`, because `sqrt` cannot know the values.
    toylang::compile("sqrt(-1.0)\n").expect("sqrt(-1.0) type-checks");
    // `float` is `Int -> Float`, exact (Int is 32 bits, Float is 64).
    toylang::compile("float(1) + 0.5\n").expect("float(1) + 0.5 type-checks");
    toylang::compile("float(2.5)\n").expect("float(2.5) type-checks: Float is returned unchanged");
    // A non-numeric argument is an ordinary type error, not a builtin-specific one.
    assert!(
        toylang::compile("float([1])\n").is_err(),
        "float takes Int or Float; a Vec is a type error"
    );
}

/// The refusal reaches a program that only uses the builtin inside a named function, not
/// just at the top level -- the walk covers every function body.
#[test]
fn a_use_inside_a_function_is_refused_too() {
    let program = toylang::compile(
        "fn shout(lines: Stream<Str>) -> Vec<PipeLine> = \
         collect(pipe_through({cmd: \"cat\", args: [], lines: lines}));\n\n\
         shout(stdin)\n",
    )
    .unwrap();
    assert_eq!(Backend::Jq.emit(&program).unwrap_err(), JQ_REFUSAL);
}

/// `toylang run` does not go through `Backend::emit`; it must refuse the same way.
#[test]
fn running_is_refused_the_same_way_as_emitting() {
    let err = toylang::run_on(program_using("pipe_through"), None, Backend::Jq).unwrap_err();
    assert_eq!(err.to_string(), JQ_REFUSAL);
}
