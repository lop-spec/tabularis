fn main() {
    // CI embeds a pinned gh-ost CLI; local compile-only checks use an empty stub.
    println!("cargo::rerun-if-env-changed=TABULARIS_GHOST_BINARY");
    println!("cargo::rerun-if-env-changed=TABULARIS_GHOST_LICENSE");
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    for (variable, file) in [
        ("TABULARIS_GHOST_BINARY", "gh-ost.bin"),
        ("TABULARIS_GHOST_LICENSE", "gh-ost.LICENSE"),
    ] {
        if let Some(source) = std::env::var_os(variable) {
            println!(
                "cargo::rerun-if-changed={}",
                std::path::Path::new(&source).display()
            );
            std::fs::copy(source, out.join(file)).expect("Could not embed gh-ost CI asset");
        } else {
            println!("cargo::warning={variable} is unset; Online DDL is unavailable in this build");
            std::fs::write(out.join(file), []).expect("Could not create gh-ost compile-only stub");
        }
    }
    // The unit-test executable statically imports comctl32!TaskDialogIndirect
    // (via a dialog dependency). That export only exists in common-controls
    // v6, which Windows activates through an application manifest. The Tauri
    // app exe gets one from tauri-build; the bare cargo test exe does not and
    // dies at load with STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139). Embed a
    // minimal manifest into test binaries so `cargo test` runs anywhere.
    #[cfg(all(target_os = "windows", target_env = "msvc"))]
    {
        let manifest =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests-common-controls.manifest");
        println!("cargo::rustc-link-arg-tests=/MANIFEST:EMBED");
        println!(
            "cargo::rustc-link-arg-tests=/MANIFESTINPUT:{}",
            manifest.display()
        );
        println!("cargo::rerun-if-changed=tests-common-controls.manifest");
    }

    tauri_build::build()
}
