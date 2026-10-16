# Local parser limit

Source: crates.io `vte` 0.15.0, upstream commit
`3b3da71c34cc1256c7e20981cf03f8eb95e08ffc` at
https://github.com/alacritty/vte.

The standard-library parser normally stores an OSC command in an unbounded
buffer. An SSH peer can send an unterminated OSC command indefinitely. This patch
caps retained OSC payloads at 4,096 bytes and discards the entire oversized
command until its normal terminator. Ordinary commands and UTF-8 processing keep
the upstream parser behavior. The existing no-std limit is unchanged.

`src/security_tests.rs` checks the size boundary, split input, both terminators,
discarding oversized commands, and normal processing afterward.

Sources, manifest, examples, upstream README, license files, and VCS metadata are
preserved. Release logs, standalone lockfile, CI files, and external fixtures are
omitted. Remove this patch when an upstream release provides equivalent limits.
