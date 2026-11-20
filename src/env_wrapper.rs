//! Optional global-environment proxy.
//!
//! The payload runs in a fresh `_ENV` proxy. Reads and ordinary writes are
//! forwarded to the original environment, while `_G` refers to the proxy itself.
//! A few raw-table operations are adapted so inspecting `_G` does not hand out
//! the backing global table and `rawset(_G, ...)` cannot bypass the proxy.

use rand::{Rng, RngCore};

const NAMES: usize = 10;

fn fresh_name(source: &str, taken: &mut Vec<String>, rng: &mut impl Rng) -> String {
    loop {
        let name = format!("_lx{:07x}", rng.gen_range(0..0x0fff_ffffu32));
        if !source.contains(&name) && !taken.contains(&name) {
            taken.push(name.clone());
            return name;
        }
    }
}

/// Render `source` into an environment proxy while preserving chunk varargs
/// and all return values.
pub fn render(source: &str, rng: &mut impl RngCore) -> String {
    let mut taken = Vec::with_capacity(NAMES);
    let names: [String; NAMES] = std::array::from_fn(|_| fresh_name(source, &mut taken, rng));
    let [real, proxy, orig_rawget, orig_rawset, orig_next, orig_pairs, rawget_fn, rawset_fn, next_fn, pairs_fn] =
        names;

    format!(
        "local {real}=_ENV\nlocal {proxy}={{}}\nlocal {orig_rawget}=rawget\nlocal {orig_rawset}=rawset\nlocal {orig_next}=next\nlocal {orig_pairs}=pairs\nlocal {rawget_fn}=function(t,k) if t=={proxy} then if k==\"_G\" then return {proxy} elseif k==\"rawget\" then return {rawget_fn} elseif k==\"rawset\" then return {rawset_fn} elseif k==\"next\" then return {next_fn} elseif k==\"pairs\" then return {pairs_fn} end return {orig_rawget}({real},k) end return {orig_rawget}(t,k) end\nlocal {rawset_fn}=function(t,k,v) if t=={proxy} then error(\"cannot rawset the protected global environment\",2) end return {orig_rawset}(t,k,v) end\nlocal {next_fn}=function(t,k) if t=={proxy} then local nk,nv={orig_next}({real},k) if nk==\"_G\" then nv={proxy} elseif nk==\"rawget\" then nv={rawget_fn} elseif nk==\"rawset\" then nv={rawset_fn} elseif nk==\"next\" then nv={next_fn} elseif nk==\"pairs\" then nv={pairs_fn} end return nk,nv end return {orig_next}(t,k) end\nlocal {pairs_fn}=function(t) if t=={proxy} then local function iter(_,k) local nk,nv={orig_next}({real},k) if nk==\"_G\" then nv={proxy} elseif nk==\"rawget\" then nv={rawget_fn} elseif nk==\"rawset\" then nv={rawset_fn} elseif nk==\"next\" then nv={next_fn} elseif nk==\"pairs\" then nv={pairs_fn} end return nk,nv end return iter,{proxy},nil end return {orig_pairs}(t) end\nsetmetatable({proxy},{{__index=function(_,k) if k==\"_G\" then return {proxy} elseif k==\"rawget\" then return {rawget_fn} elseif k==\"rawset\" then return {rawset_fn} elseif k==\"next\" then return {next_fn} elseif k==\"pairs\" then return {pairs_fn} end return {real}[k] end,__newindex=function(_,k,v) if k==\"_G\" then error(\"cannot replace the protected global environment\",2) end {real}[k]=v end,__metatable=false}})\nreturn (function(_ENV,...)\n{source}\nend)({proxy},...)"
    )
}

/// Wrap a chunk with a randomly named proxy environment.
pub fn transform(source: &str, rng: &mut impl RngCore) -> String {
    render(source, rng)
}

#[cfg(test)]
mod tests {
    use super::render;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn emits_parseable_lua() {
        let output = render("print('hello')", &mut ChaCha8Rng::seed_from_u64(1));
        crate::parser::parse(&output).unwrap();
    }

    #[test]
    fn preserves_globals_varargs_and_returns() {
        let Some(lua) = std::process::Command::new("lua5.4").arg("-v").output().ok() else {
            return;
        };
        let _ = lua;
        let source = "print(_G == _ENV, select('#', ...), ...)\n_G.lexify_envwrap_test = 42\nreturn _G.lexify_envwrap_test, select('#', ...)";
        let output = render(source, &mut ChaCha8Rng::seed_from_u64(2));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("env.lua");
        std::fs::write(&path, output).unwrap();
        let result = std::process::Command::new("lua5.4")
            .arg(&path)
            .arg("first")
            .arg("second")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&result.stdout),
            "true\t2\tfirst\tsecond\n"
        );
    }

    #[test]
    fn protects_backing_table_from_raw_access() {
        let Some(_) = std::process::Command::new("lua5.4").arg("-v").output().ok() else {
            return;
        };
        let source = "local ok = pcall(rawset, _G, 'print', false)\nlocal seen=false for k,v in pairs(_G) do if k=='_G' then seen=(v==_G) end end\nprint(ok, rawget(_G, 'type') == type, rawget(_G, '_G') == _G, seen, getmetatable(_G) == false)";
        let output = render(source, &mut ChaCha8Rng::seed_from_u64(3));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("env.lua");
        std::fs::write(&path, output).unwrap();
        let result = std::process::Command::new("lua5.4")
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&result.stdout),
            "false\ttrue\ttrue\ttrue\ttrue\n"
        );
    }
}
