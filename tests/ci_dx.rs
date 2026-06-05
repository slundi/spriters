use spriters::{Config, Manager};
use std::path::PathBuf;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn with_tmp_dir<F: FnOnce(&std::path::Path)>(test: F) {
    let tmp = std::env::temp_dir().join(format!(
        "spriters_cidx_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&tmp).unwrap();
    test(&tmp);
    std::fs::remove_dir_all(&tmp).ok();
}

fn base_config(source: &str, output: &std::path::Path) -> Config {
    let mut config = Config::default();
    config.source = PathBuf::from(source);
    config.output = output.to_path_buf();
    config.force = true;
    config
}

fn run(config: Config) -> Manager {
    let mut manager = Manager {
        project: config.project,
        config: config.clone(),
        ..Default::default()
    };
    manager.find_sprites();
    manager.save();
    manager
}

// ─── 4.1 Dry-run ─────────────────────────────────────────────────────────────

#[test]
fn dry_run_creates_no_files() {
    with_tmp_dir(|out| {
        let mut config = base_config("tests/images/america", out);
        config.dry_run = true;

        run(config);

        let entries: Vec<_> = std::fs::read_dir(out)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert!(
            entries.is_empty(),
            "dry-run must not write any files; found: {:?}",
            entries.iter().map(|e| e.file_name()).collect::<Vec<_>>()
        );
    });
}

#[test]
fn dry_run_still_discovers_images() {
    with_tmp_dir(|out| {
        let mut config = base_config("tests/images/america", out);
        config.dry_run = true;

        let mut manager = Manager {
            config: config.clone(),
            ..Default::default()
        };
        manager.find_sprites();
        // Images must be discovered even though nothing will be saved.
        assert!(
            !manager.sprites.is_empty(),
            "dry-run must still discover sprites"
        );
        assert!(
            !manager.sprites[0].images.is_empty(),
            "dry-run must still load images"
        );
    });
}

// ─── 4.3 Check mode (via Manager::is_up_to_date) ────────────────────────────

#[test]
fn check_stale_before_first_generation() {
    with_tmp_dir(|out| {
        let config = base_config("tests/images/america", out);

        let mut manager = Manager {
            config: config.clone(),
            ..Default::default()
        };
        manager.find_sprites();

        // Nothing has been written yet — sprite must be considered stale.
        assert!(
            !manager.is_up_to_date(),
            "sprite must be stale before first generation"
        );
    });
}

#[test]
fn check_up_to_date_after_generation() {
    with_tmp_dir(|out| {
        // Generate with force=true (base_config default) so the files are written.
        run(base_config("tests/images/america", out));

        // Check freshness WITHOUT force — force bypasses the hash comparison.
        let mut check_config = base_config("tests/images/america", out);
        check_config.force = false;

        let mut manager = Manager {
            config: check_config,
            ..Default::default()
        };
        manager.find_sprites();

        assert!(
            manager.is_up_to_date(),
            "sprite must be up to date immediately after generation"
        );
    });
}

#[test]
fn force_flag_marks_sprite_as_requiring_regeneration() {
    with_tmp_dir(|out| {
        // Generate first.
        run(base_config("tests/images/america", out));

        // With --force, should_regenerate always returns true, so is_up_to_date is false.
        let mut config = base_config("tests/images/america", out);
        config.force = true;

        let mut manager = Manager {
            config: config.clone(),
            ..Default::default()
        };
        manager.find_sprites();

        assert!(
            !manager.is_up_to_date(),
            "--force must always report the sprite as requiring regeneration"
        );
    });
}

// ─── 4.4 Stats ───────────────────────────────────────────────────────────────

#[test]
fn list_sprite_names_finds_generated_sprites() {
    with_tmp_dir(|out| {
        run(base_config("tests/images/america", out));

        let names = spriters::list_sprite_names(out);
        assert_eq!(names, vec!["america"]);
    });
}

#[test]
fn list_sprite_names_empty_for_fresh_dir() {
    with_tmp_dir(|out| {
        assert!(spriters::list_sprite_names(out).is_empty());
    });
}

#[test]
fn sprite_stats_dimensions_and_efficiency() {
    with_tmp_dir(|out| {
        run(base_config("tests/images/america", out));

        let stats = spriters::sprite_stats(out, "america")
            .expect("stats must be available after generation");

        assert!(stats.width > 0);
        assert!(stats.height > 0);
        assert!(stats.file_size > 0);
        assert!(stats.total_pixels > 0);
        assert!(stats.used_pixels <= stats.total_pixels);
        let pct = stats.packing_efficiency_pct();
        assert!((1..=100).contains(&pct), "efficiency {pct}% must be 1–100");
    });
}

#[test]
fn sprite_stats_returns_none_for_missing_sprite() {
    with_tmp_dir(|out| {
        assert!(spriters::sprite_stats(out, "nonexistent").is_none());
    });
}

// ─── 4.5 Shell completions ───────────────────────────────────────────────────

#[test]
fn bash_completion_covers_key_flags() {
    let script = spriters::completions::BASH;
    for flag in &[
        "--dry-run",
        "--check",
        "--exclude",
        "--css-vars",
        "--format",
        "--algorithm",
    ] {
        assert!(script.contains(flag), "bash completion missing flag {flag}");
    }
    assert!(
        script.contains("complete -F _spriters spriters"),
        "must register completion"
    );
}

#[test]
fn zsh_completion_covers_key_flags() {
    let script = spriters::completions::ZSH;
    for flag in &["--dry-run", "--check", "--exclude", "--css-vars"] {
        assert!(script.contains(flag), "zsh completion missing flag {flag}");
    }
    assert!(
        script.contains("#compdef spriters"),
        "must start with #compdef"
    );
}

#[test]
fn fish_completion_covers_key_flags() {
    let script = spriters::completions::FISH;
    for flag in &["dry-run", "check", "exclude", "css-vars", "algorithm"] {
        assert!(script.contains(flag), "fish completion missing flag {flag}");
    }
    assert!(
        script.contains("complete -c spriters"),
        "must use complete -c spriters"
    );
}
