# spriters

Library and CLI to create sprites.

This library and program is based on [glue](http://glue.readthedocs.org/), a Python CLI to create sprites.

**Why use sprites?**

It is faster 🚀: there is less to load. In an application or a game, it avoids to continually fetch new images by displaying
some part(s) of it. For the web, it avoids to perform multiple HTTP requests and loading them separately.

It can be smaller than the sum of the size of all images. But it is not always the case.

## Features

- Various image format supported, details are [here](https://crates.io/crates/image)

## Usage

For the library, run `cargo add spriters` or add `spriters = "<version>` in your `Cargo.toml`. More details on [Crates.io](https://crates.io/crates/spriters)

To install the CLI, you need to [install rust](https://www.rust-lang.org/tools/install) first. Then run `cargo install spriters`.

## Roadmap

- [ ] port the **glue** CLI in this library/CLI
- [ ] support other output image format
- [ ] make it available on crates.io
