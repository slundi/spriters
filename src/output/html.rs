//! HTML test page (`--html`) showing every class of the sprite.

use std::io::Write;

use super::SPRITERS_VERSION;
use super::classes::entries;
use crate::{Config, Sprite};

pub(super) fn generate_html(
    config: &Config,
    sprite: &Sprite,
    f: &mut dyn Write,
) -> std::io::Result<()> {
    let name = sprite.name();
    let entries = entries(config, sprite);
    writeln!(f, "<html>
    <head><title>Spriters Test Html</title>
    <link rel=\"stylesheet\" type=\"text/css\" href=\"{}.css\">
    <style type=\"text/css\">tr div:hover{{border:1px solid #ccc;}} tr div{{border:1px solid white;}}</style>
    </head><body>
    <h1>CSS Classes ({})</h1>
    <table>
    <thead><tr>
        <th>CSS Class</th>
        <th>Result</th>
    </tr></thead><tbody>", name, entries.len())?;
    for entry in &entries {
        // Same class names as the generated stylesheet (namespace, separator…). An entry
        // with a pseudo-class shows when its state applies (e.g. on hover).
        writeln!(
            f,
            "<tr><td>{selector}</td><td><div class=\"{class}\"></div></td></tr>",
            selector = entry.selector(),
            class = entry.class,
        )?;
    }
    write!(f, "</tbody></table>
    <footer>Generated using <a href=\"https://codeberg.org/slundi/spriters\">Spriters v{}</a></footer>
    </body></html>", SPRITERS_VERSION)?;
    Ok(())
}
