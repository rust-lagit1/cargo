//! Management of the directory layout of a build
//!
//! The directory layout is a little tricky at times, hence a separate file to
//! house this logic. The current layout looks like this:
//!
//! ```text
//! # This is the root directory for all output, the top-level package
//! # places all of its output here.
//! target/
//!
//!     # Cache of `rustc -Vv` output for performance.
//!     .rustc-info.json
//!
//!     # All final artifacts are linked into this directory from `deps`.
//!     # Note that named profiles will soon be included as separate directories
//!     # here. They have a restricted format, similar to Rust identifiers, so
//!     # Cargo-specific directories added in the future should use some prefix
//!     # like `.` to avoid name collisions.
//!     debug/  # or release/
//!
//!         # File used to lock the directory to prevent multiple cargo processes
//!         # from using it at the same time.
//!         .cargo-lock
//!
//!         # Hidden directory that holds all of the fingerprint files for all
//!         # packages
//!         .fingerprint/
//!             # Each package is in a separate directory.
//!             # Note that different target kinds have different filename prefixes.
//!             $pkgname-$META/
//!                 # Set of source filenames for this package.
//!                 dep-lib-$targetname
//!                 # Timestamp when this package was last built.
//!                 invoked.timestamp
//!                 # The fingerprint hash.
//!                 lib-$targetname
//!                 # Detailed information used for logging the reason why
//!                 # something is being recompiled.
//!                 lib-$targetname.json
//!                 # The console output from the compiler. This is cached
//!                 # so that warnings can be redisplayed for "fresh" units.
//!                 output-lib-$targetname
//!
//!         # This is the root directory for all rustc artifacts except build
//!         # scripts, examples, and test and bench executables. Almost every
//!         # artifact should have a metadata hash added to its filename to
//!         # prevent collisions. One notable exception is dynamic libraries.
//!         deps/
//!
//!         # Root directory for all compiled examples.
//!         examples/
//!
//!         # Directory used to store incremental data for the compiler (when
//!         # incremental is enabled.
//!         incremental/
//!
//!         # This is the location at which the output of all custom build
//!         # commands are rooted.
//!         build/
//!
//!             # Each package gets its own directory where its build script and
//!             # script output are placed
//!             $pkgname-$META/    # For the build script itself.
//!                 # The build script executable (name may be changed by user).
//!                 build-script-build-$META
//!                 # Hard link to build-script-build-$META.
//!                 build-script-build
//!                 # Dependency information generated by rustc.
//!                 build-script-build-$META.d
//!                 # Debug information, depending on platform and profile
//!                 # settings.
//!                 <debug symbols>
//!
//!             # The package shows up twice with two different metadata hashes.
//!             $pkgname-$META/  # For the output of the build script.
//!                 # Timestamp when the build script was last executed.
//!                 invoked.timestamp
//!                 # Directory where script can output files ($OUT_DIR).
//!                 out/
//!                 # Output from the build script.
//!                 output
//!                 # Path to `out`, used to help when the target directory is
//!                 # moved.
//!                 root-output
//!                 # Stderr output from the build script.
//!                 stderr
//!
//!     # Output from rustdoc
//!     doc/
//!
//!     # Used by `cargo package` and `cargo publish` to build a `.crate` file.
//!     package/
//!
//!     # Experimental feature for generated build scripts.
//!     .metabuild/
//! ```
//!
//! When cross-compiling, the layout is the same, except it appears in
//! `target/$TRIPLE`.

use crate::core::compiler::CompileTarget;
use crate::core::Workspace;
use crate::util::{CargoResult, FileLock};
use cargo_util::paths;
use std::path::{Path, PathBuf};

/// Contains the paths of all target output locations.
///
/// See module docs for more information.
pub struct Layout {
    /// The root directory: `/path/to/target`.
    /// If cross compiling: `/path/to/target/$TRIPLE`.
    root: PathBuf,
    /// The final artifact destination: `$root/debug` (or `release`).
    dest: PathBuf,
    /// The directory with rustc artifacts: `$dest/deps`
    deps: PathBuf,
    /// The directory for build scripts: `$dest/build`
    build: PathBuf,
    /// The directory for incremental files: `$dest/incremental`
    incremental: PathBuf,
    /// The directory for fingerprints: `$dest/.fingerprint`
    fingerprint: PathBuf,
    /// The directory for examples: `$dest/examples`
    examples: PathBuf,
    /// The directory for rustdoc output: `$root/doc`
    doc: PathBuf,
    /// The directory for rustdoc output: `$root/doc/src`
    src: PathBuf,
    /// The directory for temporary data of integration tests and benches: `$dest/tmp`
    tmp: PathBuf,
    /// The lockfile for a build (`.cargo-lock`). Will be unlocked when this
    /// struct is `drop`ped.
    _lock: FileLock,
}

impl Layout {
    /// Calculate the paths for build output, lock the build directory, and return as a Layout.
    ///
    /// This function will block if the directory is already locked.
    ///
    /// `dest` should be the final artifact directory name. Currently either
    /// "debug" or "release".
    pub fn new(
        ws: &Workspace<'_>,
        target: Option<CompileTarget>,
        dest: &str,
    ) -> CargoResult<Layout> {
        let mut root = ws.target_dir();
        if let Some(target) = target {
            root.push(target.short_name());
        }
        let dest = root.join(dest);
        // If the root directory doesn't already exist go ahead and create it
        // here. Use this opportunity to exclude it from backups as well if the
        // system supports it since this is a freshly created folder.
        //
        paths::create_dir_all_excluded_from_backups_atomic(root.as_path_unlocked())?;
        // Now that the excluded from backups target root is created we can create the
        // actual destination (sub)subdirectory.
        paths::create_dir_all(dest.as_path_unlocked())?;

        // For now we don't do any more finer-grained locking on the artifact
        // directory, so just lock the entire thing for the duration of this
        // compile.
        let lock = dest.open_rw(".cargo-lock", ws.config(), "build directory")?;
        let root = root.into_path_unlocked();
        let dest = dest.into_path_unlocked();

        Ok(Layout {
            deps: dest.join("deps"),
            build: dest.join("build"),
            incremental: dest.join("incremental"),
            fingerprint: dest.join(".fingerprint"),
            examples: dest.join("examples"),
            doc: root.join("doc"),
            src: root.join("doc/src"),
            tmp: dest.join("tmp"),
            root,
            dest,
            _lock: lock,
        })
    }

    /// Makes sure all directories stored in the Layout exist on the filesystem.
    pub fn prepare(&mut self) -> CargoResult<()> {
        paths::create_dir_all(&self.deps)?;
        paths::create_dir_all(&self.incremental)?;
        paths::create_dir_all(&self.fingerprint)?;
        paths::create_dir_all(&self.examples)?;
        paths::create_dir_all(&self.build)?;

        Ok(())
    }

    /// Fetch the destination path for final artifacts  (`/…/target/debug`).
    pub fn dest(&self) -> &Path {
        &self.dest
    }
    /// Fetch the deps path.
    pub fn deps(&self) -> &Path {
        &self.deps
    }
    /// Fetch the examples path.
    pub fn examples(&self) -> &Path {
        &self.examples
    }
    /// Fetch the doc path.
    pub fn doc(&self) -> &Path {
        &self.doc
    }
    /// Fetch the doc/src path.
    pub fn src(&self) -> &Path {
        &self.src
    }
    /// Fetch the root path (`/…/target`).
    pub fn root(&self) -> &Path {
        &self.root
    }
    /// Fetch the incremental path.
    pub fn incremental(&self) -> &Path {
        &self.incremental
    }
    /// Fetch the fingerprint path.
    pub fn fingerprint(&self) -> &Path {
        &self.fingerprint
    }
    /// Fetch the build script path.
    pub fn build(&self) -> &Path {
        &self.build
    }
    /// Create and return the tmp path.
    pub fn prepare_tmp(&self) -> CargoResult<&Path> {
        paths::create_dir_all(&self.tmp)?;
        Ok(&self.tmp)
    }
}
