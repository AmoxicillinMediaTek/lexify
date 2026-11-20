mod antitamper;
mod boolean_obfuscator;
pub mod config;
mod control_flow;
mod dead_code;
mod env_wrapper;
mod formatter;
mod lexer;
mod number_obfuscator;
mod parser;
mod renamer;
mod string_encrypt;
mod string_splitter;
mod table_obfuscator;
mod vm;
mod wrapper;

use config::Config;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

pub use config::Level;

/// Obfuscate Lua source in memory using the selected transformations.
/// Without an explicit seed every call draws fresh entropy, so the same input
/// produces a different transformation on each run.
pub fn obfuscate(source: &str, mut config: Config) -> Result<String, String> {
    let parsed = parser::parse(source).map_err(|e| format!("input parse error: {e}"))?;
    config.apply_level();
    let mut tokens = parsed.tokens;
    let mut rng = match config.seed {
        Some(seed) => ChaCha8Rng::seed_from_u64(seed),
        None => ChaCha8Rng::from_entropy(),
    };

    if config.rename {
        renamer::transform(&mut tokens, &mut rng);
    }
    if config.split_strings {
        let options = string_splitter::Options {
            split_size: config.split_size,
            lookup_tables: config.lookup_tables,
            randomize_lookup: config.randomize_lookup,
            shared_lookup: config.shared_lookup,
        };
        let renamed_source = formatter::format(&tokens, false);
        let split = string_splitter::transform(&renamed_source, &mut rng, &options)?;
        tokens = lexer::lex(&split)?;
    }
    // AST splitting already represents literal values as byte fragments.
    if config.encrypt_strings && !config.split_strings {
        string_encrypt::transform(&mut tokens, &mut rng)?;
    }
    if config.obfuscate_numbers && !config.split_strings {
        number_obfuscator::transform(&mut tokens);
    }
    if config.obfuscate_booleans {
        boolean_obfuscator::transform(&mut tokens);
    }
    if config.obfuscate_tables {
        table_obfuscator::transform(&mut tokens);
    }
    if config.control_flow {
        control_flow::transform(&mut tokens);
    }
    if config.dead_code {
        dead_code::transform(&mut tokens, &mut rng)?;
    }
    // CFF and the historical VM option share this safe block-dispatch lowering.
    // Treat them as aliases so enabling both never wraps the generated dispatcher again.
    if config.cff || config.vm {
        let formatted = formatter::format(&tokens, false);
        let flattened = vm::transform(&formatted, &mut rng)?;
        tokens = lexer::lex(&flattened)?;
    }

    let mut output = formatter::format(&tokens, config.minify);
    if config.wrapper {
        output = wrapper::transform(&output, &mut rng);
    }
    if config.envwrap {
        output = env_wrapper::transform(&output, &mut rng);
    }
    if config.antitamper {
        output = antitamper::transform(&output, &mut rng);
    } else {
        output = format!("-- [[script protected by lexify]]\n{output}");
    }
    parser::parse(&output).map_err(|e| format!("transformed output failed validation: {e}"))?;
    check_forbidden(&output, config.allow_loadstring)?;
    Ok(output)
}

fn check_forbidden(code: &str, allow_loadstring: bool) -> Result<(), String> {
    let tokens = lexer::lex(code)?;
    for window in tokens.windows(3) {
        if window[0].text == "string" && window[1].text == "." && window[2].text == "char" {
            return Err("output contains forbidden string.char access".into());
        }
    }
    if allow_loadstring {
        return Ok(());
    }
    for token in tokens {
        if token.kind == lexer::Kind::Ident && (token.text == "load" || token.text == "loadstring")
        {
            return Err(format!(
                "output contains forbidden identifier {}",
                token.text
            ));
        }
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::{obfuscate, Config};
    use wasm_bindgen::prelude::*;

    fn bool_option(options: &serde_json::Value, key: &str) -> bool {
        options
            .get(key)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    }

    #[wasm_bindgen]
    pub fn wasm_obfuscate(payload: &str) -> Result<String, JsValue> {
        let value: serde_json::Value = serde_json::from_str(payload)
            .map_err(|error| JsValue::from_str(&format!("Invalid request: {error}")))?;
        let source = value
            .get("source")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| JsValue::from_str("Enter some Lua source first."))?;
        let mut config = Config::default();
        // The UI expands presets into explicit checkboxes. Avoid applying the preset
        // a second time so user-disabled passes stay disabled.
        config.level = super::Level::Basic;
        config.minify = bool_option(&value, "minify");
        config.rename = bool_option(&value, "rename");
        config.encrypt_strings = bool_option(&value, "encryptStrings");
        config.obfuscate_numbers = bool_option(&value, "obfuscateNumbers");
        config.obfuscate_booleans = bool_option(&value, "obfuscateBooleans");
        config.obfuscate_tables = bool_option(&value, "obfuscateTables");
        config.control_flow = bool_option(&value, "controlFlow");
        config.cff = bool_option(&value, "cff");
        config.dead_code = bool_option(&value, "deadCode");
        config.split_strings = bool_option(&value, "splitStrings");
        config.lookup_tables = bool_option(&value, "lookupTables");
        config.randomize_lookup = bool_option(&value, "randomizeLookup");
        config.shared_lookup = bool_option(&value, "sharedLookup");
        config.vm = bool_option(&value, "vm");
        config.wrapper = bool_option(&value, "wrapper");
        config.envwrap = bool_option(&value, "envwrap");
        config.allow_loadstring = bool_option(&value, "allowLoadstring");
        config.antitamper = bool_option(&value, "antitamper");
        config.split_size = value
            .get("splitSize")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(4)
            .clamp(1, 64) as usize;
        obfuscate(source, config).map_err(|error| JsValue::from_str(&error))
    }
}
