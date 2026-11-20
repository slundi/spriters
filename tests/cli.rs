//! End-to-end tests of the `spriters` binary: exit codes and messages printed to stderr.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

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

/// The binary with `args` and no `SPRITERS_*` variable.
fn command(args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_spriters"));
    command.args(args);
    for (key, _) in std::env::vars() {
        if key.starts_with("SPRITERS_") {
            command.env_remove(key);
        }
    }
    command
}

/// Run the binary with `args` and an empty stdin.
fn spriters(args: &[&str]) -> Output {
    command(args)
        .stdin(Stdio::null())
        .output()
        .expect("failed to run spriters")
}

/// Run the binary with `input` written to a stdin pipe. Unless `close` is set, the pipe
/// then stays open, as Node's `child_process.spawn` does. Returns `None` (after killing
/// the process) if it has not exited within 10 seconds.
fn spriters_piped(mut command: Command, input: &str, close: bool) -> Option<Output> {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run spriters");
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(input.as_bytes()).unwrap();
    let open_stdin = if close {
        drop(stdin);
        None
    } else {
        Some(stdin)
    };

    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            drop(open_stdin);
            return Some(child.wait_with_output().unwrap());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child.kill().ok();
    child.wait().ok();
    None
}

/// Number of frames in a JSON (array format) sprite descriptor.
fn json_frames(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .unwrap()
        .matches("\"filename\"")
        .count()
}

/// Add a second, distinct icon next to `red.png`.
fn add_blue_icon(source: &Path) {
    image::RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 255, 255]))
        .save(source.join("blue.png"))
        .unwrap();
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
fn open_stdin_pipe_does_not_block() {
    // Review 3.6: a never-closed stdin pipe (Node spawn, docker run -i, CI) hung forever.
    with_tmp_dir(|tmp| {
        let (source, out) = source_and_output(tmp);
        let output = spriters_piped(
            command(&["-i", source.to_str().unwrap(), "-o", out.to_str().unwrap()]),
            "",
            false,
        )
        .expect("spriters must not wait for stdin");

        assert!(output.status.success(), "{}", stderr(&output));
        assert!(out.join("icons.png").exists());
    });
}

#[test]
fn piped_paths_are_ignored_without_stdin_flag() {
    with_tmp_dir(|tmp| {
        let (source, out) = source_and_output(tmp);
        add_blue_icon(&source);
        let listed = format!("{}\n", source.join("red.png").display());
        let output = spriters_piped(
            command(&[
                "-i",
                source.to_str().unwrap(),
                "-o",
                out.to_str().unwrap(),
                "--json",
            ]),
            &listed,
            true,
        )
        .unwrap();

        assert!(output.status.success(), "{}", stderr(&output));
        assert_eq!(json_frames(&out.join("icons.json")), 2, "-i must be walked");
    });
}

#[test]
fn stdin_flag_reads_the_file_list() {
    with_tmp_dir(|tmp| {
        let (source, out) = source_and_output(tmp);
        add_blue_icon(&source);
        let listed = format!("{}\n", source.join("red.png").display());
        let output = spriters_piped(
            command(&[
                "--stdin",
                "-i",
                source.to_str().unwrap(),
                "-o",
                out.to_str().unwrap(),
                "--json",
            ]),
            &listed,
            true,
        )
        .unwrap();

        assert!(output.status.success(), "{}", stderr(&output));
        assert_eq!(json_frames(&out.join("icons.json")), 1, "only listed files");
    });
}

#[test]
fn input_dash_reads_the_file_list() {
    with_tmp_dir(|tmp| {
        let (source, out) = source_and_output(tmp);
        add_blue_icon(&source);
        let mut piped = command(&["-i", "-", "-o", out.to_str().unwrap(), "--json"]);
        // The source defaults to the working directory, which names the sprite.
        piped.current_dir(&source);
        let output = spriters_piped(piped, "red.png\n", true).unwrap();

        assert!(output.status.success(), "{}", stderr(&output));
        assert_eq!(json_frames(&out.join("icons.json")), 1);
    });
}

#[test]
fn unknown_option_fails_with_message() {
    let output = spriters(&["--bogus"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("--bogus"));
}
