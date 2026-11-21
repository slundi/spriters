//! Atomic writing of the generated files.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

/// Temporary file `.<file name>.<pid>.tmp` in which `path` is written before being renamed
/// (see [`replace_file`]): hidden, in the same directory so that the rename stays on one
/// file system, and unique per process so that concurrent runs do not mix their content.
pub(super) fn temp_path(path: &std::path::Path) -> PathBuf {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!(".{name}.{}.tmp", std::process::id()))
}

/// Name of the file whose temporary file (see [`temp_path`]) is `file_name`, if it is one.
pub(super) fn temp_target(file_name: &str) -> Option<&str> {
    let (target, pid) = file_name
        .strip_prefix('.')?
        .strip_suffix(".tmp")?
        .rsplit_once('.')?;
    (!target.is_empty() && !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit()))
        .then_some(target)
}

/// Write `file` through a buffer with `write`, then flush it explicitly: dropping a
/// `BufWriter` ignores the error of its last flush (e.g. a full disk).
fn write_buffered(
    file: File,
    write: impl FnOnce(&mut BufWriter<File>) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let mut writer = BufWriter::new(file);
    write(&mut writer)?;
    writer.flush()
}

/// Replace `path` atomically: the content is written to a temporary file next to it (see
/// [`temp_path`]), renamed over `path` once complete. A crash or a write error never leaves
/// a truncated file: `path` keeps its previous content, and the temporary file is removed.
pub(super) fn replace_file(
    path: &std::path::Path,
    write: impl FnOnce(&mut BufWriter<File>) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let temp = temp_path(path);
    let result = File::create(&temp)
        .and_then(|file| write_buffered(file, write))
        .and_then(|()| std::fs::rename(&temp, path));
    if result.is_err() {
        // Nothing to clean when it could not even be created.
        std::fs::remove_file(&temp).ok();
    }
    result
}

/// Write the `kind` output file `path` with `write` (see [`replace_file`]). Errors (no
/// write permission, full disk) are logged instead of panicking: the other outputs are
/// still generated.
pub(super) fn write_output(
    path: &std::path::Path,
    kind: &str,
    write: impl FnOnce(&mut dyn Write) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let result = replace_file(path, |writer| write(writer));
    match &result {
        Ok(()) => log::info!("Generated {kind}: {:?}", path),
        Err(e) => log::error!("Cannot write {kind} {:?}: {}", path, e),
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_output_returns_writer_errors() {
        let dir =
            std::env::temp_dir().join(format!("spriters_write_output_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("icons.css");

        assert!(write_output(&path, "CSS", |f| f.write_all(b".a {}")).is_ok());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), ".a {}");
        assert!(write_output(&path, "CSS", |_| Err(std::io::Error::other("boom"))).is_err());
        // Cannot be created: the path is a directory.
        assert!(write_output(&dir, "CSS", |f| f.write_all(b".a {}")).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// `/dev/full` accepts the file creation, then fails every write with "no space left".
    #[cfg(target_os = "linux")]
    #[test]
    fn test_full_disk_is_reported_on_flush() {
        use super::super::png8::encode_png8;
        use super::super::render::encode_image;
        use image::RgbaImage;

        let full = || File::create("/dev/full").unwrap();
        // A few buffered bytes only fail when the `BufWriter` is flushed.
        assert!(write_buffered(full(), |f| f.write_all(b".a {}")).is_err());

        let canvas = RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]));
        for format in [image::ImageFormat::Png, image::ImageFormat::WebP] {
            let result = write_buffered(full(), |f| encode_image(&canvas, format, f));
            assert!(result.is_err(), "{format:?}");
        }
        assert!(write_buffered(full(), |f| encode_png8(&canvas, f)).is_err());
    }

    /// Names of the files in `dir`, sorted.
    fn dir_entries(dir: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn test_replace_file_keeps_previous_content_on_error() {
        // Review 6.3: `File::create` truncated the file first, so a failed or interrupted
        // write left a partial file behind.
        let dir = std::env::temp_dir().join(format!("spriters_replace_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("icons.css");
        std::fs::write(&path, ".old {}").unwrap();

        let failed = replace_file(&path, |f| {
            f.write_all(b".new")?;
            f.flush()?; // the partial content reaches the disk
            Err(std::io::Error::other("interrupted"))
        });
        assert!(failed.is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), ".old {}");
        assert_eq!(
            dir_entries(&dir),
            ["icons.css"],
            "temporary file left behind"
        );

        replace_file(&path, |f| f.write_all(b".new {}")).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), ".new {}");
        assert_eq!(dir_entries(&dir), ["icons.css"]);

        // A new file appears complete or not at all.
        let created = dir.join("icons.json");
        assert!(replace_file(&created, |_| Err(std::io::Error::other("boom"))).is_err());
        assert!(!created.exists());
        // The content is complete while it is written: never visible under its name.
        replace_file(&created, |f| {
            assert!(!created.exists());
            f.write_all(b"{}")
        })
        .unwrap();
        assert_eq!(dir_entries(&dir), ["icons.css", "icons.json"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_temp_path_round_trip() {
        let temp = temp_path(std::path::Path::new("out/icons.css"));
        assert_eq!(temp.parent(), Some(std::path::Path::new("out")));
        let name = temp.file_name().unwrap().to_str().unwrap();
        assert_eq!(name, format!(".icons.css.{}.tmp", std::process::id()));
        assert_eq!(temp_target(name), Some("icons.css"));
        assert_eq!(
            temp_target("..icons.spriters.12.tmp"),
            Some(".icons.spriters")
        );

        for name in [
            "icons.css",
            ".icons.css",
            "icons.css.12.tmp",
            ".icons.css.tmp",
            ".icons.css.12a.tmp",
            "..12.tmp",
            ".icons.css..tmp",
        ] {
            assert_eq!(temp_target(name), None, "{name}");
        }
    }
}
