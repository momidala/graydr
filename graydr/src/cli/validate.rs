use std::fs;
use crate::parser::module::parse_module_file;
use crate::parser::template::parse_template_file;
use crate::cli::args::ValidateArgs;

pub fn run_validate(args: ValidateArgs) {
    let mut has_error = false;

    for path in &args.files {
        let filename = path.to_string_lossy();
        let source = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error reading {}: {}", filename, e);
                has_error = true;
                continue;
            }
        };
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let result = match ext {
            "gmod" => parse_module_file(&source, &filename).map(|_| ()),
            "gtpl" => parse_template_file(&source, &filename).map(|_| ()),
            _ => {
                eprintln!("warning: skipping unknown file type: {}", filename);
                continue;
            }
        };
        if let Err(e) = result {
            eprintln!("{}", e);
            has_error = true;
        }
    }

    if has_error {
        std::process::exit(1);
    }
}
