use std::error::Error;
use std::process::Command;

/// Run a git command and return its trimmed stdout, or `None` when git is
/// absent, fails, or the directory is not a repository.
fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?.trim().to_owned();
    if text.is_empty() { None } else { Some(text) }
}

fn main() -> Result<(), Box<dyn Error>> {
    // vergen supplies what does not depend on git. Its gitcl feature is
    // deliberately not used: when the build happens outside a worktree it
    // falls back to the literal string VERGEN_IDEMPOTENT_OUTPUT, and that
    // then appears in --version. Every AUR build hits exactly that case,
    // because makepkg unpacks a source tarball with no .git, and the bug
    // report template asks users for this output.
    vergen::Emitter::default()
        .add_instructions(
            &vergen::BuildBuilder::default()
                .build_timestamp(true)
                .build()?,
        )?
        .add_instructions(
            &vergen::CargoBuilder::default()
                .target_triple(true)
                .build()?,
        )?
        .add_instructions(&vergen::RustcBuilder::default().semver(true).build()?)?
        .emit()?;

    // The git parts are asked here so the fallback is ours to choose. Absent
    // is not an error; it is the normal case for a distribution build.
    let sha = git(&["rev-parse", "--short=8", "HEAD"]).unwrap_or_else(|| "unknown".to_owned());
    let commit_date = git(&["log", "-1", "--format=%cs"]).unwrap_or_else(|| "unknown".to_owned());
    // Only meaningful inside a worktree; a tarball build is clean by
    // definition and must not be labelled dirty.
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"]).is_some();

    // A release build has to know its own commit. v0.3.4 shipped a .deb whose
    // --version read "unknown, unknown" while the musl payload from the same
    // run read "44d4d5bc": the gnu payloads are built in a container that runs
    // as root over a workspace owned by uid 1001, git refuses that as dubious
    // ownership, and the exception actions/checkout writes goes into a
    // temporary HOME it takes away again. The fallback above then did exactly
    // what it is for, and nothing turned red. That output is what the bug
    // report template asks users for, so it must not degrade quietly.
    //
    // Only the release workflow sets this. A tarball build, which is the case
    // the fallback exists for, never sees it.
    println!("cargo::rerun-if-env-changed=BUPS_REQUIRE_GIT_PROVENANCE");
    if std::env::var_os("BUPS_REQUIRE_GIT_PROVENANCE").is_some()
        && (sha == "unknown" || commit_date == "unknown")
    {
        return Err(format!(
            "BUPS_REQUIRE_GIT_PROVENANCE is set but git answered nothing \
             (sha {sha}, date {commit_date}); the binary cannot label itself"
        )
        .into());
    }

    println!("cargo::rustc-env=BUPS_GIT_SHA={sha}");
    println!("cargo::rustc-env=BUPS_GIT_DATE={commit_date}");
    println!(
        "cargo::rustc-env=BUPS_DIRTY_SUFFIX={}",
        if dirty { "-dirty" } else { "" }
    );

    // Without this the values would be baked in at the first build and never
    // re-evaluated. Both files are absent in a tarball build, which cargo
    // treats as "nothing to watch" rather than an error.
    println!("cargo::rerun-if-changed=.git/HEAD");
    println!("cargo::rerun-if-changed=.git/index");

    Ok(())
}
