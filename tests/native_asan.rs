//! The native runtime is C, and C's memory errors do not show up as a wrong answer: a write one
//! byte past a malloc block goes unnoticed because malloc rounds sizes up. This links a program
//! with the runtime built under AddressSanitizer, so such a write fails the run.
//!
//! `link` finds the compiler as `cc` on PATH, so the sanitizer goes in through a `cc` wrapper
//! placed first on the PATH of a `toylang build` child, not through a flag on `link`.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

/// Whether the real `cc` can build and run with AddressSanitizer; when it cannot (no libasan
/// installed), the test has nothing to say.
fn cc_supports_asan(dir: &Path) -> bool {
    let probe = dir.join("probe.c");
    std::fs::write(&probe, "int main(void) { return 0; }\n").unwrap();
    Command::new("cc")
        .args(["-fsanitize=address", "-o"])
        .arg(dir.join("probe"))
        .arg(&probe)
        .status()
        .is_ok_and(|s| s.success())
}

/// A `cc` in `bin` that adds -fsanitize=address. It drops its own directory from PATH first, so
/// the `cc` it execs is the real one rather than itself.
fn write_asan_cc(bin: &Path) {
    let wrapper = bin.join("cc");
    let script = format!(
        "#!/bin/sh\nPATH=${{PATH#{}:}}\nexec cc -fsanitize=address -g \"$@\"\n",
        bin.display()
    );
    std::fs::write(&wrapper, script).unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn run_under_asan(program: &str) -> Option<String> {
    let dir = tempfile::tempdir().unwrap();
    if !cc_supports_asan(dir.path()) {
        return None;
    }
    let bin = dir.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    write_asan_cc(&bin);
    std::fs::write(dir.path().join("p.toy"), program).unwrap();

    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let build = Command::new(env!("CARGO_BIN_EXE_toylang"))
        .args(["build", "p.toy", "native"])
        .current_dir(dir.path())
        .env("PATH", path)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    // Leaks are not the question: the runtime's stance is that nothing frees.
    let run = Command::new(dir.path().join("p"))
        .env("ASAN_OPTIONS", "detect_leaks=0")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "sanitizer stopped the run:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    Some(String::from_utf8(run.stdout).unwrap())
}

#[test]
fn printing_zero_and_nan_stays_inside_the_allocation() {
    let Some(out) = run_under_asan("[0.0, 0.0 / 0.0]\n") else {
        eprintln!("skipped: cc cannot build with -fsanitize=address here");
        return;
    };
    assert_eq!(out, "[0,NaN]\n");
}
