fn main() {
    // The unit-test executable statically imports comctl32!TaskDialogIndirect
    // (via a dialog dependency). That export only exists in common-controls
    // v6, which Windows activates through an application manifest. The Tauri
    // app exe gets one from tauri-build; the bare cargo test exe does not and
    // dies at load with STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139). Embed a
    // minimal manifest into test binaries so `cargo test` runs anywhere.
    #[cfg(all(target_os = "windows", target_env = "msvc"))]
    {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests-common-controls.manifest");
        println!("cargo::rustc-link-arg-tests=/MANIFEST:EMBED");
        println!(
            "cargo::rustc-link-arg-tests=/MANIFESTINPUT:{}",
            manifest.display()
        );
        println!("cargo::rerun-if-changed=tests-common-controls.manifest");
    }

    tauri_build::build()
}
