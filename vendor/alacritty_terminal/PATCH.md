# Local combining-character limit

Source: crates.io `alacritty_terminal` 0.26.0, upstream commit
`94e7c8874e526b1e67b349d9ba30ddf81669119e` at
https://github.com/alacritty/alacritty, directory `alacritty_terminal`.

An SSH peer can keep appending zero-width characters to one cell without moving
the cursor. This patch retains at most 32 combining characters per cell, bounding
memory and shaping work while retaining ordinary combining text. Additional
characters are ignored until another cell receives input.

Latch's terminal regression tests exercise repeated input chunks and preserve
ordinary combining characters. The parser has a separate local OSC buffer cap in
`vendor/vte`.

Sources, manifests, upstream README, license, and VCS metadata are preserved.
The large upstream external reference fixtures and their `[[test]]` manifest
entry are omitted; upstream in-source unit tests remain. Release logs and the
standalone lockfile are omitted. Remove this patch when an upstream release
provides an equivalent limit.
