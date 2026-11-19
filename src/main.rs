use clap::Parser as ClapParser;
use lexify_obfuscator::{config::Config, obfuscate, Level};
use std::{fs, path::PathBuf};

#[derive(ClapParser, Debug)]
#[command(
    name = "lexify-obfuscator",
    version,
    about = "Lexify Obfuscator: AST-validated Lua/Luau source transformer"
)]
struct Args {
    input: PathBuf,
    #[arg(short, long)]
    output: Option<PathBuf>,
    #[arg(long, value_enum, default_value = "basic")]
    level: Level,
    #[arg(long)]
    minify: bool,
    #[arg(long)]
    rename: bool,
    #[arg(long)]
    encrypt_strings: bool,
    #[arg(long)]
    obfuscate_numbers: bool,
    #[arg(long)]
    control_flow: bool,
    #[arg(long)]
    obfuscate_booleans: bool,
    #[arg(long)]
    obfuscate_tables: bool,
    #[arg(long)]
    dead_code: bool,
    /// Rewrite string literals as concatenated fragments (AST-backed).
    #[arg(long)]
    split_strings: bool,
    /// Store fragments in lookup tables instead of inlining them.
    #[arg(long)]
    lookup_tables: bool,
    /// Random table keys and chunk boundaries instead of sequential keys.
    #[arg(long)]
    randomize_lookup: bool,
    /// Keep every fragment in one shared lookup table.
    #[arg(long)]
    shared_lookup: bool,
    /// Maximum bytes per fragment.
    #[arg(long, default_value_t = 4, value_parser = clap::builder::RangedU64ValueParser::<usize>::new().range(1..))]
    split_size: usize,
    #[arg(long, default_value = "lua54")]
    target: String,
}

fn run(args: Args) -> Result<(), String> {
    let source =
        fs::read_to_string(&args.input).map_err(|e| format!("{}: {e}", args.input.display()))?;
    let config = Config {
        level: args.level,
        minify: args.minify,
        rename: args.rename,
        encrypt_strings: args.encrypt_strings,
        obfuscate_numbers: args.obfuscate_numbers,
        obfuscate_booleans: args.obfuscate_booleans,
        obfuscate_tables: args.obfuscate_tables,
        dead_code: args.dead_code,
        control_flow: args.control_flow,
        split_strings: args.split_strings,
        lookup_tables: args.lookup_tables,
        randomize_lookup: args.randomize_lookup,
        shared_lookup: args.shared_lookup,
        split_size: args.split_size,
        target: args.target,
        seed: None,
    };
    let output = obfuscate(&source, config)?;
    if let Some(path) = args.output {
        fs::write(&path, output).map_err(|e| format!("{}: {e}", path.display()))?;
    } else {
        print!("{output}");
    }
    Ok(())
}

fn main() {
    if let Err(error) = run(Args::parse()) {
        eprintln!("lexify-obfuscator: {error}");
        std::process::exit(2)
    }
}
