// Sheaf: `onnxruntime` is set where ort publishes a prebuilt CPU library to link statically (the
// same targets as the `ort` dependency in Cargo.toml); elsewhere rten reads the lines.
fn main() {
    println!("cargo::rustc-check-cfg=cfg(onnxruntime)");
    let var = |k: &str| std::env::var(k).unwrap_or_default();
    let (os, arch, env) = (var("CARGO_CFG_TARGET_OS"), var("CARGO_CFG_TARGET_ARCH"), var("CARGO_CFG_TARGET_ENV"));
    let prebuilt = match os.as_str() {
        "linux" => arch == "x86_64" || arch == "aarch64",
        "macos" => arch == "aarch64",
        "windows" => arch == "x86_64" && env == "msvc",
        _ => false,
    };
    if prebuilt {
        println!("cargo::rustc-cfg=onnxruntime");
    }
}
