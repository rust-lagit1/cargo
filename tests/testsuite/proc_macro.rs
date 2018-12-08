use crate::support::is_nightly;
use crate::support::project;

#[test]
fn probe_cfg_before_crate_type_discovery() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [target.'cfg(not(stage300))'.dependencies.noop]
            path = "../noop"
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            #[macro_use]
            extern crate noop;

            #[derive(Noop)]
            struct X;

            fn main() {}
        "#,
        )
        .build();
    let _noop = project()
        .at("noop")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "noop"
            version = "0.0.1"
            authors = []

            [lib]
            proc-macro = true
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            extern crate proc_macro;
            use proc_macro::TokenStream;

            #[proc_macro_derive(Noop)]
            pub fn noop(_input: TokenStream) -> TokenStream {
                "".parse().unwrap()
            }
        "#,
        )
        .build();

    p.cargo("build").run();
}

#[test]
fn noop() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.noop]
            path = "../noop"
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            #[macro_use]
            extern crate noop;

            #[derive(Noop)]
            struct X;

            fn main() {}
        "#,
        )
        .build();
    let _noop = project()
        .at("noop")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "noop"
            version = "0.0.1"
            authors = []

            [lib]
            proc-macro = true
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            extern crate proc_macro;
            use proc_macro::TokenStream;

            #[proc_macro_derive(Noop)]
            pub fn noop(_input: TokenStream) -> TokenStream {
                "".parse().unwrap()
            }
        "#,
        )
        .build();

    p.cargo("build").run();
    p.cargo("build").run();
}

#[test]
fn impl_and_derive() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.transmogrify]
            path = "../transmogrify"
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            #[macro_use]
            extern crate transmogrify;

            trait ImplByTransmogrify {
                fn impl_by_transmogrify(&self) -> bool;
            }

            #[derive(Transmogrify, Debug)]
            struct X { success: bool }

            fn main() {
                let x = X::new();
                assert!(x.impl_by_transmogrify());
                println!("{:?}", x);
            }
        "#,
        )
        .build();
    let _transmogrify = project()
        .at("transmogrify")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "transmogrify"
            version = "0.0.1"
            authors = []

            [lib]
            proc-macro = true
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            extern crate proc_macro;
            use proc_macro::TokenStream;

            #[proc_macro_derive(Transmogrify)]
            #[doc(hidden)]
            pub fn transmogrify(input: TokenStream) -> TokenStream {
                "
                    impl X {
                        fn new() -> Self {
                            X { success: true }
                        }
                    }

                    impl ImplByTransmogrify for X {
                        fn impl_by_transmogrify(&self) -> bool {
                            true
                        }
                    }
                ".parse().unwrap()
            }
        "#,
        )
        .build();

    p.cargo("build").run();
    p.cargo("run").with_stdout("X { success: true }").run();
}

#[test]
fn plugin_and_proc_macro() {
    if !is_nightly() {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            plugin = true
            proc-macro = true
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            #![feature(plugin_registrar, rustc_private)]
            #![feature(proc_macro, proc_macro_lib)]

            extern crate rustc_plugin;
            use rustc_plugin::Registry;

            extern crate proc_macro;
            use proc_macro::TokenStream;

            #[plugin_registrar]
            pub fn plugin_registrar(reg: &mut Registry) {}

            #[proc_macro_derive(Questionable)]
            pub fn questionable(input: TokenStream) -> TokenStream {
                input
            }
        "#,
        )
        .build();

    let msg = "  lib.plugin and lib.proc-macro cannot both be true";
    p.cargo("build")
        .with_status(101)
        .with_stderr_contains(msg)
        .run();
}

#[test]
fn proc_macro_doctest() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            [lib]
            proc-macro = true
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
#![crate_type = "proc-macro"]

extern crate proc_macro;

use proc_macro::TokenStream;

/// ```
/// assert!(true);
/// ```
#[proc_macro_derive(Bar)]
pub fn derive(_input: TokenStream) -> TokenStream {
    "".parse().unwrap()
}

#[test]
fn a() {
  assert!(true);
}
"#,
        )
        .build();

    foo.cargo("test")
        .with_stdout_contains("test a ... ok")
        .with_stdout_contains_n("test [..] ... ok", 2)
        .run();
}

#[test]
fn proc_macro_crate_type() {
    // Verify that `crate-type = ["proc-macro"]` is the same as `proc-macro = true`
    // and that everything, including rustdoc, works correctly.
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            [dependencies]
            pm = { path = "pm" }
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            //! ```
            //! use foo::THING;
            //! assert_eq!(THING, 123);
            //! ```
            #[macro_use]
            extern crate pm;
            #[derive(MkItem)]
            pub struct S;
            #[cfg(test)]
            mod tests {
                use super::THING;
                #[test]
                fn it_works() {
                    assert_eq!(THING, 123);
                }
            }
        "#,
        )
        .file(
            "pm/Cargo.toml",
            r#"
            [package]
            name = "pm"
            version = "0.1.0"
            [lib]
            crate-type = ["proc-macro"]
        "#,
        )
        .file(
            "pm/src/lib.rs",
            r#"
            extern crate proc_macro;
            use proc_macro::TokenStream;

            #[proc_macro_derive(MkItem)]
            pub fn mk_item(_input: TokenStream) -> TokenStream {
                "pub const THING: i32 = 123;".parse().unwrap()
            }
        "#,
        )
        .build();

    foo.cargo("test")
        .with_stdout_contains("test tests::it_works ... ok")
        .with_stdout_contains_n("test [..] ... ok", 2)
        .run();
}

#[test]
fn proc_macro_crate_type_warning() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            [lib]
            crate-type = ["proc-macro"]
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("build")
        .with_stderr_contains(
            "[WARNING] library `foo` should only specify `proc-macro = true` instead of setting `crate-type`")
        .run();
}

#[test]
fn proc_macro_crate_type_warning_plugin() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            [lib]
            crate-type = ["proc-macro"]
            plugin = true
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("build")
        .with_stderr_contains(
            "[WARNING] proc-macro library `foo` should not specify `plugin = true`")
        .with_stderr_contains(
            "[WARNING] library `foo` should only specify `proc-macro = true` instead of setting `crate-type`")
        .run();
}

#[test]
fn proc_macro_crate_type_multiple() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            [lib]
            crate-type = ["proc-macro", "rlib"]
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("build")
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]/foo/Cargo.toml`

Caused by:
  cannot mix `proc-macro` crate type with others
",
        )
        .with_status(101)
        .run();
}
