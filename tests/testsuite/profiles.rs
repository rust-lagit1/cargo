use std::env;

use cargotest::is_nightly;
use cargotest::support::{basic_lib_manifest, execs, project};
use cargotest::ChannelChanger;
use hamcrest::assert_that;

#[test]
fn profile_overrides() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]

            name = "test"
            version = "0.0.0"
            authors = []

            [profile.dev]
            opt-level = 1
            debug = false
            rpath = true
        "#,
        )
        .file("src/lib.rs", "")
        .build();
    assert_that(
        p.cargo("build").arg("-v"),
        execs().with_status(0).with_stderr(&format!(
            "\
[COMPILING] test v0.0.0 ({url})
[RUNNING] `rustc --crate-name test src[/]lib.rs --crate-type lib \
        --emit=dep-info,link \
        -C opt-level=1 \
        -C debug-assertions=on \
        -C metadata=[..] \
        -C rpath \
        --out-dir [..] \
        -L dependency={dir}[/]target[/]debug[/]deps`
[FINISHED] dev [optimized] target(s) in [..]
",
            dir = p.root().display(),
            url = p.url(),
        )),
    );
}

#[test]
fn opt_level_override_0() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]

            name = "test"
            version = "0.0.0"
            authors = []

            [profile.dev]
            opt-level = 0
        "#,
        )
        .file("src/lib.rs", "")
        .build();
    assert_that(
        p.cargo("build").arg("-v"),
        execs().with_status(0).with_stderr(&format!(
            "\
[COMPILING] test v0.0.0 ({url})
[RUNNING] `rustc --crate-name test src[/]lib.rs --crate-type lib \
        --emit=dep-info,link \
        -C debuginfo=2 \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency={dir}[/]target[/]debug[/]deps`
[FINISHED] [..] target(s) in [..]
",
            dir = p.root().display(),
            url = p.url()
        )),
    );
}

#[test]
fn debug_override_1() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "test"
            version = "0.0.0"
            authors = []

            [profile.dev]
            debug = 1
        "#,
        )
        .file("src/lib.rs", "")
        .build();
    assert_that(
        p.cargo("build").arg("-v"),
        execs().with_status(0).with_stderr(&format!(
            "\
[COMPILING] test v0.0.0 ({url})
[RUNNING] `rustc --crate-name test src[/]lib.rs --crate-type lib \
        --emit=dep-info,link \
        -C debuginfo=1 \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency={dir}[/]target[/]debug[/]deps`
[FINISHED] [..] target(s) in [..]
",
            dir = p.root().display(),
            url = p.url()
        )),
    );
}

fn check_opt_level_override(profile_level: &str, rustc_level: &str) {
    let p = project("foo")
        .file(
            "Cargo.toml",
            &format!(
                r#"
            [package]

            name = "test"
            version = "0.0.0"
            authors = []

            [profile.dev]
            opt-level = {level}
        "#,
                level = profile_level
            ),
        )
        .file("src/lib.rs", "")
        .build();
    assert_that(
        p.cargo("build").arg("-v"),
        execs().with_status(0).with_stderr(&format!(
            "\
[COMPILING] test v0.0.0 ({url})
[RUNNING] `rustc --crate-name test src[/]lib.rs --crate-type lib \
        --emit=dep-info,link \
        -C opt-level={level} \
        -C debuginfo=2 \
        -C debug-assertions=on \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency={dir}[/]target[/]debug[/]deps`
[FINISHED] [..] target(s) in [..]
",
            dir = p.root().display(),
            url = p.url(),
            level = rustc_level
        )),
    );
}

#[test]
fn opt_level_overrides() {
    if !is_nightly() {
        return;
    }

    for &(profile_level, rustc_level) in &[
        ("1", "1"),
        ("2", "2"),
        ("3", "3"),
        ("\"s\"", "s"),
        ("\"z\"", "z"),
    ] {
        check_opt_level_override(profile_level, rustc_level)
    }
}

#[test]
fn top_level_overrides_deps() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]

            name = "test"
            version = "0.0.0"
            authors = []

            [profile.release]
            opt-level = 1
            debug = true

            [dependencies.foo]
            path = "foo"
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            "foo/Cargo.toml",
            r#"
            [package]

            name = "foo"
            version = "0.0.0"
            authors = []

            [profile.release]
            opt-level = 0
            debug = false

            [lib]
            name = "foo"
            crate_type = ["dylib", "rlib"]
        "#,
        )
        .file("foo/src/lib.rs", "")
        .build();
    assert_that(
        p.cargo("build").arg("-v").arg("--release"),
        execs().with_status(0).with_stderr(&format!(
            "\
[COMPILING] foo v0.0.0 ({url}/foo)
[RUNNING] `rustc --crate-name foo foo[/]src[/]lib.rs \
        --crate-type dylib --crate-type rlib \
        --emit=dep-info,link \
        -C prefer-dynamic \
        -C opt-level=1 \
        -C debuginfo=2 \
        -C metadata=[..] \
        --out-dir {dir}[/]target[/]release[/]deps \
        -L dependency={dir}[/]target[/]release[/]deps`
[COMPILING] test v0.0.0 ({url})
[RUNNING] `rustc --crate-name test src[/]lib.rs --crate-type lib \
        --emit=dep-info,link \
        -C opt-level=1 \
        -C debuginfo=2 \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency={dir}[/]target[/]release[/]deps \
        --extern foo={dir}[/]target[/]release[/]deps[/]\
                     {prefix}foo[..]{suffix} \
        --extern foo={dir}[/]target[/]release[/]deps[/]libfoo.rlib`
[FINISHED] release [optimized + debuginfo] target(s) in [..]
",
            dir = p.root().display(),
            url = p.url(),
            prefix = env::consts::DLL_PREFIX,
            suffix = env::consts::DLL_SUFFIX
        )),
    );
}

#[test]
fn profile_in_non_root_manifest_triggers_a_warning() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.1.0"
            authors = []

            [workspace]
            members = ["bar"]

            [profile.dev]
            debug = false
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
            [project]
            name = "bar"
            version = "0.1.0"
            authors = []
            workspace = ".."

            [profile.dev]
            opt-level = 1
        "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build").cwd(p.root().join("bar")).arg("-v"),
        execs().with_status(0).with_stderr(
            "\
[WARNING] profiles for the non root package will be ignored, specify profiles at the workspace root:
package:   [..]
workspace: [..]
[COMPILING] bar v0.1.0 ([..])
[RUNNING] `rustc [..]`
[FINISHED] dev [unoptimized] target(s) in [..]",
        ),
    );
}

#[test]
fn profile_in_virtual_manifest_works() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar"]

            [profile.dev]
            opt-level = 1
            debug = false
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
            [project]
            name = "bar"
            version = "0.1.0"
            authors = []
            workspace = ".."
        "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build").cwd(p.root().join("bar")).arg("-v"),
        execs().with_status(0).with_stderr(
            "\
[COMPILING] bar v0.1.0 ([..])
[RUNNING] `rustc [..]`
[FINISHED] dev [optimized] target(s) in [..]",
        ),
    );
}

#[test]
fn profile_override_gated() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [profile.dev.build-override]
            opt-level = 3
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build").masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  feature `profile-overrides` is required

consider adding `cargo-features = [\"profile-overrides\"]` to the manifest
",
        ),
    );

    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [profile.dev.overrides."*"]
            opt-level = 3
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build").masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  feature `profile-overrides` is required

consider adding `cargo-features = [\"profile-overrides\"]` to the manifest
",
        ),
    );
}

#[test]
fn profile_override_basic() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["profile-overrides"]

            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = {path = "bar"}

            [profile.dev.overrides.bar]
            opt-level = 3
        "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build -v").masquerade_as_nightly_cargo(),
        execs().with_status(0).with_stderr(
"[COMPILING] bar [..]
[RUNNING] `rustc --crate-name bar [..] -C opt-level=3 [..]`
[COMPILING] foo [..]
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
        )
        // TODO: does_not_contain does not support patterns!
        // .with_stderr_does_not_contain("\
        //     `rustc --crate-name bar[..]-C opt-level=3"),
    );
}

#[test]
fn profile_override_bad_name() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["profile-overrides"]

            [package]
            name = "foo"
            version = "0.0.1"

            [dependencies]
            bar = {path = "bar"}

            [profile.dev.overrides.bart]
            opt-level = 3

            [profile.dev.overrides.no-suggestion]
            opt-level = 3
        "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build").masquerade_as_nightly_cargo(),
        execs().with_status(0).with_stderr_contains("\
[WARNING] package `bart` for profile override not found

Did you mean `bar`?
[WARNING] package `no-suggestion` for profile override not found
[COMPILING] [..]
"));
}

#[test]
fn profile_panic_test_bench() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"

            [profile.test]
            panic = "abort"

            [profile.bench]
            panic = "abort"
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(0).with_stderr_contains("\
[WARNING] `panic` setting is ignored for `test` profile
[WARNING] `panic` setting is ignored for `bench` profile
"));
}
