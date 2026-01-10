use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    vergen_gitcl::Emitter::default()
        .add_instructions(
            &vergen_gitcl::BuildBuilder::default()
                .build_timestamp(true)
                .build()?,
        )?
        .add_instructions(
            &vergen_gitcl::CargoBuilder::default()
                .target_triple(true)
                .build()?,
        )?
        .add_instructions(
            &vergen_gitcl::GitclBuilder::default()
                .sha(true)
                .commit_timestamp(true)
                .dirty(true)
                .build()?,
        )?
        .add_instructions(&vergen_gitcl::RustcBuilder::default().semver(true).build()?)?
        .emit()?;
    Ok(())
}
