use crate::command_prelude::*;

use cargo::ops::{self, UpdateOptions};

pub fn cli() -> App {
    subcommand("update")
        .about("Update dependencies as recorded in the local lock file")
        .arg_package_spec_simple("Package to update")
        .arg(opt(
            "aggressive",
            "Force updating all dependencies of <name> as well",
        ))
        .arg_dry_run("Don't actually write the lockfile")
        .arg(opt("precise", "Update a single dependency to exactly PRECISE").value_name("PRECISE"))
        .arg_manifest_path()
        .after_help(
            "\
This command requires that a `Cargo.lock` already exists as generated by
`cargo build` or related commands.

If SPEC is given, then a conservative update of the lockfile will be
performed. This means that only the dependency specified by SPEC will be
updated. Its transitive dependencies will be updated only if SPEC cannot be
updated without updating dependencies.  All other dependencies will remain
locked at their currently recorded versions.

If PRECISE is specified, then --aggressive must not also be specified. The
argument PRECISE is a string representing a precise revision that the package
being updated should be updated to. For example, if the package comes from a git
repository, then PRECISE would be the exact revision that the repository should
be updated to.

If SPEC is not given, then all dependencies will be re-resolved and
updated.

For more information about package id specifications, see `cargo help pkgid`.
",
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;

    let update_opts = UpdateOptions {
        aggressive: args.is_present("aggressive"),
        precise: args.value_of("precise"),
        to_update: values(args, "package"),
        dry_run: args.is_present("dry-run"),
        config,
    };
    ops::update_lockfile(&ws, &update_opts)?;
    Ok(())
}
