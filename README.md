# spriters

Library and CLI to create sprites.

This library and program is based on [glue](http://glue.readthedocs.org/), a Python CLI to create sprites.

**Why use sprites?**

It is faster 🚀: there is less to load. In an application or a game, it avoids to continually fetch new images by displaying
some part(s) of it. For the web, it avoids to perform multiple HTTP requests and loading them separately.

It can be smaller than the sum of the size of all images. But it is not always the case.

## Features

- 🚀 Fast, because it is written in Rust
- 🖼 Various image format supported, details are [here](https://crates.io/crates/image)
- ✂️ Crop images from alpha or a given color

## Usage

For the library, run `cargo add spriters` or add `spriters = "<version>` in your `Cargo.toml`. More details on [Crates.io](https://crates.io/crates/spriters)

To install the CLI, you need to [install rust](https://www.rust-lang.org/tools/install) first. Then run `cargo install spriters`.

## Roadmap

- [x] ✂️ crop image
- [ ] 📏 resize images with max width and/or height
- [ ] 📄 support other output image format
- [ ] 📦 make it available on crates.io
- [ ] 🖥 multi-thread processing per sprite when in project mode and for crop and resize operations per file
- [x] 👁 remove watcher because it can be achieved with other tool like [fswatch](https://github.com/emcrisostomo/fswatch) or [eagle-eye](https://github.com/aag/eagle-eye) or [watchman](https://github.com/facebook/watchman) ...
- [ ] 🟰 check for duplicate images:
  - [x] warn user about duplicate files
  - [x] add option to fail or not when a duplicate is found
  - [ ] map multiple name for a same sprite
