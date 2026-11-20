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
    without_spriters_env(&mut command);
    command
}

/// Remove every `SPRITERS_*` variable from the environment of `command`.
fn without_spriters_env(command: &mut Command) -> &mut Command {
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

/// The process ended normally: no panic message and not Rust's panic exit code (101).
fn assert_no_panic(output: &Output) {
    let message = stderr(output);
    assert!(!message.contains("panicked"), "panic: {message}");
    assert_ne!(output.status.code(), Some(101), "panic: {message}");
}

#[test]
fn input_dot_uses_the_directory_name() {
    // Review 3.7: `-i .` panicked on `file_name().unwrap()`.
    with_tmp_dir(|tmp| {
        let (source, out) = source_and_output(tmp);
        let output = command(&["-i", ".", "-o", out.to_str().unwrap()])
            .current_dir(&source)
            .stdin(Stdio::null())
            .output()
            .unwrap();

        assert_no_panic(&output);
        assert!(output.status.success(), "{}", stderr(&output));
        assert!(out.join("icons.png").exists());
        assert!(out.join("icons.css").exists());
    });
}

#[test]
fn relative_url_is_accepted() {
    // Review 3.7: `-u /static/` panicked with RelativeUrlWithoutBase.
    with_tmp_dir(|tmp| {
        let (source, out) = source_and_output(tmp);
        let output = spriters(&[
            "-i",
            source.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "-u",
            "/static/",
        ]);

        assert_no_panic(&output);
        assert!(output.status.success(), "{}", stderr(&output));
        let css = std::fs::read_to_string(out.join("icons.css")).unwrap();
        assert!(css.contains("url('/static/icons.png')"), "{css}");
    });
}

#[cfg(unix)]
#[test]
fn non_utf8_url_is_a_clean_error() {
    use std::os::unix::ffi::OsStrExt;
    let mut invalid = command(&["-u"]);
    invalid.arg(std::ffi::OsStr::from_bytes(b"/st\xffatic/"));
    let output = invalid.stdin(Stdio::null()).output().unwrap();

    assert_no_panic(&output);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("--url"), "{}", stderr(&output));
}

#[test]
fn invalid_ratios_and_spacing_are_clean_errors() {
    with_tmp_dir(|tmp| {
        let (source, out) = source_and_output(tmp);
        for (option, value, expected) in [
            ("--ratios", "abc", "invalid ratio 'abc'"),
            ("--ratios", "0", "invalid ratio '0'"),
            ("--padding", "abc", "invalid padding 'abc'"),
            ("--margin", "1 2 3 4 5", "invalid margin '1 2 3 4 5'"),
        ] {
            let output = spriters(&[
                "-i",
                source.to_str().unwrap(),
                "-o",
                out.to_str().unwrap(),
                option,
                value,
            ]);

            assert_no_panic(&output);
            assert!(!output.status.success(), "{option} {value} must fail");
            assert!(
                stderr(&output).contains(expected),
                "{option} {value}: {}",
                stderr(&output)
            );
        }
    });
}

#[test]
fn stats_ignores_bare_metadata_file() {
    // Review 3.7: a file named `.spriters` made `spriters stats` panic.
    with_tmp_dir(|tmp| {
        std::fs::write(tmp.join(".spriters"), "").unwrap();
        let output = spriters(&["stats", tmp.to_str().unwrap()]);

        assert_no_panic(&output);
        assert!(stderr(&output).contains("No sprite metadata found"));
    });
}

#[cfg(unix)]
#[test]
fn unwritable_output_does_not_panic() {
    use std::os::unix::fs::PermissionsExt;
    with_tmp_dir(|tmp| {
        let (source, out) = source_and_output(tmp);
        std::fs::set_permissions(&out, std::fs::Permissions::from_mode(0o555)).unwrap();
        let output = spriters(&[
            "-i",
            source.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--json",
        ]);
        std::fs::set_permissions(&out, std::fs::Permissions::from_mode(0o755)).unwrap();

        // Running as root bypasses the permission: then the run simply succeeds.
        assert_no_panic(&output);
    });
}

#[test]
fn positional_source_and_output_as_documented() {
    // Review 3.8: the README's `spriters icons/ dist/ --retina` was rejected.
    with_tmp_dir(|tmp| {
        let (source, out) = source_and_output(tmp);
        let output = spriters(&[source.to_str().unwrap(), out.to_str().unwrap(), "--json"]);

        assert!(output.status.success(), "{}", stderr(&output));
        assert!(out.join("icons.png").exists());
        assert!(out.join("icons.json").exists());
    });
}

#[test]
fn positional_directory_conflicting_with_option_fails() {
    with_tmp_dir(|tmp| {
        let (source, out) = source_and_output(tmp);
        let output = spriters(&[
            "-i",
            source.to_str().unwrap(),
            source.to_str().unwrap(),
            out.to_str().unwrap(),
        ]);

        assert!(!output.status.success());
        assert!(stderr(&output).contains("source directory given twice"));
        assert!(!out.join("icons.png").exists());
    });
}

#[test]
fn check_mode_exit_codes() {
    with_tmp_dir(|tmp| {
        let (source, out) = source_and_output(tmp);
        let args = ["-i", source.to_str().unwrap(), "-o", out.to_str().unwrap()];
        let check = |extra: &[&str]| {
            let mut all = args.to_vec();
            all.extend_from_slice(extra);
            spriters(&all)
        };

        let stale = check(&["--check"]);
        assert!(!stale.status.success(), "nothing generated yet");
        assert!(stderr(&stale).contains("out of date"));
        assert!(!out.join("icons.png").exists(), "--check must not generate");

        assert!(check(&[]).status.success());
        let fresh = check(&["--check"]);
        assert!(fresh.status.success(), "{}", stderr(&fresh));
    });
}

// Review 6.1: these errors were only logged, and the exit code stayed 0.
// Directories stand in for unreadable or unwritable files: unlike permissions, this also
// fails when the tests run as root.

#[test]
fn corrupt_image_fails_but_other_images_are_generated() {
    with_tmp_dir(|tmp| {
        let (source, out) = source_and_output(tmp);
        std::fs::write(source.join("broken.png"), b"not a png").unwrap();
        let output = spriters(&[
            "-i",
            source.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--json",
        ]);

        assert!(!output.status.success());
        let message = stderr(&output);
        assert!(message.contains("Cannot open image"), "{message}");
        assert!(message.contains("broken.png"), "{message}");
        assert!(out.join("icons.png").exists());
        assert_eq!(json_frames(&out.join("icons.json")), 1);
    });
}

#[test]
fn unreadable_image_file_fails() {
    with_tmp_dir(|tmp| {
        let (source, out) = source_and_output(tmp);
        std::fs::create_dir(source.join("folder.png")).unwrap();
        let output = spriters(&["-i", source.to_str().unwrap(), "-o", out.to_str().unwrap()]);

        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("Cannot read"),
            "{}",
            stderr(&output)
        );
        assert!(out.join("icons.png").exists());
    });
}

#[test]
fn source_without_images_fails() {
    with_tmp_dir(|tmp| {
        let (source, out) = source_and_output(tmp);
        std::fs::remove_file(source.join("red.png")).unwrap();
        let output = spriters(&["-i", source.to_str().unwrap(), "-o", out.to_str().unwrap()]);

        assert!(!output.status.success());
        assert!(stderr(&output).contains("No images were found"));
    });
}

#[test]
fn unwritable_sprite_image_fails() {
    with_tmp_dir(|tmp| {
        let (source, out) = source_and_output(tmp);
        std::fs::create_dir(out.join("icons.png")).unwrap();
        let output = spriters(&["-i", source.to_str().unwrap(), "-o", out.to_str().unwrap()]);

        assert!(!output.status.success());
        let message = stderr(&output);
        assert!(message.contains("Failed to save sprite image"), "{message}");
        // The text outputs do not depend on the image file.
        assert!(out.join("icons.css").is_file());
    });
}

#[test]
fn unwritable_text_output_fails_on_every_run() {
    with_tmp_dir(|tmp| {
        let (source, out) = source_and_output(tmp);
        std::fs::create_dir(out.join("icons.css")).unwrap();
        let args = ["-i", source.to_str().unwrap(), "-o", out.to_str().unwrap()];

        let first = spriters(&args);
        assert!(!first.status.success());
        assert!(
            stderr(&first).contains("Cannot write CSS"),
            "{}",
            stderr(&first)
        );
        assert!(out.join("icons.png").is_file());

        // The failed output is not recorded as written: the next run retries it instead
        // of reporting the sprite as up to date.
        let second = spriters(&args);
        assert!(!second.status.success());
        assert!(
            !stderr(&second).contains("up to date"),
            "{}",
            stderr(&second)
        );

        std::fs::remove_dir(out.join("icons.css")).unwrap();
        let fixed = spriters(&args);
        assert!(fixed.status.success(), "{}", stderr(&fixed));
        assert!(out.join("icons.css").is_file());
    });
}

/// Run the binary with `args` as on a full disk: a file size limit of 0 makes every
/// write to a regular file fail (`SIGXFSZ` is ignored, so writes return `EFBIG` instead
/// of killing the process). Writes to stderr, a pipe, still work.
#[cfg(target_os = "linux")]
fn spriters_on_full_disk(args: &[&str]) -> Output {
    let mut shell = Command::new("sh");
    shell
        .arg("-c")
        .arg(r#"ulimit -f 0 && trap '' XFSZ && exec "$0" "$@""#)
        .arg(env!("CARGO_BIN_EXE_spriters"))
        .args(args);
    without_spriters_env(&mut shell)
        .stdin(Stdio::null())
        .output()
        .expect("failed to run spriters")
}

/// Review 6.2 and 6.3: on a full disk, files were truncated (`File::create` empties them
/// first) and left so without any message. The previous outputs must survive a failed run.
#[cfg(target_os = "linux")]
#[test]
fn full_disk_keeps_the_previous_outputs() {
    for extra in [None, Some("--png8")] {
        with_tmp_dir(|tmp| {
            let (source, out) = source_and_output(tmp);
            let mut args = vec![
                "-i",
                source.to_str().unwrap(),
                "-o",
                out.to_str().unwrap(),
                "--json",
            ];
            args.extend(extra);
            let first = spriters(&args);
            assert!(first.status.success(), "{}", stderr(&first));
            let files = ["icons.png", "icons.css", "icons.json", ".icons.spriters"];
            let snapshot =
                |out: &Path| files.map(|file| std::fs::read(out.join(file)).unwrap_or_default());
            let before = snapshot(&out);

            add_blue_icon(&source);
            let full = spriters_on_full_disk(&args);
            assert!(!full.status.success(), "{extra:?}");
            let message = stderr(&full);
            assert!(message.contains("Failed to save sprite image"), "{message}");
            assert!(message.contains("Cannot write CSS"), "{message}");
            assert!(message.contains("File too large"), "{message}");
            assert_eq!(snapshot(&out), before, "{extra:?}: outputs were modified");
            let mut entries: Vec<String> = std::fs::read_dir(&out)
                .unwrap()
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            entries.sort();
            assert_eq!(
                entries,
                [".icons.spriters", "icons.css", "icons.json", "icons.png"]
            );

            // Nothing was recorded as done: the next run generates everything.
            let fixed = spriters(&args);
            assert!(fixed.status.success(), "{}", stderr(&fixed));
            assert_eq!(json_frames(&out.join("icons.json")), 2);
        });
    }
}

#[test]
fn unwritable_metadata_fails() {
    with_tmp_dir(|tmp| {
        let (source, out) = source_and_output(tmp);
        std::fs::create_dir(out.join(".icons.spriters")).unwrap();
        let output = spriters(&["-i", source.to_str().unwrap(), "-o", out.to_str().unwrap()]);

        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("Cannot write"),
            "{}",
            stderr(&output)
        );
        // The outputs themselves are written.
        assert!(out.join("icons.png").is_file());
        assert!(out.join("icons.css").is_file());
    });
}

#[test]
fn colliding_labels_fail_with_a_message() {
    // Review 6.4: two images with the same label were silently given the same class.
    with_tmp_dir(|tmp| {
        let (source, out) = source_and_output(tmp);
        std::fs::copy(source.join("red.png"), source.join("my_icon.png")).unwrap();
        add_blue_icon(&source);
        std::fs::rename(source.join("blue.png"), source.join("my-icon.png")).unwrap();
        let output = spriters(&["-i", source.to_str().unwrap(), "-o", out.to_str().unwrap()]);

        assert!(!output.status.success());
        let message = stderr(&output);
        assert!(
            message.contains("'my-icon' is already used by"),
            "{message}"
        );
        assert!(message.contains("my_icon.png is left out"), "{message}");
        assert!(out.join("icons.css").is_file());
    });
}

#[test]
fn no_duplicate_fails_after_writing_the_sprites() {
    // Review 6.5: the process exited in the middle of discovery, before any output (and
    // in project mode, while other sprites were being written).
    with_tmp_dir(|tmp| {
        let (source, out) = source_and_output(tmp);
        let project = tmp.join("project");
        std::fs::create_dir_all(project.join("flags")).unwrap();
        std::fs::rename(&source, project.join("icons")).unwrap();
        for name in ["fr.png", "fr-copy.png"] {
            image::RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 255, 255]))
                .save(project.join("flags").join(name))
                .unwrap();
        }
        let output = spriters(&[
            "--project",
            "--no-duplicate",
            "-i",
            project.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ]);

        assert!(!output.status.success());
        let message = stderr(&output);
        assert!(message.contains("is left out: identical to"), "{message}");
        assert!(message.contains("--no-duplicate"), "{message}");
        for file in ["icons.png", "icons.css", "flags.png", "flags.css"] {
            assert!(out.join(file).is_file(), "{file}: {message}");
        }
    });
}

#[test]
fn project_with_one_broken_sprite_fails_and_builds_the_others() {
    with_tmp_dir(|tmp| {
        let (source, out) = source_and_output(tmp);
        let project = tmp.join("project");
        std::fs::create_dir_all(project.join("broken")).unwrap();
        std::fs::rename(&source, project.join("icons")).unwrap();
        std::fs::write(project.join("broken").join("bad.png"), b"not a png").unwrap();
        let output = spriters(&[
            "--project",
            "-i",
            project.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ]);

        assert!(!output.status.success());
        assert!(out.join("icons.png").is_file());
    });
}

#[test]
fn unknown_option_fails_with_message() {
    let output = spriters(&["--bogus"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("--bogus"));
}
