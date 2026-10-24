fn main() -> Result<(), embed_resource::CompilationResult> {
    assert_eq!(
        std::env::var("CARGO_CFG_TARGET_OS").as_deref(),
        Ok("windows"),
        "Latch supports Windows only"
    );
    println!("cargo:rerun-if-changed=assets/latch.rc");
    println!("cargo:rerun-if-changed=assets/latch.ico");
    println!("cargo:rerun-if-changed=Cargo.toml");
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
        && std::env::var("PROFILE").as_deref() == Ok("release")
        && std::env::var("DEBUG").as_deref() == Ok("false")
    {
        // rustc requests a PDB even when release debug information is disabled.
        println!("cargo:rustc-link-arg-bin=latch=/DEBUG:NONE");
    }
    let version_macros = [
        format!(
            "LATCH_VERSION_NUMERIC={},{},{},0",
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR"),
            env!("CARGO_PKG_VERSION_PATCH")
        ),
        format!("LATCH_VERSION_STRING=\"{}\"", env!("CARGO_PKG_VERSION")),
    ];
    embed_resource::compile_for("assets/latch.rc", ["latch"], version_macros).manifest_required()
}
