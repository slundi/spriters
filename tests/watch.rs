//! End-to-end tests of `--watch` (review 6.9).

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Wait up to 15 seconds for `condition`.
fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn save_icon(path: &Path, size: u32, color: [u8; 4]) {
    image::RgbaImage::from_pixel(size, size, image::Rgba(color))
        .save(path)
        .unwrap();
}

/// Width × height of the sprite image `path`, `None` while it cannot be read.
fn sprite_area(path: &Path) -> Option<u32> {
    image::image_dimensions(path)
        .ok()
        .map(|(width, height)| width * height)
}

/// Kills the watcher when the test ends, even on failure.
struct Watcher(Option<Child>);
impl Watcher {
    fn is_running(&mut self) -> bool {
        self.0.as_mut().unwrap().try_wait().unwrap().is_none()
    }
    /// Stop it and return what it printed on stderr.
    fn stop(mut self) -> String {
        let mut child = self.0.take().unwrap();
        child.kill().ok();
        let output = child.wait_with_output().unwrap();
        String::from_utf8_lossy(&output.stderr).into_owned()
    }
}
impl Drop for Watcher {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            child.kill().ok();
            child.wait().ok();
        }
    }
}

#[test]
fn watch_survives_the_removal_of_the_source_directory() {
    let tmp = std::env::temp_dir().join(format!("spriters_watch_e2e_{}", std::process::id()));
    std::fs::remove_dir_all(&tmp).ok();
    let source = tmp.join("icons");
    let output = tmp.join("out");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&output).unwrap();
    save_icon(&source.join("red.png"), 4, [255, 0, 0, 255]);

    let mut command = Command::new(env!("CARGO_BIN_EXE_spriters"));
    command
        .args(["--watch", "-i"])
        .arg(&source)
        .arg("-o")
        .arg(&output)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    for (key, _) in std::env::vars().filter(|(key, _)| key.starts_with("SPRITERS_")) {
        command.env_remove(key);
    }
    let mut watcher = Watcher(Some(command.spawn().unwrap()));
    let sprite = output.join("icons.png");
    wait_until("the first sprite", || sprite_area(&sprite) == Some(16));
    let css = std::fs::read_to_string(output.join("icons.css")).unwrap();

    std::fs::remove_dir_all(&source).unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    assert!(watcher.is_running(), "--watch stopped with its source");
    // Nothing is regenerated while the source is missing: the outputs are kept.
    assert_eq!(
        std::fs::read_to_string(output.join("icons.css")).unwrap(),
        css
    );

    // Created again in one step, as a checkout or a sync tool does.
    let staging = tmp.join("staging");
    std::fs::create_dir_all(&staging).unwrap();
    save_icon(&staging.join("red.png"), 8, [255, 0, 0, 255]);
    std::fs::rename(&staging, &source).unwrap();
    wait_until("the sprite of the new directory", || {
        sprite_area(&sprite) == Some(64)
    });

    // The new directory is watched: later changes are picked up.
    save_icon(&source.join("blue.png"), 8, [0, 0, 255, 255]);
    wait_until("the sprite with the added icon", || {
        sprite_area(&sprite) == Some(128)
    });

    assert!(watcher.is_running());
    let stderr = watcher.stop();
    assert!(
        stderr.contains("was removed, waiting for it to be created again"),
        "{stderr}"
    );
    std::fs::remove_dir_all(&tmp).ok();
}
