//! Tests for unstable `patch-files` feature.

use cargo_test_support::basic_manifest;
use cargo_test_support::compare::assert_e2e;
use cargo_test_support::git;
use cargo_test_support::paths;
use cargo_test_support::prelude::*;
use cargo_test_support::project;
use cargo_test_support::registry;
use cargo_test_support::registry::Package;
use cargo_test_support::str;
use cargo_test_support::Project;

const HELLO_PATCH: &'static str = r#"
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -0,0 +1,3 @@
+pub fn hello() {
+    println!("Hello, patched!")
+}
"#;

const PATCHTOOL: &'static str = r#"
[patchtool]
path = ["patch", "-N", "-p1", "-i"]
"#;

/// Helper to create a package with a patch.
fn patched_project() -> Project {
    Package::new("bar", "1.0.0").publish();
    project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["patch-files"]

                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                bar = "1"

                [patch.crates-io]
                bar = { patches = ["patches/hello.patch"] }
            "#,
        )
        .file("src/main.rs", "fn main() { bar::hello(); }")
        .file("patches/hello.patch", HELLO_PATCH)
        .file(".cargo/config.toml", PATCHTOOL)
        .build()
}

#[cargo_test]
fn gated_manifest() {
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                bar = "1"

                [patch.crates-io]
                bar = { patches = [] }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] ignoring `patches` on patch for `bar` in `https://github.com/rust-lang/crates.io-index`; see https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#patch-files about the status of this feature.
[UPDATING] `dummy-registry` index
[ERROR] failed to resolve patches for `https://github.com/rust-lang/crates.io-index`

Caused by:
  patch for `bar` in `https://github.com/rust-lang/crates.io-index` points to the same source, but patches must point to different sources

"#]])
        .run();
}

#[cargo_test]
fn gated_config() {
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                bar = "1"

                [patch.crates-io]
                bar = { patches = [] }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [patch.crates-io]
                bar = { patches = [] }
            "#,
        )
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] ignoring `patches` on patch for `bar` in `https://github.com/rust-lang/crates.io-index`; see https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#patch-files about the status of this feature.
[WARNING] [patch] in cargo config: ignoring `patches` on patch for `bar` in `https://github.com/rust-lang/crates.io-index`; see https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#patch-files about the status of this feature.
[UPDATING] `dummy-registry` index
[ERROR] failed to resolve patches for `https://github.com/rust-lang/crates.io-index`

Caused by:
  patch for `bar` in `https://github.com/rust-lang/crates.io-index` points to the same source, but patches must point to different sources

"#]])
        .run();
}

#[cargo_test]
fn warn_if_in_normal_dep() {
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                bar = { version = "1", patches = [] }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] unused manifest key: dependencies.bar.patches; see https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#patch-files about the status of this feature.
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[CHECKING] bar v1.0.0
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn disallow_empty_patches_array() {
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["patch-files"]

                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                bar = "1"

                [patch.crates-io]
                bar = { patches = [] }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  patch for `bar` in `https://github.com/rust-lang/crates.io-index` requires at least one patch file when patching with patch files

"#]])
        .run();
}

#[cargo_test]
fn disallow_mismatched_source_url() {
    registry::alt_init();
    Package::new("bar", "1.0.0").alternative(true).publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["patch-files"]

                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                bar = "1"

                [patch.crates-io]
                bar = { registry = "alternative", patches = [] }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  patch for `bar` in `https://github.com/rust-lang/crates.io-index` must refer to the same source when patching with patch files

"#]])
        .run();
}

#[cargo_test]
fn disallow_path_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["patch-files"]

                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                bar = "1"

                [patch.crates-io]
                bar = { path = "bar", patches = [""] }
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "1.0.0"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  patch for `bar` in `https://github.com/rust-lang/crates.io-index` requires a registry source when patching with patch files

"#]])
        .run();
}

#[cargo_test]
fn disallow_git_dep() {
    let git = git::repo(&paths::root().join("bar"))
        .file("Cargo.toml", &basic_manifest("bar", "1.0.0"))
        .file("src/lib.rs", "")
        .build();
    let url = git.url();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                cargo-features = ["patch-files"]

                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                bar = "1"

                [patch.crates-io]
                bar = {{ git = "{url}", patches = [""] }}
                "#
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  patch for `bar` in `https://github.com/rust-lang/crates.io-index` requires a registry source when patching with patch files

"#]])
        .run();
}

#[cargo_test(requires_patch)]
fn patch() {
    let p = patched_project();

    p.cargo("run")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[PATCHING] bar v1.0.0
[LOCKING] 2 packages to latest compatible versions
[COMPILING] bar v1.0.0 (bar@1.0.0 with 1 patch file)
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .with_stdout_data(str![[r#"
Hello, patched!

"#]])
        .run();

    assert_e2e().eq(p.read_lockfile(), str![[r##"
# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "bar"
version = "1.0.0"
source = "patched+registry+https://github.com/rust-lang/crates.io-index?name=bar&version=1.0.0&patch=patches%2Fhello.patch"

[[package]]
name = "foo"
version = "0.0.0"
dependencies = [
 "bar",
]

"##]]);
}

#[cargo_test(requires_patch)]
fn patch_in_config() {
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                bar = "1"
            "#,
        )
        .file("src/main.rs", "fn main() { bar::hello(); }")
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                [patch.crates-io]
                bar = {{ patches = ["patches/hello.patch"] }}
                {PATCHTOOL}
            "#
            ),
        )
        .file("patches/hello.patch", HELLO_PATCH)
        .build();

    p.cargo("run -Zpatch-files")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[PATCHING] bar v1.0.0
[LOCKING] 2 packages to latest compatible versions
[COMPILING] bar v1.0.0 (bar@1.0.0 with 1 patch file)
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .with_stdout_data(str![[r#"
Hello, patched!

"#]])
        .run();
}

#[cargo_test(requires_patch)]
fn patch_for_alternative_registry() {
    registry::alt_init();
    Package::new("bar", "1.0.0").alternative(true).publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["patch-files"]

                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                bar = { version = "1", registry = "alternative" }

                [patch.alternative]
                bar = { registry = "alternative", patches = ["patches/hello.patch"] }
            "#,
        )
        .file("src/main.rs", "fn main() { bar::hello(); }")
        .file("patches/hello.patch", HELLO_PATCH)
        .file(".cargo/config.toml", PATCHTOOL)
        .build();

    p.cargo("run")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `alternative`)
[PATCHING] bar v1.0.0 (registry `alternative`)
[LOCKING] 2 packages to latest compatible versions
[COMPILING] bar v1.0.0 (bar@1.0.0 with 1 patch file)
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .with_stdout_data(str![[r#"
Hello, patched!

"#]])
        .run();
}

#[cargo_test(requires_patch)]
fn patch_manifest_add_dep() {
    Package::new("bar", "1.0.0").publish();
    Package::new("baz", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["patch-files"]

                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                bar = "1"

                [patch.crates-io]
                bar = { patches = ["patches/add-baz.patch"] }
            "#,
        )
        .file("src/main.rs", "fn main() { }")
        .file(
            "patches/add-baz.patch",
            r#"
                --- a/Cargo.toml
                +++ b/Cargo.toml
                @@ -3,4 +3,5 @@
                             name = "bar"
                             version = "1.0.0"
                -            authors = []
                +            [dependencies]
                +            baz = "1"

                ---
            "#,
        )
        .file(".cargo/config.toml", PATCHTOOL)
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[PATCHING] bar v1.0.0
[LOCKING] 3 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] baz v1.0.0 (registry `dummy-registry`)
[CHECKING] baz v1.0.0
[CHECKING] bar v1.0.0 (bar@1.0.0 with 1 patch file)
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(requires_patch)]
fn patch_package_version_match_semver_compat() {
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["patch-files"]

                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                bar = "1"

                [patch.crates-io]
                bar = { version = "1", patches = ["patches/turn-v1-to-v1.55.66.patch"] }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "patches/turn-v1-to-v1.55.66.patch",
            r#"
                --- a/Cargo.toml
                +++ b/Cargo.toml
                @@ -3,3 +3,3 @@
                             name = "bar"
                -            version = "1.0.0"
                +            version = "1.55.66"
                             authors = []

            "#,
        )
        .file(".cargo/config.toml", PATCHTOOL)
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[PATCHING] bar v1.0.0
[LOCKING] 2 packages to latest compatible versions
[CHECKING] bar v1.55.66 (bar@1.0.0 with 1 patch file)
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    assert_e2e().eq(p.read_lockfile(), str![[r##"
# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "bar"
version = "1.55.66"
source = "patched+registry+https://github.com/rust-lang/crates.io-index?name=bar&version=1.0.0&patch=patches%2Fturn-v1-to-v1.55.66.patch"

[[package]]
name = "foo"
version = "0.0.0"
dependencies = [
 "bar",
]

"##]]);
}

#[cargo_test(requires_patch)]
fn patch_package_version_match_semver_incompat() {
    Package::new("bar", "1.0.0")
        .file(
            "src/lib.rs",
            r#"
                pub fn foo(bar: Bar) {}
                pub struct Bar;
            "#,
        )
        .publish();
    Package::new("bar", "2.0.0")
        .file(
            "src/lib.rs",
            r#"
                pub fn foo(bar: Bar) {}
                pub struct Bar;
            "#,
        )
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["patch-files"]

                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                bar1 = { version = "1", package = "bar" }
                bar2 = { version = "2", package = "bar" }

                [patch.crates-io]
                bar = { version = "1", patches = ["patches/turn-v1-to-v2.patch"] }
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    bar2::foo(bar1::Bar);
                }
            "#,
        )
        .file(
            "patches/turn-v1-to-v2.patch",
            r#"
                --- a/Cargo.toml
                +++ b/Cargo.toml
                @@ -3,3 +3,3 @@
                             name = "bar"
                -            version = "1.0.0"
                +            version = "2.0.0"
                             authors = []

            "#,
        )
        .file(".cargo/config.toml", PATCHTOOL)
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_status(101)
        .with_stderr_data(
            str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[PATCHING] bar v1.0.0
[LOCKING] 3 packages to latest compatible versions
[ADDING] bar v1.0.0 (latest: v2.0.0)
[CHECKING] bar v2.0.0 (bar@1.0.0 with 1 patch file)
[CHECKING] bar v1.0.0
[CHECKING] foo v0.0.0 ([ROOT]/foo)
error[E0308]: mismatched types
...
For more information about this error, try `rustc --explain E0308`.
[ERROR] could not compile `foo` (bin "foo") due to 1 previous error

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test(requires_patch)]
fn patch_package_version_match_nothing() {
    Package::new("bar", "2.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["patch-files"]

                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                bar = "2"

                # After patching this no longer matches any dependency
                # so original bar@2.0.0 is still used.
                [patch.crates-io]
                bar = { patches = ["patches/v2.patch"] }
            "#,
        )
        .file("src/main.rs", "fn main() { }")
        .file(
            "patches/v2.patch",
            r#"
                --- a/Cargo.toml
                +++ b/Cargo.toml
                @@ -3,3 +3,3 @@
                             name = "bar"
                -            version = "2.0.0"
                +            version = "1.55.66"
                             authors = []

                --- a/src/lib.rs
                +++ b/src/lib.rs
                @@ -1,0 +1,1 @@
                +compile_error!("YOU SHALL NOT PASS!");
            "#,
        )
        .file(".cargo/config.toml", PATCHTOOL)
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v2.0.0 (registry `dummy-registry`)
[PATCHING] bar v2.0.0
[WARNING] Patch `bar v1.55.66 (bar@2.0.0 with 1 patch file)` was not used in the crate graph.
Check that the patched package version and available features are compatible
with the dependency requirements. If the patch has a different version from
what is locked in the Cargo.lock file, run `cargo update` to use the new
version. This may also occur with an optional dependency that is not enabled.
[LOCKING] 2 packages to latest compatible versions
[CHECKING] bar v2.0.0
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    assert_e2e().eq(p.read_lockfile(), str![[r##"
# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "bar"
version = "2.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "a184cee92224be6149c9e218327188d1d74a4514f971b1e3ce0170ea94ea5da7"

[[package]]
name = "foo"
version = "0.0.0"
dependencies = [
 "bar",
]

[[patch.unused]]
name = "bar"
version = "1.55.66"
source = "patched+registry+https://github.com/rust-lang/crates.io-index?name=bar&version=2.0.0&patch=patches%2Fv2.patch"

"##]]);
}

#[cargo_test(requires_patch)]
fn not_tracking_unresolved_patches() {
    // Patch that doesn't not match any dependency in graph is not tracked.
    // We need exact veersion from `Summary` to write into lockfile,
    // but unresolved patches are still a dependency requirement from `Dependency`.
    Package::new("bar", "2.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["patch-files"]

                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                bar = "2"

                [patch.crates-io]
                bar = { version = "1", patches = ["patches/hello.patch"] }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("patches/hello.patch", HELLO_PATCH)
        .file(".cargo/config.toml", PATCHTOOL)
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] bar v2.0.0 (registry `dummy-registry`)
[CHECKING] bar v2.0.0
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    assert_e2e().eq(
        p.read_lockfile(),
        str![[r##"
# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "bar"
version = "2.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "a184cee92224be6149c9e218327188d1d74a4514f971b1e3ce0170ea94ea5da7"

[[package]]
name = "foo"
version = "0.0.0"
dependencies = [
 "bar",
]

"##]],
    );
}

#[cargo_test(requires_patch)]
fn patch_for_multiple_major_versions() {
    Package::new("bar", "1.0.0").publish();
    Package::new("bar", "2.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["patch-files"]

                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                bar1 = { version = "1", package = "bar" }
                bar2 = { version = "2", package = "bar" }

                [patch.crates-io]
                bar = { patches = ["patches/hello.patch"] }
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    bar1::hello();
                    bar2::hello();
                }
            "#,
        )
        .file(
            "patches/hello.patch",
            r#"
                --- a/src/lib.rs
                +++ b/src/lib.rs
                @@ -0,0 +1,3 @@
                +pub fn hello() {
                +    println!("Hello, patched for {}!", std::env!("CARGO_PKG_VERSION"))
                +}
            "#,
        )
        .file(".cargo/config.toml", PATCHTOOL)
        .build();

    p.cargo("run")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_stderr_data(
            str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[PATCHING] bar v1.0.0
[DOWNLOADING] crates ...
[DOWNLOADED] bar v2.0.0 (registry `dummy-registry`)
[PATCHING] bar v2.0.0
[LOCKING] 3 packages to latest compatible versions
[COMPILING] bar v2.0.0 (bar@2.0.0 with 1 patch file)
[COMPILING] bar v1.0.0 (bar@1.0.0 with 1 patch file)
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]]
            .unordered(),
        )
        .with_stdout_data(str![[r#"
Hello, patched for 1.0.0!
Hello, patched for 2.0.0!

"#]])
        .run();

    assert_e2e().eq(p.read_lockfile(), str![[r##"
# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "bar"
version = "1.0.0"
source = "patched+registry+https://github.com/rust-lang/crates.io-index?name=bar&version=1.0.0&patch=patches%2Fhello.patch"

[[package]]
name = "bar"
version = "2.0.0"
source = "patched+registry+https://github.com/rust-lang/crates.io-index?name=bar&version=2.0.0&patch=patches%2Fhello.patch"

[[package]]
name = "foo"
version = "0.0.0"
dependencies = [
 "bar 1.0.0",
 "bar 2.0.0",
]

"##]]);
}

#[cargo_test(requires_patch)]
fn multiple_patches() {
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["patch-files"]

                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                bar = "1"

                [patch.crates-io.bar]
                patches = ["patches/hello.patch", "../hola.patch"]
            "#,
        )
        .file("src/main.rs", "fn main() { bar::hello(); bar::hola(); }")
        .file("patches/hello.patch", HELLO_PATCH)
        .file(
            "../hola.patch",
            r#"
                --- a/src/lib.rs
                +++ b/src/lib.rs
                @@ -3,0 +4,3 @@
                +pub fn hola() {
                +    println!("¡Hola, patched!")
                +}
            "#,
        )
        .file(".cargo/config.toml", PATCHTOOL)
        .build();

    p.cargo("run")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[PATCHING] bar v1.0.0
[LOCKING] 2 packages to latest compatible versions
[COMPILING] bar v1.0.0 (bar@1.0.0 with 2 patch files)
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .with_stdout_data(str![[r#"
Hello, patched!
¡Hola, patched!

"#]])
        .run();

    assert_e2e().eq(p.read_lockfile(), str![[r##"
# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "bar"
version = "1.0.0"
source = "patched+registry+https://github.com/rust-lang/crates.io-index?name=bar&version=1.0.0&patch=patches%2Fhello.patch&patch=..%2Fhola.patch"

[[package]]
name = "foo"
version = "0.0.0"
dependencies = [
 "bar",
]

"##]]);
}

#[cargo_test]
fn patch_nonexistent_patch() {
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["patch-files"]

                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                bar = "1"

                [patch.crates-io]
                bar = { patches = ["patches/hello.patch"] }
            "#,
        )
        .file("src/main.rs", "fn main() { bar::hello(); }")
        .build();

    p.cargo("run")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[ERROR] failed to load source for dependency `bar`

Caused by:
  Unable to update bar@1.0.0 with 1 patch file

Caused by:
  failed to open file `patches/hello.patch`

Caused by:
  [..]

"#]])
        .run();
}

#[cargo_test]
fn patch_without_patchtool() {
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["patch-files"]

                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                bar = "1"

                [patch.crates-io]
                bar = { patches = ["patches/hello.patch"] }
            "#,
        )
        .file("src/main.rs", "fn main() { bar::hello(); }")
        .file("patches/hello.patch", HELLO_PATCH)
        .build();

    p.cargo("run")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[PATCHING] bar v1.0.0
[ERROR] failed to load source for dependency `bar`

Caused by:
  Unable to update bar@1.0.0 with 1 patch file

Caused by:
  failed to apply patches

Caused by:
  missing `[patchtool]` for patching dependencies

"#]])
        .run();
}

#[cargo_test(requires_patch)]
fn no_rebuild_if_no_patch_changed() {
    let p = patched_project();

    p.cargo("run")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[PATCHING] bar v1.0.0
[LOCKING] 2 packages to latest compatible versions
[COMPILING] bar v1.0.0 (bar@1.0.0 with 1 patch file)
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .with_stdout_data(str![[r#"
Hello, patched!

"#]])
        .run();

    p.cargo("run -v")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[FRESH] bar v1.0.0 (bar@1.0.0 with 1 patch file)
[FRESH] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .with_stdout_data(str![[r#"
Hello, patched!

"#]])
        .run();
}

#[cargo_test(requires_patch)]
fn rebuild_if_patch_changed() {
    let p = patched_project();

    p.cargo("run")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[PATCHING] bar v1.0.0
[LOCKING] 2 packages to latest compatible versions
[COMPILING] bar v1.0.0 (bar@1.0.0 with 1 patch file)
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .with_stdout_data(str![[r#"
Hello, patched!

"#]])
        .run();

    p.change_file(
        "patches/hello.patch",
        r#"
            --- a/src/lib.rs
            +++ b/src/lib.rs
            @@ -0,0 +1,3 @@
            +pub fn hello() {
            +    println!("¡Hola, patched!")
            +}
        "#,
    );

    p.cargo("run")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[PATCHING] bar v1.0.0
[COMPILING] bar v1.0.0 (bar@1.0.0 with 1 patch file)
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .with_stdout_data(str![[r#"
¡Hola, patched!

"#]])
        .run();
}

#[cargo_test(requires_patch)]
fn cargo_metadata() {
    let p = patched_project();

    p.cargo("generate-lockfile")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[PATCHING] bar v1.0.0
[LOCKING] 2 packages to latest compatible versions

"#]])
        .run();

    p.cargo("metadata")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_stdout_data(str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "patched+registry+https://github.com/rust-lang/crates.io-index?name=bar&version=1.0.0&patch=patches%2Fhello.patch#bar@1.0.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/home/.cargo/patched-src/github.com-1ecc6299db9ec823/bar-1.0.0/46806b943777e31e/Cargo.toml",
      "metadata": null,
      "name": "bar",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": "patched+registry+https://github.com/rust-lang/crates.io-index?name=bar&version=1.0.0&patch=patches%2Fhello.patch",
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "bar",
          "src_path": "[ROOT]/home/.cargo/patched-src/github.com-1ecc6299db9ec823/bar-1.0.0/46806b943777e31e/src/lib.rs",
          "test": true
        }
      ],
      "version": "1.0.0"
    },
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [
        {
          "features": [],
          "kind": null,
          "name": "bar",
          "optional": false,
          "registry": null,
          "rename": null,
          "req": "^1",
          "source": "registry+https://github.com/rust-lang/crates.io-index",
          "target": null,
          "uses_default_features": true
        }
      ],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo#0.0.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": null,
      "name": "foo",
      "publish": [],
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "bin"
          ],
          "doc": true,
          "doctest": false,
          "edition": "2015",
          "kind": [
            "bin"
          ],
          "name": "foo",
          "src_path": "[ROOT]/foo/src/main.rs",
          "test": true
        }
      ],
      "version": "0.0.0"
    }
  ],
  "resolve": {
    "nodes": [
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "patched+registry+https://github.com/rust-lang/crates.io-index?name=bar&version=1.0.0&patch=patches%2Fhello.patch#bar@1.0.0"
      },
      {
        "dependencies": [
          "patched+registry+https://github.com/rust-lang/crates.io-index?name=bar&version=1.0.0&patch=patches%2Fhello.patch#bar@1.0.0"
        ],
        "deps": [
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": null
              }
            ],
            "name": "bar",
            "pkg": "patched+registry+https://github.com/rust-lang/crates.io-index?name=bar&version=1.0.0&patch=patches%2Fhello.patch#bar@1.0.0"
          }
        ],
        "features": [],
        "id": "path+[ROOTURL]/foo#0.0.0"
      }
    ],
    "root": "path+[ROOTURL]/foo#0.0.0"
  },
  "target_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.0.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.0.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]].json())
        .run();
}

#[cargo_test(requires_patch)]
fn cargo_pkgid() {
    let p = patched_project();

    p.cargo("generate-lockfile")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[PATCHING] bar v1.0.0
[LOCKING] 2 packages to latest compatible versions

"#]])
        .run();

    p.cargo("pkgid bar")
        .masquerade_as_nightly_cargo(&["patch-files"])
        .with_stdout_data(str![[r#"
patched+registry+https://github.com/rust-lang/crates.io-index?name=bar&version=1.0.0&patch=patches%2Fhello.patch#bar@1.0.0

"#]])
        .run();
}
