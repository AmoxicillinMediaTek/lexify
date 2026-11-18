mod ast;
mod config;
mod control_flow;
mod boolean_obfuscator;
mod dead_code;
mod table_obfuscator;
mod formatter;
mod lexer;
mod number_obfuscator;
mod parser;
mod renamer;
mod string_encrypt;

use clap::Parser as ClapParser;
use config::{Config,Level};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::{fs, path::PathBuf};

#[derive(ClapParser,Debug)]
#[command(name="lexify-obfuscator", version, about="Lexify Obfuscator: AST-validated Lua/Luau source transformer")]
struct Args {
    input: PathBuf,
    #[arg(short, long)] output: Option<PathBuf>,
    #[arg(long, value_enum, default_value="basic")] level: Level,
    #[arg(long)] seed: Option<u64>,
    #[arg(long)] minify: bool,
    #[arg(long)] rename: bool,
    #[arg(long)] encrypt_strings: bool,
    #[arg(long)] obfuscate_numbers: bool,
    #[arg(long)] control_flow: bool,
    #[arg(long)] obfuscate_booleans: bool,
    #[arg(long)] obfuscate_tables: bool,
    #[arg(long)] dead_code: bool,
    #[arg(long, default_value="lua54")] target: String,
}

fn run(args: Args)->Result<(),String>{
    let source=fs::read_to_string(&args.input).map_err(|e|format!("{}: {e}",args.input.display()))?;
    parser::parse(&source).map_err(|e|format!("input parse error: {e}"))?;
    let mut cfg=Config{level:args.level,minify:args.minify,rename:args.rename,encrypt_strings:args.encrypt_strings,obfuscate_numbers:args.obfuscate_numbers,obfuscate_booleans:args.obfuscate_booleans,obfuscate_tables:args.obfuscate_tables,dead_code:args.dead_code,control_flow:args.control_flow,target:args.target,seed:args.seed}; cfg.apply_level();
    let _target=&cfg.target;
    let mut tokens=lexer::lex(&source)?;
    let mut rng=ChaCha8Rng::seed_from_u64(cfg.seed.unwrap_or_else(||rand::random()));
    if cfg.rename {renamer::transform(&mut tokens,&mut rng);}
    if cfg.encrypt_strings { string_encrypt::transform(&mut tokens,&mut rng)?; }
    if cfg.obfuscate_numbers { number_obfuscator::transform(&mut tokens); }
    if cfg.obfuscate_booleans {boolean_obfuscator::transform(&mut tokens);}
    if cfg.obfuscate_tables {table_obfuscator::transform(&mut tokens);}
    if cfg.control_flow {control_flow::transform(&mut tokens);}
    if cfg.dead_code {dead_code::transform(&mut tokens,&mut rng)?;}
    let output=formatter::format(&tokens,cfg.minify);
    parser::parse(&output).map_err(|e|format!("transformed output failed validation: {e}"))?;
    check_forbidden(&output)?;
    if let Some(path)=args.output {fs::write(&path,output).map_err(|e|format!("{}: {e}",path.display()))?;}else{print!("{output}");}
    Ok(())
}
fn check_forbidden(code:&str)->Result<(),String>{
    let tokens=lexer::lex(code)?;
    for w in tokens.windows(3){ if w[0].text=="string"&&w[1].text=="."&&w[2].text=="char" {return Err("output contains forbidden string.char access".into())} }
    for t in tokens { if t.kind==lexer::Kind::Ident && (t.text=="load"||t.text=="loadstring") {return Err(format!("output contains forbidden identifier {}",t.text))} }
    Ok(())
}
fn main(){if let Err(e)=run(Args::parse()){eprintln!("lexify-obfuscator: {e}");std::process::exit(2)}}

#[cfg(test)] mod tests {
    use super::*;
    fn transform(s:&str,level:Level,seed:Option<u64>)->String{
        let mut t=lexer::lex(s).unwrap(); let mut c=Config{level,minify:true,rename:false,encrypt_strings:false,obfuscate_numbers:false,obfuscate_booleans:false,obfuscate_tables:false,dead_code:false,control_flow:false,target:"lua54".into(),seed}; c.apply_level(); let mut rng=ChaCha8Rng::seed_from_u64(seed.unwrap_or(3));if c.rename{renamer::transform(&mut t,&mut rng);}if c.encrypt_strings{let _=string_encrypt::transform(&mut t,&mut rng);}if c.obfuscate_numbers{number_obfuscator::transform(&mut t)}if c.obfuscate_booleans{boolean_obfuscator::transform(&mut t)}if c.obfuscate_tables{table_obfuscator::transform(&mut t)}if c.control_flow{control_flow::transform(&mut t);}if c.dead_code{let _=dead_code::transform(&mut t,&mut rng);}formatter::format(&t,true)
    }
    #[test] fn parser_handles_nested_closure_and_loops(){parser::parse("local function f(x) local y=2; return function(z) for i=1,3 do y=y+z+i end return y end end").unwrap();}
    #[test] fn minify_validates_and_retains_behavioral_syntax(){let s="local x = 1\nif x < 2 then print('ok') end";let o=transform(s,Level::Basic,None);parser::parse(&o).unwrap();assert!(o.contains("print"));}
    #[test] fn strings_reencoded_without_forbidden_functions(){let mut t=lexer::lex("print('hello')").unwrap();let mut rng=ChaCha8Rng::seed_from_u64(1);string_encrypt::transform(&mut t,&mut rng).unwrap();let o=formatter::format(&t,true);assert!(o.contains("table.concat"));assert!(!o.contains("string.char"));}
    #[test] fn numeric_identity_transformation_parses(){let mut t=lexer::lex("local x=15").unwrap();number_obfuscator::transform(&mut t);let o=formatter::format(&t,true);parser::parse(&o).unwrap();assert!(o.contains("14+1"));}
    #[test] fn forbidden_access_detected(){assert!(check_forbidden("local x=string.char(1)").is_err());assert!(check_forbidden("load('x')").is_err());}
    #[test] fn deterministic_seed_is_stable(){assert_eq!(transform("local x=5",Level::Basic,Some(4)),transform("local x=5",Level::Basic,Some(4)));}
    #[test] fn renamer_preserves_closure_and_globals(){let src="local outer=3;local function f(x)local inner=x+outer;return function(y)return inner+y end end;print(f(4)(5))";let mut ts=lexer::lex(src).unwrap();let mut rng=ChaCha8Rng::seed_from_u64(19);renamer::transform(&mut ts,&mut rng);let out=formatter::format(&ts,true);assert!(!out.contains("outer"));assert!(!out.contains("inner"));assert!(out.contains("print"));parser::parse(&out).unwrap();}
    #[test] fn boolean_control_and_table_transforms_parse(){let src="local t={a=true,2,false};if t.a then print(t[1]) end";let mut ts=lexer::lex(src).unwrap();boolean_obfuscator::transform(&mut ts);table_obfuscator::transform(&mut ts);control_flow::transform(&mut ts);let out=formatter::format(&ts,true);parser::parse(&out).unwrap();assert!(out.contains("not not"));}
    #[test] fn dead_code_is_inserted(){let mut ts=lexer::lex("print(1)").unwrap();let mut rng=ChaCha8Rng::seed_from_u64(22);dead_code::transform(&mut ts,&mut rng).unwrap();let out=formatter::format(&ts,true);assert!(out.starts_with("if false"));parser::parse(&out).unwrap();}
}
