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
    // onnxruntime's CoreML code checks the macOS version (`@available`), which calls
    // ___isPlatformVersionAtLeast from clang's runtime; rustc does not link that runtime, and a build
    // for an older macOS than the SDK's (the release targets macOS 11) needs it.
    if prebuilt && os == "macos" {
        if let Ok(out) = std::process::Command::new("clang").arg("--print-resource-dir").output() {
            let dir = String::from_utf8_lossy(&out.stdout).trim().to_string();
            println!("cargo::rustc-link-search=native={dir}/lib/darwin");
            println!("cargo::rustc-link-lib=static=clang_rt.osx");
        }
    }
}
