//! Runtime integrity guard.
//!
//! `transform` prefixes the finished chunk with a small prologue that re-reads
//! its own file, fingerprints the bytes and compares the result against a
//! digest baked in at build time. The same prologue also verifies that it is
//! running on a stock interpreter before it trusts anything it reads:
//!
//! * `debug.info` / `debug.getinfo` are captured on the very first line, before
//!   any other code can replace the `debug` table, and every later probe uses
//!   that captured reference.
//! * The captured reader must itself be a C function. A Lua replacement answers
//!   "Lua" for `what` (or anything but `"[C]"` for `s`), and asking a fake
//!   about itself is exactly the check that catches it.
//! * The reader is then called with bad arguments. Errors raised inside C carry
//!   no `source:line:` position, so a call mediated by a Lua frame advertises
//!   itself in the message. A reader that swallows those errors never produces
//!   the expected diagnostic and is rejected as well.
//! * The `linedefined` of the guard's own checker is used as a seed: it decodes
//!   a sealed marker whose plaintext fingerprint is baked in. Being one line off
//!   anywhere above the checker (padding, deleted or inserted lines) yields a
//!   different seed and fails the marker instead of merely flipping a flag.
//!
//! When the chunk was not loaded from a file (stdin, a REPL, `load` inside a
//! host application) the byte comparison is skipped instead of failing, so the
//! guard never changes the behaviour of code that cannot be checked. The
//! environment check has no such escape: a missing or unrecognisable debug
//! library is treated as tampering, because nothing can be verified there.

use rand::Rng;

/// Mersenne prime used as the hash modulus; keeps every intermediate product
/// below 2^53 so the Lua side stays exact even with double-only numbers.
const MODULUS: u64 = 2_147_483_647;

/// Number of randomised identifiers substituted into [`PROLOGUE`].
const NAMES: usize = 13;

const PROLOGUE: &str = r##"-- [[script protected by lexify]]
do
local {{INFO}}=type(debug)=="table" and debug.info or nil
local {{GI}}=type(debug)=="table" and debug.getinfo or nil
local {{HK}}=type(debug)=="table" and debug.gethook or nil
local {{FN}}={{INFO}} or {{GI}}
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
local function {{CRT}}(f)
if type(f)~="function" then return false end
if {{GI}}~=nil then
local ok,r=pcall({{GI}},f,"S")
if ok and type(r)=="table" then return r.what=="C" end
end
if {{INFO}}~=nil then
local ok,r=pcall({{INFO}},f,"s")
if ok and r=="[C]" then return true end
end
return false
end
local function {{LNF}}(f)
if {{GI}}~=nil then
local ok,r=pcall({{GI}},f,"S")
if ok and type(r)=="table" and type(r.linedefined)=="number" then return r.linedefined end
end
if {{INFO}}~=nil then
local ok,r=pcall({{INFO}},f,"S")
if ok and type(r)=="table" and type(r.linedefined)=="number" then return r.linedefined end
local ok2,n=pcall({{INFO}},f,"l")
if ok2 and type(n)=="number" then return n end
end
return nil
end
local function {{PRB}}(a,b)
local ok,e=pcall({{FN}},a,b)
if ok then return false end
if type(e)=="string" and not e:find("\n",1,true) and e:find("argument",1,true) and not e:find("^[^:]*:%d+:") then return true end
return nil
end
local function {{ERR}}()
local a,b,c={{PRB}}(1,"\1"),{{PRB}}("\1",1),{{PRB}}(1,5)
if a==nil or b==nil or c==nil then return false end
return a or b or c
end
local function {{CHECK}}()
local d={{GI}} and {{GI}}(1,"S")
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
if type(debug)~="table" then return false end
if not {{CRT}}({{FN}}) then return false end
if {{HK}}~=nil then
if not {{CRT}}({{HK}}) then return false end
if {{HK}}()~=nil then return false end
end
if not {{ERR}}() then return false end
for _,v in ipairs({string.byte,string.format,string.find,table.concat,pcall,select,type,error}) do
if not {{CRT}}(v) then return false end
end
local k={{LNF}}({{ENV}})
if type(k)~="number" then return false end
local s="{{CIPHER}}"
local p,q=5381,5381
for i=1,#s do
local x=(s:byte(i)-k-i)%256
p=(p*65599+x)%2147483647
q=(q*131+x)%2147483647
end
return string.format("%08x%08x",p,q)=="{{MARK}}"
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

/// One-based line number of the first prologue line containing `needle`.
///
/// The prologue is always prepended, so a template line is a chunk line. This
/// is what lets the build side predict the checker's own `linedefined`.
fn line_of(needle: &str) -> usize {
    PROLOGUE
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("prologue has no {needle} line"))
        + 1
}

/// Seal `token` under `key`, emitting one Lua decimal escape per byte.
///
/// The runtime replays `(byte - key - i) % 256`, so a key that is off by even
/// one produces a different fingerprint.
fn seal(token: &[u8], key: usize) -> String {
    token
        .iter()
        .enumerate()
        .map(|(i, byte)| format!("\\{:03}", (usize::from(*byte) + key + i + 1) % 256))
        .collect()
}

fn render(names: &[String; NAMES], literal: &str, cipher: &str, mark: &str) -> String {
    PROLOGUE
        .replace("{{LITV}}", literal)
        .replace("{{CIPHER}}", cipher)
        .replace("{{MARK}}", mark)
        .replace("{{ZERO}}", "'\"0\"'")
        .replace("{{LIT}}", &names[0])
        .replace("{{HASH}}", &names[1])
        .replace("{{CHECK}}", &names[2])
        .replace("{{ENV}}", &names[3])
        .replace("{{OK}}", &names[4])
        .replace("{{INFO}}", &names[5])
        .replace("{{GI}}", &names[6])
        .replace("{{HK}}", &names[7])
        .replace("{{FN}}", &names[8])
        .replace("{{CRT}}", &names[9])
        .replace("{{LNF}}", &names[10])
        .replace("{{PRB}}", &names[11])
        .replace("{{ERR}}", &names[12])
}

/// Prefix `source` with the integrity guard and bake in its digest.
pub fn transform(source: &str, rng: &mut impl Rng) -> String {
    let mut taken = Vec::new();
    let names: [String; NAMES] = std::array::from_fn(|_| fresh_name(rng, &mut taken));

    // The marker is random per run: only its sealed form and its fingerprint
    // appear in the output, so there is no plaintext token to patch.
    let token: Vec<u8> = (0..8).map(|_| b'a' + rng.gen_range(0..26u8)).collect();
    let cipher = seal(&token, line_of("local function {{ENV}}("));
    let mark = digest(&token);

    // The digest is taken with the busy literal normalised to "0", which is
    // exactly what the runtime reconstructs before hashing, so the value can
    // live inside the text it covers.
    // Trailing whitespace is normalised away on both sides so an editor that
    // appends a final newline does not look like tampering.
    let provisional = format!("{}{}", render(&names, "\"0\"", &cipher, &mark), source);
    let tag = digest(provisional.trim_end().as_bytes());
    format!(
        "{}{}",
        render(&names, &format!("\"{tag}\""), &cipher, &mark),
        source
    )
}

#[cfg(test)]
mod tests {
    use super::{digest, transform};
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    const PAYLOAD: &str =
        "local total = 0\nfor i = 1, 3 do total = total + i end\nprint('total', total)\n";

    fn lua() -> Option<&'static str> {
        std::process::Command::new("lua5.4")
            .arg("-v")
            .output()
            .ok()
            .map(|_| "lua5.4")
    }

    fn guard(source: &str, seed: u64) -> String {
        transform(source, &mut ChaCha8Rng::seed_from_u64(seed))
    }

    /// Loads the guarded chunk from a host script that has already meddled with
    /// `debug`. Returns `(ran, stderr)`.
    fn run_behind_patch(patch: &str, guarded: &str) -> (bool, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("guarded.lua");
        std::fs::write(&path, guarded).unwrap();
        let host = dir.path().join("host.lua");
        std::fs::write(
            &host,
            format!(
                "{patch}\nlocal c=assert(loadfile([[{}]]))\nc()\nprint('payload ran')\n",
                path.display()
            ),
        )
        .unwrap();
        let out = std::process::Command::new(lua().unwrap())
            .arg(&host)
            .output()
            .unwrap();
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    #[test]
    fn digest_is_stable_and_prefix_sensitive() {
        assert_eq!(digest(b""), "0000150500001505");
        assert_eq!(digest(b"a"), digest(b"a"));
        assert_ne!(digest(b"ab"), digest(b"ba"));
        assert_eq!(digest(b"x").len(), 16);
    }

    #[test]
    fn marker_is_sealed_per_run_and_never_in_clear() {
        let a = guard(PAYLOAD, 1);
        let b = guard(PAYLOAD, 2);
        assert!(a.contains("integrity check failed"));
        assert!(a.ends_with(PAYLOAD));
        assert_ne!(a, b, "guard text is not randomised");
    }

    #[test]
    fn guard_runs_clean_and_catches_edits() {
        let Some(lua) = lua() else { return };
        let guarded = guard(PAYLOAD, 7);
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
        let guarded = guard("print('stdin ok')\n", 11);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stdin.lua");
        std::fs::write(&path, &guarded).unwrap();
        let out = std::process::Command::new(lua).arg(&path).output().unwrap();
        assert!(out.status.success());
        assert_eq!(String::from_utf8_lossy(&out.stdout), "stdin ok\n");
    }

    /// A chunk read from stdin has no file to compare against, so only the
    /// seeded marker stands between a shifted file and a clean run.
    #[test]
    fn line_shift_is_caught_without_the_byte_check() {
        let Some(lua) = lua() else { return };
        let guarded = guard(PAYLOAD, 13);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("piped.lua");

        for (label, text, expected_ok) in [
            ("as written", guarded.clone(), true),
            // A trailing newline is normalised away by the guard.
            ("padded", format!("{guarded}\n"), true),
            // A leading newline shifts every line number by one.
            ("shifted", format!("\n{guarded}"), false),
        ] {
            std::fs::write(&path, text).unwrap();
            let piped = std::process::Command::new(lua)
                .arg("-")
                .stdin(std::fs::File::open(&path).unwrap())
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&piped.stderr);
            if expected_ok {
                assert!(piped.status.success(), "{label} was rejected: {stderr}");
                assert_eq!(String::from_utf8_lossy(&piped.stdout), "total\t6\n");
            } else {
                assert!(!piped.status.success(), "{label} ran anyway");
                assert!(
                    stderr.contains("integrity check failed"),
                    "{label}: {stderr}"
                );
            }
        }
    }

    #[test]
    fn hooked_debug_table_is_rejected() {
        if lua().is_none() {
            return;
        }
        let guarded = guard(PAYLOAD, 17);

        let patches = [
            // A plain Lua forwarder: the reader is reported as a Lua function.
            "local o=debug.getinfo\ndebug.getinfo=function(f,w) return o(f,w) end",
            // A liar that claims to be C when asked about itself: the genuine
            // inner call still raises its argument error through a Lua frame,
            // so the message grows a source:line: prefix.
            "local o=debug.getinfo\nlocal s\ns=function(f,w) if f==s then return {what=\"C\"} end return o(f,w) end\ndebug.getinfo=s",
            // A liar that also hides the diagnostic: no probe then produces the
            // C-shaped error the guard insists on.
            "local o=debug.getinfo\nlocal s\ns=function(f,w) if f==s then return {what=\"C\"} end local ok,r=pcall(o,f,w) if ok then return r end end\ndebug.getinfo=s",
            // A debug hook installed behind our back.
            "debug.sethook(function() end,\"l\")",
            // The whole debug table removed.
            "debug=nil",
        ];

        for patch in patches {
            let (ran, stderr) = run_behind_patch(patch, &guarded);
            assert!(!ran, "chunk ran behind patch: {patch}");
            assert!(
                stderr.contains("integrity check failed"),
                "unexpected stderr for {patch}: {stderr}"
            );
        }
    }

    /// The pristine interpreter must still pass every probe.
    #[test]
    fn untouched_interpreter_passes() {
        if lua().is_none() {
            return;
        }
        let guarded = guard(PAYLOAD, 19);
        let (ran, stderr) = run_behind_patch("", &guarded);
        assert!(ran, "guard rejected a stock interpreter: {stderr}");
    }
}
