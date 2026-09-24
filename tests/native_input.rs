//! What a built native program does with stdin that does not match its declared type. Everything
//! else in the suite reaches the native backend through `run_on`, which validates the input in
//! the host first, so the runtime's own reader (runtime-rs/src/json.rs) never sees a bad value
//! there. These run the built binary the way `./adults < data.json` does: the message names the
//! path into the value, and the exit status is 1.

use std::io::Write;
use std::process::{Command, Stdio};

struct Built {
    _dir: tempfile::TempDir,
    exe: std::path::PathBuf,
}

fn build(program: &str) -> Built {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("p.toy"), program).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_toylang"))
        .args(["build", "p.toy", "native"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let exe = dir.path().join("p");
    Built { _dir: dir, exe }
}

/// Exit code, stdout and stderr of the built program fed `stdin` (bytes, so an invalid UTF-8 one
/// can be sent).
fn run(built: &Built, stdin: &[u8]) -> (Option<i32>, String, String) {
    let mut child = Command::new(&built.exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    let out = child.wait_with_output().unwrap();
    (
        out.status.code(),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

fn refusal(built: &Built, stdin: &str) -> String {
    let (code, out, err) = run(built, stdin.as_bytes());
    assert_eq!((code, out.as_str()), (Some(1), ""), "{stdin}: {err}");
    err
}

const USERS: &str = "\
fn id(db: { users: Vec<{ name: Str, age: Int }> }) -> { users: Vec<{ name: Str, age: Int }> } = db;


id(parse stdin)
";

#[test]
fn a_mismatch_names_the_path_into_the_value() {
    let built = build(USERS);
    let ok = r#"{"users": [{"name": "ada", "age": 36, "extra": [1, {"x": null}]}]}"#;
    assert_eq!(
        run(&built, ok.as_bytes()),
        (
            Some(0),
            "{\"users\":[{\"name\":\"ada\",\"age\":36}]}\n".into(),
            String::new()
        )
    );

    let cases = [
        (
            r#"{"users": [{"name": "ada", "age": 36}, {"name": "bo", "age": "9"}]}"#,
            "expected an integer at input.users[1].age",
        ),
        (
            r#"{"users": [{"name": "ada"}]}"#,
            "missing field `age` at input.users[0]",
        ),
        (
            r#"{"users": {"name": "ada"}}"#,
            "expected an array at input.users",
        ),
        (
            r#"{"users": [{"name": 7, "age": 1}]}"#,
            "expected a string at input.users[0].name",
        ),
        (
            r#"{"users": [{"name": "a", "age": 1.5}]}"#,
            "expected an integer, found a non-integer number at input.users[0].age",
        ),
        (
            r#"{"users": [{"name": "a", "age": 2147483648}]}"#,
            "integer is out of range at input.users[0].age",
        ),
        (
            r#"{"users": []} []"#,
            "trailing content after the value at input",
        ),
        (r#"[]"#, "expected an object at input"),
        ("", "unexpected end of input at input"),
        (
            r#"{"users": [{"name": "a""#,
            "unexpected end of input at input.users[0]",
        ),
        (
            r#"{"users": [{"name": "\ud800", "age": 1}]}"#,
            "unpaired surrogate",
        ),
    ];
    for (input, want) in cases {
        let err = refusal(&built, input);
        assert!(err.starts_with("toylang: input: "), "{input}: {err}");
        assert!(err.contains(want), "{input}: wanted `{want}` in `{err}`");
    }
}

#[test]
fn an_enum_names_itself_and_the_way_its_value_missed() {
    let built = build(
        "enum Shape { Point, Circle{r: Int} }\n\n\nfn f(s: Shape) -> Shape = s;\n\n\nf(parse stdin)\n",
    );
    assert_eq!(
        run(&built, br#"{"Circle": {"r": 2}}"#).1,
        "{\"Circle\":{\"r\":2}}\n"
    );
    let cases = [
        (
            r#""circle""#,
            "`circle` is not a unit variant of Shape at input",
        ),
        (
            r#"{"point": 1}"#,
            "`point` is not a payload variant of Shape at input",
        ),
        (
            r#""Circle""#,
            "`Circle` is not a unit variant of Shape at input",
        ),
        (
            r#"{"Circle": {"r": 2}, "Point": 1}"#,
            "expected `}` at input",
        ),
        ("3", "expected Shape at input"),
        (
            r#"{"Circle": {"r": "x"}}"#,
            "expected an integer at input.Circle.r",
        ),
    ];
    for (input, want) in cases {
        let err = refusal(&built, input);
        assert!(err.contains(want), "{input}: wanted `{want}` in `{err}`");
    }
}

/// A stream names the source, not a line number, and refuses the whole run at the bad line.
#[test]
fn each_line_of_a_stream_is_one_value() {
    let program = "\
fn id(xs: Stream<{ a: Int }>) -> Stream<{ a: Int }> = xs;


jsonlines(id(stdin | map parse(.)))
";
    let built = build(program);
    let (code, out, _) = run(&built, b"{\"a\": 1}\n\n  \n{\"a\": 2}\n");
    assert_eq!((code, out.as_str()), (Some(0), "{\"a\":1}\n{\"a\":2}\n"));
    let (code, out, err) = run(&built, b"{\"a\": 1}\n{\"a\": \"x\"}\n{\"a\": 3}\n");
    assert_eq!(code, Some(1));
    assert_eq!(
        out, "{\"a\":1}\n",
        "what was read before the bad line is kept"
    );
    assert!(err.contains("expected an integer at inputs.a"), "{err}");
    // A value that runs over a line is a broken value, not the start of one.
    let (code, _, err) = run(&built, b"{\"a\":\n1}\n");
    assert_eq!(code, Some(1));
    assert!(err.contains("unexpected end of input at inputs"), "{err}");
}

/// The measurement behind `float_roundtrip` (plans/native-runtime-rust-research.md, section 3.3):
/// through the built program, so nothing in the host reads the number first. The correctly
/// rounded double for this text prints as `91186252760.18954`; serde_json's default float path
/// reads the neighbouring double, which prints `91186252760.18956`.
#[test]
fn a_long_digit_float_reads_correctly_rounded() {
    let built = build("fn id(x: Float) -> Float = x;\n\n\nid(parse stdin)\n");
    assert_eq!(run(&built, b"91186252760.18955").1, "91186252760.18954\n");
    assert_eq!(run(&built, b"8525388853933633.0").1, "8525388853933633\n");
    // 64 characters or more: the C reader refused these as "out of range".
    let long = format!("0.{}", "1".repeat(80));
    assert_eq!(run(&built, long.as_bytes()).1, "0.1111111111111111\n");
}

#[test]
fn stdin_lines_that_are_not_utf8_are_refused() {
    let built = build("join_lines(collect stdin)\n");
    assert_eq!(run(&built, b"a\r\nb").1, "a\r\nb\n");
    let (code, out, err) = run(&built, b"ok\n\xed\xa0\x80\n");
    assert_eq!((code, out.as_str()), (Some(1), ""));
    assert_eq!(err, "toylang: input: stdin is not valid UTF-8 at lines\n");
}

#[test]
fn dividing_by_zero_exits_1() {
    let built = build("fn f(n: Int) -> Int = 10 / n;\n\n\nf(parse stdin)\n");
    assert_eq!(run(&built, b"5").1, "2\n");
    let (code, out, err) = run(&built, b"0");
    assert_eq!(
        (code, out.as_str(), err.as_str()),
        (Some(1), "", "toylang: divided by zero\n")
    );
}
