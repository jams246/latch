# Local russh-sftp patch

Source: `russh-sftp` 3.0.1 from crates.io, archive SHA-256
`9bb17f72aba4314656b359ffba72c68dcbd70ed9e97497898b75498fe7198042`.
Upstream: <https://github.com/AspectUnk/russh-sftp>, commit
`e8de8f903cb019f32beb26cc1195c7c71bef121f`.
The Apache-2.0 license and complete library source are preserved. Examples,
benchmarks and their development dependencies are omitted.

The upstream directory reader accumulated responses without a total limit and
rebuilt the entire list on every response. A hostile server could exhaust memory
or CPU without ending the directory. This patch appends in linear time and limits
each listing to 100,000 entries, 16 MiB of retained entry data, and 30 seconds.
Empty non-EOF responses fail. Directory handles close after read failures as well
as success. Dot entries are skipped iteratively to avoid attacker-controlled
stack growth. Existing framing already rejects packets over the configured limit
before allocation; regression tests exercise oversized and truncated packets.
The deprecated atomic `fetch_update` spelling is updated to `try_update` for the
project's Rust 1.99 toolchain; the operation is unchanged.

The crate stays excluded from the application workspace. Verify its unit tests
from a copy under `.verification/` with an empty `[workspace]` in its manifest and
the application lockfile copied alongside it, using `cargo test --manifest-path
<copy>/Cargo.toml --target-dir target --lib --offline`. This keeps the generated
standalone lockfile inside the ignored verification workspace. Remove the patch
once an upstream release provides equivalent directory resource bounds and
iterative dot-entry handling.
