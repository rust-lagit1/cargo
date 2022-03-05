//! Tests for setting custom rustc flags.

use cargo_test_support::registry::Package;
use cargo_test_support::{
    basic_lib_manifest, basic_manifest, paths, project, project_in_home, rustc_host,
};
use std::fs;

#[cargo_test]
fn env_rustflags_normal_source() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("tests/c.rs", "#[test] fn f() { }")
        .file(
            "benches/d.rs",
            r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run1(_ben: &mut test::Bencher) { }
            "#,
        )
        .build();

    // Use RUSTFLAGS to pass an argument that will generate an error
    p.cargo("build --lib")
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("build --bin=a")
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("build --example=b")
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("test")
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("bench")
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
}

#[cargo_test]
fn env_rustflags_variants() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                #[cfg(not(for_lib))]
                compile_error!("did not use the expected rustflags");
            "#,
        )
        .file(
            "build.rs",
            r#"
                fn main() { assert!(cfg!(for_brs)) }
            "#,
        )
        .build();
    let host = rustc_host();

    // First, check that the extended rustflags are feature-gated.
    p.cargo("build")
        .arg("--target")
        .arg(&host)
        .env(
            &format!("RUSTFLAGS_{}", host.replace('-', "_").to_uppercase()),
            "--cfg=for_brs --cfg=for_lib",
        )
        .with_status(101)
        .with_stderr_contains("[..]assertion failed: cfg!(for_brs)[..]")
        .run();
    // Without --target, both lib.rs and build.rs should pick up RUSTFLAGS.
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .arg("-Ztargeted-rustflags")
        .env("RUSTFLAGS", "--cfg=for_lib --cfg=for_brs")
        .run();
    // Same with CARGO_ENCODED_RUSTFLAGS.
    // NOTE: We just assume that CARGO_ENCODED_RUSTFLAGS works like RUSTFLAGS after this.
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .arg("-Ztargeted-rustflags")
        .env("CARGO_ENCODED_RUSTFLAGS", "--cfg=for_lib\x1f--cfg=for_brs")
        .run();
    // TARGET_RUSTFLAGS should _not_ affect the build script.
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .arg("-Ztargeted-rustflags")
        .env("TARGET_RUSTFLAGS", "--cfg=for_lib --cfg=for_brs")
        .with_status(101)
        .with_stderr_contains("[..]assertion failed: cfg!(for_brs)[..]")
        .run();
    // With --target, lib.rs should pick up RUSTFLAGS, but build.rs should not.
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .arg("-Ztargeted-rustflags")
        .arg("--target")
        .arg(&host)
        .env("RUSTFLAGS", "--cfg=for_lib --cfg=for_brs")
        .with_status(101)
        .with_stderr_contains("[..]assertion failed: cfg!(for_brs)[..]")
        .run();
    // build.rs picks up HOST_RUSTFLAGS (but lib does not).
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .arg("-Ztargeted-rustflags")
        .arg("--target")
        .arg(&host)
        .env("HOST_RUSTFLAGS", "--cfg=for_brs")
        .env("RUSTFLAGS", "--cfg=for_lib")
        .run();
    // lib.rs picks up TARGET_RUSTFLAGS too.
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .arg("-Ztargeted-rustflags")
        .arg("--target")
        .arg(&host)
        .env("HOST_RUSTFLAGS", "--cfg=for_brs")
        .env("TARGET_RUSTFLAGS", "--cfg=for_lib")
        .run();
    // Without `target-applies-to-host = false`, host picks up RUSTFLAGS_$HOST, and it takes
    // precedence over all the other flags.
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .arg("-Ztargeted-rustflags")
        .arg("--target")
        .arg(&host)
        .env(
            &format!("RUSTFLAGS_{}", host.replace('-', "_").to_uppercase()),
            "--cfg=for_brs --cfg=for_lib",
        )
        .env("HOST_RUSTFLAGS", "--cfg=host")
        .env("TARGET_RUSTFLAGS", "--cfg=target")
        .env("RUSTFLAGS", "--cfg=generic")
        .run();
    // But with `target-applies-to-host = false`, host _only_ picks up `HOST_RUSTFLAGS`.
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .arg("-Ztargeted-rustflags")
        .arg("-Ztarget-applies-to-host")
        .arg("-Zunstable-options")
        .arg("--config")
        .arg("target-applies-to-host=false")
        .arg("--target")
        .arg(&host)
        .env(
            &format!("RUSTFLAGS_{}", host.replace('-', "_").to_uppercase()),
            "--cfg=for_lib",
        )
        .env("HOST_RUSTFLAGS", "--cfg=for_brs")
        .env("RUSTFLAGS", "--cfg=generic")
        .run();
}

#[cargo_test]
fn env_rustflags_specificity() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                #[cfg(not(expected))]
                compile_error!("did not use the expected rustflags");
            "#,
        )
        .build();
    let host = rustc_host();

    // CARGO_ENCODED_RUSTFLAGS should be preferred over RUSTFLAGS.
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .arg("-Ztargeted-rustflags")
        .env("CARGO_ENCODED_RUSTFLAGS", "--cfg=expected")
        .env("RUSTFLAGS", "--cfg=rustflags")
        .run();
    // RUSTFLAGS with target specified should be preferred over CARGO_ENCODED_RUSTFLAGS.
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .arg("-Ztargeted-rustflags")
        .env(
            &format!("RUSTFLAGS_{}", host.replace('-', "_").to_uppercase()),
            "--cfg=expected",
        )
        .env("CARGO_ENCODED_RUSTFLAGS", "--cfg=encoded")
        .run();
    // Same with TARGET_RUSTFLAGS over CARGO_ENCODED_RUSTFLAGS.
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .arg("-Ztargeted-rustflags")
        .arg("--target")
        .arg(&host)
        .env("TARGET_RUSTFLAGS", "--cfg=expected")
        .env("CARGO_ENCODED_RUSTFLAGS", "--cfg=encoded")
        .run();
    // But CARGO_ENCODED_RUSTFLAGS should win ties.
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .arg("-Ztargeted-rustflags")
        .env(
            &format!(
                "CARGO_ENCODED_RUSTFLAGS_{}",
                host.replace('-', "_").to_uppercase()
            ),
            "--cfg=expected",
        )
        .env(
            &format!("RUSTFLAGS_{}", host.replace('-', "_").to_uppercase()),
            "--cfg=rustflags",
        )
        .run();
}

#[cargo_test]
fn env_rustflags_build_script() {
    // RUSTFLAGS should be passed to rustc for build scripts
    // when --target is not specified.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() { assert!(cfg!(foo)); }
            "#,
        )
        .build();

    p.cargo("build").env("RUSTFLAGS", "--cfg foo").run();
}

#[cargo_test]
fn env_rustflags_build_script_dep() {
    // RUSTFLAGS should be passed to rustc for build scripts
    // when --target is not specified.
    // In this test if --cfg foo is not passed the build will fail.
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                build = "build.rs"

                [build-dependencies.bar]
                path = "../bar"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();
    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file(
            "src/lib.rs",
            r#"
                fn bar() { }
                #[cfg(not(foo))]
                fn bar() { }
            "#,
        )
        .build();

    foo.cargo("build").env("RUSTFLAGS", "--cfg foo").run();
}

#[cargo_test]
fn env_rustflags_plugin() {
    // RUSTFLAGS should be passed to rustc for plugins
    // when --target is not specified.
    // In this test if --cfg foo is not passed the build will fail.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [lib]
                name = "foo"
                plugin = true
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                fn main() { }
                #[cfg(not(foo))]
                fn main() { }
            "#,
        )
        .build();

    p.cargo("build").env("RUSTFLAGS", "--cfg foo").run();
}

#[cargo_test]
fn env_rustflags_plugin_dep() {
    // RUSTFLAGS should be passed to rustc for plugins
    // when --target is not specified.
    // In this test if --cfg foo is not passed the build will fail.
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [lib]
                name = "foo"
                plugin = true

                [dependencies.bar]
                path = "../bar"
            "#,
        )
        .file("src/lib.rs", "fn foo() {}")
        .build();
    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_lib_manifest("bar"))
        .file(
            "src/lib.rs",
            r#"
                fn bar() { }
                #[cfg(not(foo))]
                fn bar() { }
            "#,
        )
        .build();

    foo.cargo("build").env("RUSTFLAGS", "--cfg foo").run();
}

#[cargo_test]
fn env_rustflags_normal_source_with_target() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("tests/c.rs", "#[test] fn f() { }")
        .file(
            "benches/d.rs",
            r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run1(_ben: &mut test::Bencher) { }
            "#,
        )
        .build();

    let host = &rustc_host();

    // Use RUSTFLAGS to pass an argument that will generate an error
    p.cargo("build --lib --target")
        .arg(host)
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("build --bin=a --target")
        .arg(host)
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("build --example=b --target")
        .arg(host)
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("test --target")
        .arg(host)
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("bench --target")
        .arg(host)
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
}

#[cargo_test]
fn env_rustflags_build_script_with_target() {
    // RUSTFLAGS should not be passed to rustc for build scripts
    // when --target is specified.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() { assert!(!cfg!(foo)); }
            "#,
        )
        .build();

    let host = rustc_host();
    p.cargo("build --target")
        .arg(host)
        .env("RUSTFLAGS", "--cfg foo")
        .run();
}

#[cargo_test]
fn env_rustflags_build_script_with_target_doesnt_apply_to_host_kind() {
    // RUSTFLAGS should *not* be passed to rustc for build scripts when --target is specified as the
    // host triple even if target-applies-to-host-kind is enabled, to match legacy Cargo behavior.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() { assert!(!cfg!(foo)); }
            "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
                target-applies-to-host = true
            "#,
        )
        .build();

    let host = rustc_host();
    p.cargo("build --target")
        .masquerade_as_nightly_cargo()
        .arg(host)
        .arg("-Ztarget-applies-to-host")
        .env("RUSTFLAGS", "--cfg foo")
        .run();
}

#[cargo_test]
fn env_rustflags_build_script_dep_with_target() {
    // RUSTFLAGS should not be passed to rustc for build scripts
    // when --target is specified.
    // In this test if --cfg foo is passed the build will fail.
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                build = "build.rs"

                [build-dependencies.bar]
                path = "../bar"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();
    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file(
            "src/lib.rs",
            r#"
                fn bar() { }
                #[cfg(foo)]
                fn bar() { }
            "#,
        )
        .build();

    let host = rustc_host();
    foo.cargo("build --target")
        .arg(host)
        .env("RUSTFLAGS", "--cfg foo")
        .run();
}

#[cargo_test]
fn env_rustflags_plugin_with_target() {
    // RUSTFLAGS should not be passed to rustc for plugins
    // when --target is specified.
    // In this test if --cfg foo is passed the build will fail.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [lib]
                name = "foo"
                plugin = true
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                fn main() { }
                #[cfg(foo)]
                fn main() { }
            "#,
        )
        .build();

    let host = rustc_host();
    p.cargo("build --target")
        .arg(host)
        .env("RUSTFLAGS", "--cfg foo")
        .run();
}

#[cargo_test]
fn env_rustflags_plugin_dep_with_target() {
    // RUSTFLAGS should not be passed to rustc for plugins
    // when --target is specified.
    // In this test if --cfg foo is passed the build will fail.
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [lib]
                name = "foo"
                plugin = true

                [dependencies.bar]
                path = "../bar"
            "#,
        )
        .file("src/lib.rs", "fn foo() {}")
        .build();
    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_lib_manifest("bar"))
        .file(
            "src/lib.rs",
            r#"
                fn bar() { }
                #[cfg(foo)]
                fn bar() { }
            "#,
        )
        .build();

    let host = rustc_host();
    foo.cargo("build --target")
        .arg(host)
        .env("RUSTFLAGS", "--cfg foo")
        .run();
}

#[cargo_test]
fn env_rustflags_recompile() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("build").run();
    // Setting RUSTFLAGS forces a recompile
    p.cargo("build")
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
}

#[cargo_test]
fn env_rustflags_recompile2() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("build").env("RUSTFLAGS", "--cfg foo").run();
    // Setting RUSTFLAGS forces a recompile
    p.cargo("build")
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
}

#[cargo_test]
fn env_rustflags_no_recompile() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("build").env("RUSTFLAGS", "--cfg foo").run();
    p.cargo("build")
        .env("RUSTFLAGS", "--cfg foo")
        .with_stdout("")
        .run();
}

#[cargo_test]
fn build_rustflags_normal_source() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("tests/c.rs", "#[test] fn f() { }")
        .file(
            "benches/d.rs",
            r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run1(_ben: &mut test::Bencher) { }
            "#,
        )
        .file(
            ".cargo/config",
            r#"
            [build]
            rustflags = ["-Z", "bogus"]
            "#,
        )
        .build();

    p.cargo("build --lib")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("build --bin=a")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("build --example=b")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("test")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("bench")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
}

#[cargo_test]
fn build_rustflags_build_script() {
    // RUSTFLAGS should be passed to rustc for build scripts
    // when --target is not specified.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() { assert!(cfg!(foo)); }
            "#,
        )
        .file(
            ".cargo/config",
            r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#,
        )
        .build();

    p.cargo("build").run();
}

#[cargo_test]
fn build_rustflags_build_script_dep() {
    // RUSTFLAGS should be passed to rustc for build scripts
    // when --target is not specified.
    // In this test if --cfg foo is not passed the build will fail.
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                build = "build.rs"

                [build-dependencies.bar]
                path = "../bar"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#,
        )
        .build();
    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file(
            "src/lib.rs",
            r#"
                fn bar() { }
                #[cfg(not(foo))]
                fn bar() { }
            "#,
        )
        .build();

    foo.cargo("build").run();
}

#[cargo_test]
fn build_rustflags_plugin() {
    // RUSTFLAGS should be passed to rustc for plugins
    // when --target is not specified.
    // In this test if --cfg foo is not passed the build will fail.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [lib]
                name = "foo"
                plugin = true
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                fn main() { }
                #[cfg(not(foo))]
                fn main() { }
            "#,
        )
        .file(
            ".cargo/config",
            r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#,
        )
        .build();

    p.cargo("build").run();
}

#[cargo_test]
fn build_rustflags_plugin_dep() {
    // RUSTFLAGS should be passed to rustc for plugins
    // when --target is not specified.
    // In this test if --cfg foo is not passed the build will fail.
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [lib]
                name = "foo"
                plugin = true

                [dependencies.bar]
                path = "../bar"
            "#,
        )
        .file("src/lib.rs", "fn foo() {}")
        .file(
            ".cargo/config",
            r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#,
        )
        .build();
    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_lib_manifest("bar"))
        .file(
            "src/lib.rs",
            r#"
                fn bar() { }
                #[cfg(not(foo))]
                fn bar() { }
            "#,
        )
        .build();

    foo.cargo("build").run();
}

#[cargo_test]
fn build_rustflags_normal_source_with_target() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("tests/c.rs", "#[test] fn f() { }")
        .file(
            "benches/d.rs",
            r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run1(_ben: &mut test::Bencher) { }
            "#,
        )
        .file(
            ".cargo/config",
            r#"
            [build]
            rustflags = ["-Z", "bogus"]
            "#,
        )
        .build();

    let host = &rustc_host();

    // Use build.rustflags to pass an argument that will generate an error
    p.cargo("build --lib --target")
        .arg(host)
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("build --bin=a --target")
        .arg(host)
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("build --example=b --target")
        .arg(host)
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("test --target")
        .arg(host)
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("bench --target")
        .arg(host)
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
}

#[cargo_test]
fn build_rustflags_build_script_with_target() {
    // RUSTFLAGS should not be passed to rustc for build scripts
    // when --target is specified.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() { assert!(!cfg!(foo)); }
            "#,
        )
        .file(
            ".cargo/config",
            r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#,
        )
        .build();

    let host = rustc_host();
    p.cargo("build --target").arg(host).run();
}

#[cargo_test]
fn build_rustflags_build_script_dep_with_target() {
    // RUSTFLAGS should not be passed to rustc for build scripts
    // when --target is specified.
    // In this test if --cfg foo is passed the build will fail.
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                build = "build.rs"

                [build-dependencies.bar]
                path = "../bar"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#,
        )
        .build();
    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file(
            "src/lib.rs",
            r#"
                fn bar() { }
                #[cfg(foo)]
                fn bar() { }
            "#,
        )
        .build();

    let host = rustc_host();
    foo.cargo("build --target").arg(host).run();
}

#[cargo_test]
fn build_rustflags_plugin_with_target() {
    // RUSTFLAGS should not be passed to rustc for plugins
    // when --target is specified.
    // In this test if --cfg foo is passed the build will fail.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [lib]
                name = "foo"
                plugin = true
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                fn main() { }
                #[cfg(foo)]
                fn main() { }
            "#,
        )
        .file(
            ".cargo/config",
            r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#,
        )
        .build();

    let host = rustc_host();
    p.cargo("build --target").arg(host).run();
}

#[cargo_test]
fn build_rustflags_plugin_dep_with_target() {
    // RUSTFLAGS should not be passed to rustc for plugins
    // when --target is specified.
    // In this test if --cfg foo is passed the build will fail.
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [lib]
                name = "foo"
                plugin = true

                [dependencies.bar]
                path = "../bar"
            "#,
        )
        .file("src/lib.rs", "fn foo() {}")
        .file(
            ".cargo/config",
            r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#,
        )
        .build();
    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_lib_manifest("bar"))
        .file(
            "src/lib.rs",
            r#"
                fn bar() { }
                #[cfg(foo)]
                fn bar() { }
            "#,
        )
        .build();

    let host = rustc_host();
    foo.cargo("build --target").arg(host).run();
}

#[cargo_test]
fn build_rustflags_recompile() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("build").run();

    // Setting RUSTFLAGS forces a recompile
    let config = r#"
        [build]
        rustflags = ["-Z", "bogus"]
        "#;
    let config_file = paths::root().join("foo/.cargo/config");
    fs::create_dir_all(config_file.parent().unwrap()).unwrap();
    fs::write(config_file, config).unwrap();

    p.cargo("build")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
}

#[cargo_test]
fn build_rustflags_recompile2() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("build").env("RUSTFLAGS", "--cfg foo").run();

    // Setting RUSTFLAGS forces a recompile
    let config = r#"
        [build]
        rustflags = ["-Z", "bogus"]
        "#;
    let config_file = paths::root().join("foo/.cargo/config");
    fs::create_dir_all(config_file.parent().unwrap()).unwrap();
    fs::write(config_file, config).unwrap();

    p.cargo("build")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
}

#[cargo_test]
fn build_rustflags_no_recompile() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#,
        )
        .build();

    p.cargo("build").env("RUSTFLAGS", "--cfg foo").run();
    p.cargo("build")
        .env("RUSTFLAGS", "--cfg foo")
        .with_stdout("")
        .run();
}

#[cargo_test]
fn build_rustflags_with_home_config() {
    // We need a config file inside the home directory
    let home = paths::home();
    let home_config = home.join(".cargo");
    fs::create_dir(&home_config).unwrap();
    fs::write(
        &home_config.join("config"),
        r#"
            [build]
            rustflags = ["-Cllvm-args=-x86-asm-syntax=intel"]
        "#,
    )
    .unwrap();

    // And we need the project to be inside the home directory
    // so the walking process finds the home project twice.
    let p = project_in_home("foo").file("src/lib.rs", "").build();

    p.cargo("build -v").run();
}

#[cargo_test]
fn target_rustflags_normal_source() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("tests/c.rs", "#[test] fn f() { }")
        .file(
            "benches/d.rs",
            r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run1(_ben: &mut test::Bencher) { }
            "#,
        )
        .file(
            ".cargo/config",
            &format!(
                "
            [target.{}]
            rustflags = [\"-Z\", \"bogus\"]
            ",
                rustc_host()
            ),
        )
        .build();

    p.cargo("build --lib")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("build --bin=a")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("build --example=b")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("test")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("bench")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
}

#[cargo_test]
fn target_rustflags_also_for_build_scripts() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() { assert!(cfg!(foo)); }
            "#,
        )
        .file(
            ".cargo/config",
            &format!(
                "
            [target.{}]
            rustflags = [\"--cfg=foo\"]
            ",
                rustc_host()
            ),
        )
        .build();

    p.cargo("build").run();
}

#[cargo_test]
fn target_rustflags_not_for_build_scripts_with_target() {
    let host = rustc_host();
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() { assert!(!cfg!(foo)); }
            "#,
        )
        .file(
            ".cargo/config",
            &format!(
                "
            [target.{}]
            rustflags = [\"--cfg=foo\"]
            ",
                host
            ),
        )
        .build();

    p.cargo("build --target").arg(host).run();

    // Enabling -Ztarget-applies-to-host should not make a difference without the config setting
    p.cargo("build --target")
        .arg(host)
        .masquerade_as_nightly_cargo()
        .arg("-Ztarget-applies-to-host")
        .run();

    // Even with the setting, the rustflags from `target.` should not apply, to match the legacy
    // Cargo behavior.
    p.change_file(
        ".cargo/config",
        &format!(
            "
        target-applies-to-host = true

        [target.{}]
        rustflags = [\"--cfg=foo\"]
        ",
            host
        ),
    );
    p.cargo("build --target")
        .arg(host)
        .masquerade_as_nightly_cargo()
        .arg("-Ztarget-applies-to-host")
        .run();
}

#[cargo_test]
fn build_rustflags_for_build_scripts() {
    let host = rustc_host();
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() { assert!(cfg!(foo)); }
            "#,
        )
        .file(
            ".cargo/config",
            "
            [build]
            rustflags = [\"--cfg=foo\"]
            ",
        )
        .build();

    // With "legacy" behavior, build.rustflags should apply to build scripts without --target
    p.cargo("build").run();

    // But should _not_ apply _with_ --target
    p.cargo("build --target")
        .arg(host)
        .with_status(101)
        .with_stderr_contains("[..]assertion failed[..]")
        .run();

    // Enabling -Ztarget-applies-to-host should not make a difference without the config setting
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .arg("-Ztarget-applies-to-host")
        .run();
    p.cargo("build --target")
        .arg(host)
        .masquerade_as_nightly_cargo()
        .arg("-Ztarget-applies-to-host")
        .with_status(101)
        .with_stderr_contains("[..]assertion failed[..]")
        .run();

    // When set to false though, the "proper" behavior where host artifacts _only_ pick up on
    // [host] should be applied.
    p.change_file(
        ".cargo/config",
        "
        target-applies-to-host = false

        [build]
        rustflags = [\"--cfg=foo\"]
        ",
    );
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .arg("-Ztarget-applies-to-host")
        .with_status(101)
        .with_stderr_contains("[..]assertion failed[..]")
        .run();
    p.cargo("build --target")
        .arg(host)
        .masquerade_as_nightly_cargo()
        .arg("-Ztarget-applies-to-host")
        .with_status(101)
        .with_stderr_contains("[..]assertion failed[..]")
        .run();
}

#[cargo_test]
fn host_rustflags_for_build_scripts() {
    let host = rustc_host();
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                // Ensure that --cfg=foo is passed.
                fn main() { assert!(cfg!(foo)); }
            "#,
        )
        .file(
            ".cargo/config",
            &format!(
                "
                target-applies-to-host = false

                [host.{}]
                rustflags = [\"--cfg=foo\"]
                ",
                host
            ),
        )
        .build();

    p.cargo("build --target")
        .arg(host)
        .masquerade_as_nightly_cargo()
        .arg("-Ztarget-applies-to-host")
        .arg("-Zhost-config")
        .run();
}

// target.{}.rustflags takes precedence over build.rustflags
#[cargo_test]
fn target_rustflags_precedence() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("tests/c.rs", "#[test] fn f() { }")
        .file(
            ".cargo/config",
            &format!(
                "
            [build]
            rustflags = [\"--cfg\", \"foo\"]

            [target.{}]
            rustflags = [\"-Z\", \"bogus\"]
            ",
                rustc_host()
            ),
        )
        .build();

    p.cargo("build --lib")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("build --bin=a")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("build --example=b")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("test")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
    p.cargo("bench")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
}

#[cargo_test]
fn cfg_rustflags_normal_source() {
    let p = project()
        .file("src/lib.rs", "pub fn t() {}")
        .file("src/bin/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("tests/c.rs", "#[test] fn f() { }")
        .file(
            ".cargo/config",
            &format!(
                r#"
                [target.'cfg({})']
                rustflags = ["--cfg", "bar"]
                "#,
                if rustc_host().contains("-windows-") {
                    "windows"
                } else {
                    "not(windows)"
                }
            ),
        )
        .build();

    p.cargo("build --lib -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg bar[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.cargo("build --bin=a -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg bar[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.cargo("build --example=b -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg bar[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.cargo("test --no-run -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg bar[..]`
[RUNNING] `rustc [..] --cfg bar[..]`
[RUNNING] `rustc [..] --cfg bar[..]`
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[EXECUTABLE] `[..]/target/debug/deps/foo-[..][EXE]`
[EXECUTABLE] `[..]/target/debug/deps/a-[..][EXE]`
[EXECUTABLE] `[..]/target/debug/deps/c-[..][EXE]`
",
        )
        .run();

    p.cargo("bench --no-run -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg bar[..]`
[RUNNING] `rustc [..] --cfg bar[..]`
[RUNNING] `rustc [..] --cfg bar[..]`
[FINISHED] bench [optimized] target(s) in [..]
[EXECUTABLE] `[..]/target/release/deps/foo-[..][EXE]`
[EXECUTABLE] `[..]/target/release/deps/a-[..][EXE]`
",
        )
        .run();
}

// target.'cfg(...)'.rustflags takes precedence over build.rustflags
#[cargo_test]
fn cfg_rustflags_precedence() {
    let p = project()
        .file("src/lib.rs", "pub fn t() {}")
        .file("src/bin/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("tests/c.rs", "#[test] fn f() { }")
        .file(
            ".cargo/config",
            &format!(
                r#"
                [build]
                rustflags = ["--cfg", "foo"]

                [target.'cfg({})']
                rustflags = ["--cfg", "bar"]
                "#,
                if rustc_host().contains("-windows-") {
                    "windows"
                } else {
                    "not(windows)"
                }
            ),
        )
        .build();

    p.cargo("build --lib -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg bar[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.cargo("build --bin=a -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg bar[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.cargo("build --example=b -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg bar[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.cargo("test --no-run -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg bar[..]`
[RUNNING] `rustc [..] --cfg bar[..]`
[RUNNING] `rustc [..] --cfg bar[..]`
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[EXECUTABLE] `[..]/target/debug/deps/foo-[..][EXE]`
[EXECUTABLE] `[..]/target/debug/deps/a-[..][EXE]`
[EXECUTABLE] `[..]/target/debug/deps/c-[..][EXE]`
",
        )
        .run();

    p.cargo("bench --no-run -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg bar[..]`
[RUNNING] `rustc [..] --cfg bar[..]`
[RUNNING] `rustc [..] --cfg bar[..]`
[FINISHED] bench [optimized] target(s) in [..]
[EXECUTABLE] `[..]/target/release/deps/foo-[..][EXE]`
[EXECUTABLE] `[..]/target/release/deps/a-[..][EXE]`
",
        )
        .run();
}

#[cargo_test]
fn target_rustflags_string_and_array_form1() {
    let p1 = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#,
        )
        .build();

    p1.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg foo[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    let p2 = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [build]
            rustflags = "--cfg foo"
            "#,
        )
        .build();

    p2.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg foo[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn target_rustflags_string_and_array_form2() {
    let p1 = project()
        .file(
            ".cargo/config",
            &format!(
                r#"
                    [target.{}]
                    rustflags = ["--cfg", "foo"]
                "#,
                rustc_host()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p1.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg foo[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    let p2 = project()
        .file(
            ".cargo/config",
            &format!(
                r#"
                    [target.{}]
                    rustflags = "--cfg foo"
                "#,
                rustc_host()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p2.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] --cfg foo[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn two_matching_in_config() {
    let p1 = project()
        .file(
            ".cargo/config",
            r#"
                [target.'cfg(unix)']
                rustflags = ["--cfg", 'foo="a"']
                [target.'cfg(windows)']
                rustflags = ["--cfg", 'foo="a"']
                [target.'cfg(target_pointer_width = "32")']
                rustflags = ["--cfg", 'foo="b"']
                [target.'cfg(target_pointer_width = "64")']
                rustflags = ["--cfg", 'foo="b"']
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    if cfg!(foo = "a") {
                        println!("a");
                    } else if cfg!(foo = "b") {
                        println!("b");
                    } else {
                        panic!()
                    }
                }
            "#,
        )
        .build();

    p1.cargo("run").run();
    p1.cargo("build").with_stderr("[FINISHED] [..]").run();
}

#[cargo_test]
fn env_rustflags_misspelled() {
    let p = project().file("src/main.rs", "fn main() { }").build();

    for cmd in &["check", "build", "run", "test", "bench"] {
        p.cargo(cmd)
            .env("RUST_FLAGS", "foo")
            .with_stderr_contains("[WARNING] Cargo does not read `RUST_FLAGS` environment variable. Did you mean `RUSTFLAGS`?")
            .run();
    }
}

#[cargo_test]
fn env_rustflags_misspelled_build_script() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() { }")
        .build();

    p.cargo("build")
        .env("RUST_FLAGS", "foo")
        .with_stderr_contains("[WARNING] Cargo does not read `RUST_FLAGS` environment variable. Did you mean `RUSTFLAGS`?")
        .run();
}

#[cargo_test]
fn remap_path_prefix_ignored() {
    // Ensure that --remap-path-prefix does not affect metadata hash.
    let p = project().file("src/lib.rs", "").build();
    p.cargo("build").run();
    let rlibs = p
        .glob("target/debug/deps/*.rlib")
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(rlibs.len(), 1);
    p.cargo("clean").run();

    let check_metadata_same = || {
        let rlibs2 = p
            .glob("target/debug/deps/*.rlib")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(rlibs, rlibs2);
    };

    p.cargo("build")
        .env(
            "RUSTFLAGS",
            "--remap-path-prefix=/abc=/zoo --remap-path-prefix /spaced=/zoo",
        )
        .run();
    check_metadata_same();

    p.cargo("clean").run();
    p.cargo("rustc -- --remap-path-prefix=/abc=/zoo --remap-path-prefix /spaced=/zoo")
        .run();
    check_metadata_same();
}

#[cargo_test]
fn remap_path_prefix_works() {
    // Check that remap-path-prefix works.
    Package::new("bar", "0.1.0")
        .file("src/lib.rs", "pub fn f() -> &'static str { file!() }")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            bar = "0.1"
            "#,
        )
        .file(
            "src/main.rs",
            r#"
            fn main() {
                println!("{}", bar::f());
            }
            "#,
        )
        .build();

    p.cargo("run")
        .env(
            "RUSTFLAGS",
            format!("--remap-path-prefix={}=/foo", paths::root().display()),
        )
        .with_stdout("/foo/home/.cargo/registry/src/[..]/bar-0.1.0/src/lib.rs")
        .run();
}

#[cargo_test]
fn host_config_rustflags_with_target() {
    // regression test for https://github.com/rust-lang/cargo/issues/10206
    let p = project()
        .file("src/lib.rs", "")
        .file("build.rs.rs", "fn main() { assert!(cfg!(foo)); }")
        .file(".cargo/config.toml", "target-applies-to-host = false")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .arg("-Zhost-config")
        .arg("-Ztarget-applies-to-host")
        .arg("-Zunstable-options")
        .arg("--config")
        .arg("host.rustflags=[\"--cfg=foo\"]")
        .run();
}
