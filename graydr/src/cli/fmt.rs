use crate::cli::args::FmtArgs;

pub fn run_fmt(args: FmtArgs) -> anyhow::Result<()> {
    let mut any_changed = false;
    for path in &args.files {
        let changed = crate::fmt::format_file(path, args.check)?;
        if changed {
            any_changed = true;
            if args.check {
                eprintln!("would reformat: {}", path.display());
            } else {
                eprintln!("formatted: {}", path.display());
            }
        }
    }
    if args.check && any_changed {
        std::process::exit(1);
    }
    Ok(())
}
