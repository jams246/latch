# Local stacksafe-macro patch

Source: `stacksafe-macro` 0.1.4 from crates.io, archive SHA-256
`172175341049678163e979d9107ca3508046d4d2a7c6682bee46ac541b17db69`.
Upstream: <https://github.com/fast/stacksafe>, commit
`a5c029b7d3a5fe6771ecd79266b6e21313cfa174`, directory `stacksafe-macro`.
The Apache-2.0 license is copied from that commit's root `LICENSE`.

GPUI 0.2.2 requires stacksafe 0.1, whose macro depends on the unmaintained
`proc-macro-error2` crate ([RUSTSEC-2026-0173](https://rustsec.org/advisories/RUSTSEC-2026-0173.html)).
This patch replaces that dependency and its imports with the maintained
[`proc-macro-error3`](https://docs.rs/proc-macro-error3/) fork, which fixes the
future incompatibility warning. Default features are disabled and `syn2-error`
is enabled to match stacksafe-macro's existing syn 2 dependency.
The upstream abort macros, diagnostic wrapper, function expansion, and stack
growth remain unchanged. The manifest is standalone; regression tests cover
valid expansion, custom crate paths, and rejected inputs.

Verify with `cargo test --locked -p stacksafe-macro`. Remove this patch when
GPUI accepts a stacksafe release that no longer needs the unmaintained crate.
