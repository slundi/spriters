//! End-to-end tests of the `spriters` binary: exit codes and messages printed to stderr.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn with_tmp_dir<F: FnOnce(&Path)>(test: F) {
    let tmp = std::env::temp_dir().join(format!(
        "spriters_cli_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&tmp).unwrap();
    test(&tmp);
    std::fs::remove_dir_all(&tmp).ok();
}

/// Run the binary with `args`, an empty stdin and no `SPRITERS_*` variable.
fn spriters(args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_spriters"));
    command.args(args).stdin(Stdio::null());
    for (key, _) in std::env::vars() {
        if key.starts_with("SPRITERS_") {
            command.env_remove(key);
        }
    }
    command.output().expect("failed to run spriters")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Source folder with one icon, and an empty output folder.
fn source_and_output(tmp: &Path) -> (PathBuf, PathBuf) {
    let source = tmp.join("icons");
    let output = tmp.join("out");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&output).unwrap();
    image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]))
        .save(source.join("red.png"))
        .unwrap();
    (source, output)
}

#[test]
fn missing_source_reports_a_clear_error() {
    with_tmp_dir(|tmp| {
        let missing = tmp.join("does-not-exist");
        let output = spriters(&["-i", missing.to_str().unwrap(), "-o", tmp.to_str().unwrap()]);

        assert!(!output.status.success());
        let message = stderr(&output);
        assert!(
            message.contains("source directory does not exist"),
            "unexpected stderr: {message}"
        );
        // Review 3.4: it used to be reported as a lexopt parsing error.
        assert!(!message.contains("unexpected argument"), "{message}");
    });
}

#[test]
fn errors_are_shown_even_with_quiet() {
    with_tmp_dir(|tmp| {
        let missing = tmp.join("does-not-exist");
        let output = spriters(&[
            "-q",
            "-i",
            tmp.to_str().unwrap(),
            "-o",
            missing.to_str().unwrap(),
        ]);

        assert!(!output.status.success());
        assert!(stderr(&output).contains("output directory does not exist"));
    });
}

#[test]
fn zero_max_width_is_rejected() {
    with_tmp_dir(|tmp| {
        let (source, out) = source_and_output(tmp);
        let output = spriters(&[
            "-i",
            source.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--max-width",
            "0",
        ]);

        assert!(!output.status.success());
        assert!(stderr(&output).contains("max-width must be greater than 0"));
    });
}

#[test]
fn malformed_spriters_toml_is_reported() {
    with_tmp_dir(|tmp| {
        let (source, out) = source_and_output(tmp);
        std::fs::write(source.join("spriters.toml"), "not valid toml [[[").unwrap();

        let output = spriters(&["-i", source.to_str().unwrap(), "-o", out.to_str().unwrap()]);

        // Review 3.4: the file was ignored without any message.
        let message = stderr(&output);
        assert!(
            message.contains("Could not parse spriters.toml"),
            "unexpected stderr: {message}"
        );
        // The file is skipped, generation still happens.
        assert!(output.status.success(), "{message}");
        assert!(out.join("icons.png").exists());
    });
}

#[test]
fn malformed_spriters_toml_warning_respects_quiet() {
    with_tmp_dir(|tmp| {
        let (source, out) = source_and_output(tmp);
        std::fs::write(source.join("spriters.toml"), "not valid toml [[[").unwrap();

        let output = spriters(&[
            "-q",
            "-i",
            source.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ]);

        assert!(output.status.success());
        assert_eq!(stderr(&output), "");
    });
}

#[test]
fn unknown_option_fails_with_message() {
    let output = spriters(&["--bogus"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("--bogus"));
}
