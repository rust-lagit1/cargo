use std::cell::RefCell;
use std::collections::hash_map::{Entry, HashMap};
use std::env;
use std::path::PathBuf;
use std::str::{self, FromStr};

use crate::core::compiler::Kind;
use crate::core::TargetKind;
use crate::util::CfgExpr;
use crate::util::{CargoResult, CargoResultExt, Cfg, Config, ProcessBuilder, Rustc};

#[derive(Clone)]
pub struct TargetInfo {
    crate_type_process: Option<ProcessBuilder>,
    crate_types: RefCell<HashMap<String, Option<(String, String)>>>,
    cfg: Option<Vec<Cfg>>,
    pub sysroot_libdir: Option<PathBuf>,
    pub rustflags: Vec<String>,
    pub rustdocflags: Vec<String>,
}

/// Type of each file generated by a Unit.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FileFlavor {
    /// Not a special file type.
    Normal,
    /// Something you can link against (e.g., a library).
    Linkable { rmeta: bool },
    /// Piece of external debug information (e.g., `.dSYM`/`.pdb` file).
    DebugInfo,
}

pub struct FileType {
    pub flavor: FileFlavor,
    suffix: String,
    prefix: String,
    // Wasm bin target will generate two files in deps such as
    // "web-stuff.js" and "web_stuff.wasm". Note the different usages of
    // "-" and "_". should_replace_hyphens is a flag to indicate that
    // we need to convert the stem "web-stuff" to "web_stuff", so we
    // won't miss "web_stuff.wasm".
    should_replace_hyphens: bool,
}

impl FileType {
    pub fn filename(&self, stem: &str) -> String {
        let stem = if self.should_replace_hyphens {
            stem.replace("-", "_")
        } else {
            stem.to_string()
        };
        format!("{}{}{}", self.prefix, stem, self.suffix)
    }
}

impl TargetInfo {
    pub fn new(
        config: &Config,
        requested_target: &Option<String>,
        rustc: &Rustc,
        kind: Kind,
    ) -> CargoResult<TargetInfo> {
        let rustflags = env_args(
            config,
            requested_target,
            &rustc.host,
            None,
            kind,
            "RUSTFLAGS",
        )?;
        let mut process = rustc.process();
        process
            .arg("-")
            .arg("--crate-name")
            .arg("___")
            .arg("--print=file-names")
            .args(&rustflags)
            .env_remove("RUSTC_LOG");

        let target_triple = requested_target
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or(&rustc.host);
        if kind == Kind::Target {
            process.arg("--target").arg(target_triple);
        }

        let crate_type_process = process.clone();
        const KNOWN_CRATE_TYPES: &[&str] =
            &["bin", "rlib", "dylib", "cdylib", "staticlib", "proc-macro"];
        for crate_type in KNOWN_CRATE_TYPES.iter() {
            process.arg("--crate-type").arg(crate_type);
        }

        let mut with_cfg = process.clone();
        with_cfg.arg("--print=sysroot");
        with_cfg.arg("--print=cfg");

        let mut has_cfg_and_sysroot = true;
        let (output, error) = rustc
            .cached_output(&with_cfg)
            .or_else(|_| {
                has_cfg_and_sysroot = false;
                rustc.cached_output(&process)
            })
            .chain_err(|| "failed to run `rustc` to learn about target-specific information")?;

        let mut lines = output.lines();
        let mut map = HashMap::new();
        for crate_type in KNOWN_CRATE_TYPES {
            let out = parse_crate_type(crate_type, &error, &mut lines)?;
            map.insert(crate_type.to_string(), out);
        }

        let mut sysroot_libdir = None;
        if has_cfg_and_sysroot {
            let line = match lines.next() {
                Some(line) => line,
                None => failure::bail!(
                    "output of --print=sysroot missing when learning about \
                     target-specific information from rustc"
                ),
            };
            let mut rustlib = PathBuf::from(line);
            if kind == Kind::Host {
                if cfg!(windows) {
                    rustlib.push("bin");
                } else {
                    rustlib.push("lib");
                }
                sysroot_libdir = Some(rustlib);
            } else {
                rustlib.push("lib");
                rustlib.push("rustlib");
                rustlib.push(target_triple);
                rustlib.push("lib");
                sysroot_libdir = Some(rustlib);
            }
        }

        let cfg = if has_cfg_and_sysroot {
            Some(lines.map(Cfg::from_str).collect::<CargoResult<Vec<_>>>()?)
        } else {
            None
        };

        Ok(TargetInfo {
            crate_type_process: Some(crate_type_process),
            crate_types: RefCell::new(map),
            sysroot_libdir,
            // recalculate `rustflags` from above now that we have `cfg`
            // information
            rustflags: env_args(
                config,
                requested_target,
                &rustc.host,
                cfg.as_ref().map(|v| v.as_ref()),
                kind,
                "RUSTFLAGS",
            )?,
            rustdocflags: env_args(
                config,
                requested_target,
                &rustc.host,
                cfg.as_ref().map(|v| v.as_ref()),
                kind,
                "RUSTDOCFLAGS",
            )?,
            cfg,
        })
    }

    pub fn cfg(&self) -> Option<&[Cfg]> {
        self.cfg.as_ref().map(|v| v.as_ref())
    }

    pub fn file_types(
        &self,
        crate_type: &str,
        flavor: FileFlavor,
        kind: &TargetKind,
        target_triple: &str,
    ) -> CargoResult<Option<Vec<FileType>>> {
        let mut crate_types = self.crate_types.borrow_mut();
        let entry = crate_types.entry(crate_type.to_string());
        let crate_type_info = match entry {
            Entry::Occupied(o) => &*o.into_mut(),
            Entry::Vacant(v) => {
                let value = self.discover_crate_type(v.key())?;
                &*v.insert(value)
            }
        };
        let (prefix, suffix) = match *crate_type_info {
            Some((ref prefix, ref suffix)) => (prefix, suffix),
            None => return Ok(None),
        };
        let mut ret = vec![FileType {
            suffix: suffix.clone(),
            prefix: prefix.clone(),
            flavor,
            should_replace_hyphens: false,
        }];

        // See rust-lang/cargo#4500.
        if target_triple.ends_with("pc-windows-msvc")
            && crate_type.ends_with("dylib")
            && suffix == ".dll"
        {
            ret.push(FileType {
                suffix: ".dll.lib".to_string(),
                prefix: prefix.clone(),
                flavor: FileFlavor::Normal,
                should_replace_hyphens: false,
            })
        }

        // See rust-lang/cargo#4535.
        if target_triple.starts_with("wasm32-") && crate_type == "bin" && suffix == ".js" {
            ret.push(FileType {
                suffix: ".wasm".to_string(),
                prefix: prefix.clone(),
                flavor: FileFlavor::Normal,
                should_replace_hyphens: true,
            })
        }

        // See rust-lang/cargo#4490, rust-lang/cargo#4960.
        // Only uplift debuginfo for binaries.
        // - Tests are run directly from `target/debug/deps/` with the
        //   metadata hash still in the filename.
        // - Examples are only uplifted for apple because the symbol file
        //   needs to match the executable file name to be found (i.e., it
        //   needs to remove the hash in the filename). On Windows, the path
        //   to the .pdb with the hash is embedded in the executable.
        let is_apple = target_triple.contains("-apple-");
        if *kind == TargetKind::Bin || (*kind == TargetKind::ExampleBin && is_apple) {
            if is_apple {
                ret.push(FileType {
                    suffix: ".dSYM".to_string(),
                    prefix: prefix.clone(),
                    flavor: FileFlavor::DebugInfo,
                    should_replace_hyphens: false,
                })
            } else if target_triple.ends_with("-msvc") {
                ret.push(FileType {
                    suffix: ".pdb".to_string(),
                    prefix: prefix.clone(),
                    flavor: FileFlavor::DebugInfo,
                    should_replace_hyphens: false,
                })
            }
        }

        Ok(Some(ret))
    }

    fn discover_crate_type(&self, crate_type: &str) -> CargoResult<Option<(String, String)>> {
        let mut process = self.crate_type_process.clone().unwrap();

        process.arg("--crate-type").arg(crate_type);

        let output = process.exec_with_output().chain_err(|| {
            format!(
                "failed to run `rustc` to learn about \
                 crate-type {} information",
                crate_type
            )
        })?;

        let error = str::from_utf8(&output.stderr).unwrap();
        let output = str::from_utf8(&output.stdout).unwrap();
        Ok(parse_crate_type(crate_type, error, &mut output.lines())?)
    }
}

/// Takes rustc output (using specialized command line args), and calculates the file prefix and
/// suffix for the given crate type, or returns `None` if the type is not supported. (e.g., for a
/// Rust library like `libcargo.rlib`, we have prefix "lib" and suffix "rlib").
///
/// The caller needs to ensure that the lines object is at the correct line for the given crate
/// type: this is not checked.
//
// This function can not handle more than one file per type (with wasm32-unknown-emscripten, there
// are two files for bin (`.wasm` and `.js`)).
fn parse_crate_type(
    crate_type: &str,
    error: &str,
    lines: &mut str::Lines<'_>,
) -> CargoResult<Option<(String, String)>> {
    let not_supported = error.lines().any(|line| {
        (line.contains("unsupported crate type") || line.contains("unknown crate type"))
            && line.contains(crate_type)
    });
    if not_supported {
        return Ok(None);
    }
    let line = match lines.next() {
        Some(line) => line,
        None => failure::bail!(
            "malformed output when learning about \
             crate-type {} information",
            crate_type
        ),
    };
    let mut parts = line.trim().split("___");
    let prefix = parts.next().unwrap();
    let suffix = match parts.next() {
        Some(part) => part,
        None => failure::bail!(
            "output of --print=file-names has changed in \
             the compiler, cannot parse"
        ),
    };

    Ok(Some((prefix.to_string(), suffix.to_string())))
}

/// Acquire extra flags to pass to the compiler from various locations.
///
/// The locations are:
///
///  - the `RUSTFLAGS` environment variable
///
/// then if this was not found
///
///  - `target.*.rustflags` from the manifest (Cargo.toml)
///  - `target.cfg(..).rustflags` from the manifest
///
/// then if neither of these were found
///
///  - `build.rustflags` from the manifest
///
/// Note that if a `target` is specified, no args will be passed to host code (plugins, build
/// scripts, ...), even if it is the same as the target.
fn env_args(
    config: &Config,
    requested_target: &Option<String>,
    host_triple: &str,
    target_cfg: Option<&[Cfg]>,
    kind: Kind,
    name: &str,
) -> CargoResult<Vec<String>> {
    // We *want* to apply RUSTFLAGS only to builds for the
    // requested target architecture, and not to things like build
    // scripts and plugins, which may be for an entirely different
    // architecture. Cargo's present architecture makes it quite
    // hard to only apply flags to things that are not build
    // scripts and plugins though, so we do something more hacky
    // instead to avoid applying the same RUSTFLAGS to multiple targets
    // arches:
    //
    // 1) If --target is not specified we just apply RUSTFLAGS to
    // all builds; they are all going to have the same target.
    //
    // 2) If --target *is* specified then we only apply RUSTFLAGS
    // to compilation units with the Target kind, which indicates
    // it was chosen by the --target flag.
    //
    // This means that, e.g., even if the specified --target is the
    // same as the host, build scripts in plugins won't get
    // RUSTFLAGS.
    let compiling_with_target = requested_target.is_some();
    let is_target_kind = kind == Kind::Target;

    if compiling_with_target && !is_target_kind {
        // This is probably a build script or plugin and we're
        // compiling with --target. In this scenario there are
        // no rustflags we can apply.
        return Ok(Vec::new());
    }

    // First try RUSTFLAGS from the environment
    if let Ok(a) = env::var(name) {
        let args = a
            .split(' ')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        return Ok(args.collect());
    }

    let mut rustflags = Vec::new();

    let name = name
        .chars()
        .flat_map(|c| c.to_lowercase())
        .collect::<String>();
    // Then the target.*.rustflags value...
    let target = requested_target
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or(host_triple);
    let key = format!("target.{}.{}", target, name);
    if let Some(args) = config.get_list_or_split_string(&key)? {
        let args = args.val.into_iter();
        rustflags.extend(args);
    }
    // ...including target.'cfg(...)'.rustflags
    if let Some(target_cfg) = target_cfg {
        if let Some(table) = config.get_table("target")? {
            let cfgs = table
                .val
                .keys()
                .filter(|key| CfgExpr::matches_key(key, target_cfg));

            // Note that we may have multiple matching `[target]` sections and
            // because we're passing flags to the compiler this can affect
            // cargo's caching and whether it rebuilds. Ensure a deterministic
            // ordering through sorting for now. We may perhaps one day wish to
            // ensure a deterministic ordering via the order keys were defined
            // in files perhaps.
            let mut cfgs = cfgs.collect::<Vec<_>>();
            cfgs.sort();

            for n in cfgs {
                let key = format!("target.{}.{}", n, name);
                if let Some(args) = config.get_list_or_split_string(&key)? {
                    let args = args.val.into_iter();
                    rustflags.extend(args);
                }
            }
        }
    }

    if !rustflags.is_empty() {
        return Ok(rustflags);
    }

    // Then the `build.rustflags` value.
    let key = format!("build.{}", name);
    if let Some(args) = config.get_list_or_split_string(&key)? {
        let args = args.val.into_iter();
        return Ok(args.collect());
    }

    Ok(Vec::new())
}
