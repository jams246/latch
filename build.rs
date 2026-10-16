fn main() -> Result<(), embed_resource::CompilationResult> {
    println!("cargo:rerun-if-changed=assets/latch.rc");
    println!("cargo:rerun-if-changed=assets/latch.ico");
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
        && std::env::var("PROFILE").as_deref() == Ok("release")
        && std::env::var("DEBUG").as_deref() == Ok("false")
    {
        // rustc requests a PDB even when release debug information is disabled.
        println!("cargo:rustc-link-arg-bin=latch=/DEBUG:NONE");
    }
    embed_resource::compile_for("assets/latch.rc", ["latch"], embed_resource::NONE)
        .manifest_required()
}
