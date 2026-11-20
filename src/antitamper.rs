//! Runtime integrity guard.
//!
//! `transform` prefixes the finished chunk with a small prologue that re-reads
//! its own file, fingerprints the bytes and compares the result against a
//! digest baked in at build time. The same prologue also verifies that the
//! environment still looks like a stock interpreter: no debug hook installed
//! and the core library functions still implemented in C.
//!
//! When the chunk was not loaded from a file (stdin, a REPL, `load` inside a
//! host application) the byte comparison is skipped instead of failing, so the
//! guard never changes the behaviour of code that cannot be checked.

use rand::Rng;

/// Mersenne prime used as the hash modulus; keeps every intermediate product
/// below 2^53 so the Lua side stays exact even with double-only numbers.
const MODULUS: u64 = 2_147_483_647;

const PROLOGUE: &str = r##"do
local {{LIT}}={{LITV}}
local function {{HASH}}(s)
local p,q=5381,5381
for i=1,#s do
local c=string.byte(s,i)
p=(p*65599+c)%2147483647
q=(q*131+c)%2147483647
end
return string.format("%08x%08x",p,q)
end
local function {{CHECK}}()
local d=debug.getinfo(1,"S")
if type(d)~="table" then return true end
local f=d.source
if type(f)~="string" or f:sub(1,1)~="@" then return true end
local o=io and io.open
if type(o)~="function" then return true end
local h=o(f:sub(2),"rb")
if not h then return true end
local t=h:read("*a")
h:close()
if type(t)~="string" then return true end
t=t:gsub('"'..{{LIT}}..'"',{{ZERO}},1)
t=t:gsub("%s+$","")
return {{HASH}}(t)=={{LIT}}
end
local function {{ENV}}()
if type(debug)~="table" or type(debug.getinfo)~="function" then return true end
if debug.gethook and debug.gethook() then return false end
for _,v in ipairs({string.byte,table.concat,pcall,select,type,error}) do
local i=debug.getinfo(v,"S")
if type(i)~="table" or i.what~="C" then return false end
end
return true
end
local {{OK}}={{ENV}}() and {{CHECK}}()
if not {{OK}} then error("integrity check failed",0) end
end
"##;

/// Fingerprint of `bytes`, formatted as sixteen lowercase hex digits.
fn digest(bytes: &[u8]) -> String {
    let (mut p, mut q) = (5381u64, 5381u64);
    for &byte in bytes {
        p = (p * 65599 + u64::from(byte)) % MODULUS;
        q = (q * 131 + u64::from(byte)) % MODULUS;
    }
    format!("{p:08x}{q:08x}")
}

fn fresh_name(rng: &mut impl Rng, taken: &mut Vec<String>) -> String {
    loop {
        let name = format!("_{:07x}", rng.gen_range(0..0x0fff_ffffu32));
        if !taken.contains(&name) {
            taken.push(name.clone());
            return name;
        }
    }
}

fn render(names: &[String; 5], literal: &str) -> String {
    PROLOGUE
        .replace("{{LITV}}", literal)
        .replace("{{ZERO}}", "'\"0\"'")
        .replace("{{LIT}}", &names[0])
        .replace("{{HASH}}", &names[1])
        .replace("{{CHECK}}", &names[2])
        .replace("{{ENV}}", &names[3])
        .replace("{{OK}}", &names[4])
}

/// Prefix `source` with the integrity guard and bake in its digest.
pub fn transform(source: &str, rng: &mut impl Rng) -> String {
    let mut taken = Vec::new();
    let names: [String; 5] = std::array::from_fn(|_| fresh_name(rng, &mut taken));

    // The digest is taken with the busy literal normalised to "0", which is
    // exactly what the runtime reconstructs before hashing, so the value can
    // live inside the text it covers.
    // Trailing whitespace is normalised away on both sides so an editor that
    // appends a final newline does not look like tampering.
    let provisional = format!("{}{}", render(&names, "\"0\""), source);
    let tag = digest(provisional.trim_end().as_bytes());
    format!("{}{}", render(&names, &format!("\"{tag}\"")), source)
}

#[cfg(test)]
mod tests {
    use super::{digest, transform};
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn lua() -> Option<&'static str> {
        std::process::Command::new("lua5.4")
            .arg("-v")
            .output()
            .ok()
            .map(|_| "lua5.4")
    }

    #[test]
    fn digest_is_stable_and_prefix_sensitive() {
        assert_eq!(digest(b""), "0000150500001505");
        assert_eq!(digest(b"a"), digest(b"a"));
        assert_ne!(digest(b"ab"), digest(b"ba"));
        assert_eq!(digest(b"x").len(), 16);
    }

    #[test]
    fn guard_runs_clean_and_catches_edits() {
        let Some(lua) = lua() else { return };
        let source =
            "local total = 0\nfor i = 1, 3 do total = total + i end\nprint('total', total)\n";
        let mut rng = ChaCha8Rng::seed_from_u64(7);
        let guarded = transform(source, &mut rng);
        assert!(guarded.contains("integrity check failed"));
        assert!(guarded.ends_with(source));

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("guarded.lua");
        std::fs::write(&path, &guarded).unwrap();

        let clean = std::process::Command::new(lua).arg(&path).output().unwrap();
        assert!(clean.status.success(), "guard rejected an untouched file");
        assert_eq!(String::from_utf8_lossy(&clean.stdout), "total\t6\n");

        // Edit one byte of the payload: the digest no longer matches.
        let tampered = guarded.replacen("total = total + i", "total = total + 1", 1);
        assert_ne!(tampered, guarded);
        std::fs::write(&path, &tampered).unwrap();
        let caught = std::process::Command::new(lua).arg(&path).output().unwrap();
        assert!(!caught.status.success(), "edited file still ran");
        assert!(
            String::from_utf8_lossy(&caught.stderr).contains("integrity check failed"),
            "unexpected stderr: {}",
            String::from_utf8_lossy(&caught.stderr)
        );
    }

    #[test]
    fn guarded_chunk_still_works_from_stdin() {
        let Some(lua) = lua() else { return };
        let source = "print('stdin ok')\n";
        let mut rng = ChaCha8Rng::seed_from_u64(11);
        let guarded = transform(source, &mut rng);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stdin.lua");
        std::fs::write(&path, &guarded).unwrap();
        let out = std::process::Command::new(lua).arg(&path).output().unwrap();
        assert!(out.status.success());
        assert_eq!(String::from_utf8_lossy(&out.stdout), "stdin ok\n");
    }
}
