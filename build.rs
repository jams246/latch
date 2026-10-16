fn main() -> Result<(), embed_resource::CompilationResult> {
    println!("cargo:rerun-if-changed=assets/latch.rc");
    println!("cargo:rerun-if-changed=assets/latch.ico");
    embed_resource::compile_for("assets/latch.rc", ["latch"], embed_resource::NONE)
        .manifest_required()
}
