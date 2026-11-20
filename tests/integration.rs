use assert_cmd::Command;
use std::fs;

#[test]
fn cli_transforms_and_matches_lua_behavior() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.lua");
    let output = dir.path().join("out.lua");
    let source = "local base = 3\nlocal function make(x) local captured=x+base; return function(y) return captured+y end end\nlocal f=make(4)\nif f(5)>10 then print('ok', f(1)) end\n";
    fs::write(&input, source).unwrap();
    Command::cargo_bin("lexify-obfuscator")
        .unwrap()
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--level")
        .arg("extreme")
        .assert()
        .success();
    let code = fs::read_to_string(&output).unwrap();
    assert!(!code.contains("string.char"));
    assert!(!code.contains("loadstring"));
    assert!(!code.contains("load("));
    assert_ne!(code, source);
    let original = dir.path().join("original.lua");
    fs::write(&original, source).unwrap();
    if let Ok(a) = std::process::Command::new("lua5.4").arg(&original).output() {
        let b = std::process::Command::new("lua5.4")
            .arg(&output)
            .output()
            .unwrap();
        assert_eq!(a.stdout, b.stdout);
        assert_eq!(a.status.success(), b.status.success());
    }
}

#[test]
fn ast_splitting_and_randomized_shared_lookup_preserve_fixture_output() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/strings.lua");
    let input = dir.path().join("strings.lua");
    let output_a = dir.path().join("out-a.lua");
    let output_b = dir.path().join("out-b.lua");
    let output_extreme = dir.path().join("extreme.lua");
    fs::copy(fixture, &input).unwrap();
    for output in [&output_a, &output_b] {
        Command::cargo_bin("lexify-obfuscator")
            .unwrap()
            .arg(&input)
            .arg("-o")
            .arg(output)
            .args([
                "--split-strings",
                "--lookup-tables",
                "--randomize-lookup",
                "--shared-lookup",
                "--split-size",
                "3",
            ])
            .assert()
            .success();
    }
    Command::cargo_bin("lexify-obfuscator")
        .unwrap()
        .arg(&input)
        .arg("-o")
        .arg(&output_extreme)
        .args(["--level", "extreme"])
        .assert()
        .success();
    let generated = fs::read(&output_a).unwrap();
    assert_ne!(
        generated,
        fs::read(&output_b).unwrap(),
        "unseeded runs should draw fresh randomness"
    );
    let code = String::from_utf8(generated).unwrap();
    assert!(!code.contains("string.char"));
    assert!(!code.contains("loadstring"));
    assert!(!code.contains("load("));
    let Ok(original) = std::process::Command::new("lua5.4").arg(&input).output() else {
        return;
    };
    let generated = std::process::Command::new("lua5.4")
        .arg(&output_a)
        .output()
        .unwrap();
    let extreme = std::process::Command::new("lua5.4")
        .arg(&output_extreme)
        .output()
        .unwrap();
    assert!(original.status.success());
    assert!(generated.status.success());
    assert!(extreme.status.success());
    assert_eq!(original.stdout, generated.stdout);
    assert_eq!(original.stdout, extreme.stdout);
}

#[test]
fn cli_rejects_invalid_lua() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("bad.lua");
    fs::write(&input, "local = broken").unwrap();
    Command::cargo_bin("lexify-obfuscator")
        .unwrap()
        .arg(input)
        .assert()
        .failure();
}

const VM_SOURCE: &str = "local function fact(n)\n  if n <= 1 then return 1 end\n  return n * fact(n - 1)\nend\nlocal total = 0\nfor i = 1, 5 do total = total + fact(i) end\nlocal function double(x) return x * 2 end\nlocal parts = {}\nfor i = 1, 4 do parts[#parts + 1] = tostring(i) end\nprint(total, double(3), table.concat(parts, '-'), select('#', ...))\n";

#[test]
fn vm_and_wrapper_preserve_behavior() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("vm.lua");
    fs::write(&input, VM_SOURCE).unwrap();
    let Ok(original) = std::process::Command::new("lua5.4").arg(&input).output() else {
        return;
    };
    assert!(original.status.success());

    let mut shapes = std::collections::HashSet::new();
    for (index, level) in ["basic", "extreme", "extreme"].iter().enumerate() {
        let output = dir.path().join(format!("vm-{index}.lua"));
        let assert = Command::cargo_bin("lexify-obfuscator")
            .unwrap()
            .arg(&input)
            .arg("-o")
            .arg(&output)
            .args(["--level", level, "--vm", "--wrapper"])
            .assert()
            .success();
        let stderr = String::from_utf8_lossy(&assert.get_output().stderr).to_string();
        assert!(stderr.contains("warning"), "missing VM warning: {stderr}");

        let code = fs::read_to_string(&output).unwrap();
        assert!(code.contains("while"), "no dispatcher in output: {code}");
        assert!(code.contains("0x"), "no opcodes in output: {code}");
        let generated = std::process::Command::new("lua5.4")
            .arg(&output)
            .output()
            .unwrap();
        assert_eq!(
            original.stdout, generated.stdout,
            "level {level} changed behaviour:\n{code}"
        );
        assert!(generated.status.success());
        shapes.insert(code);
    }
    assert!(
        shapes.len() > 1,
        "unseeded runs should draw fresh layouts and wrappers"
    );
}

#[test]
fn load_identifiers_need_the_opt_in() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("load.lua");
    let output = dir.path().join("load-out.lua");
    let source = "local f = loadstring('return 1 + 1')\nlocal g = load('return 2 + 2')\nprint(f and f(), g and g())\n";
    fs::write(&input, source).unwrap();

    for identifier in ["loadstring", "load"] {
        let assert = Command::cargo_bin("lexify-obfuscator")
            .unwrap()
            .arg(&input)
            .assert()
            .failure();
        let stderr = String::from_utf8_lossy(&assert.get_output().stderr).to_string();
        assert!(
            stderr.contains(&format!("contains forbidden identifier {identifier}")),
            "unexpected error for {identifier}: {stderr}"
        );
    }

    Command::cargo_bin("lexify-obfuscator")
        .unwrap()
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .args(["--loadstring", "--rename"])
        .assert()
        .success();
    let code = fs::read_to_string(&output).unwrap();
    assert!(code.contains("loadstring"), "{code}");

    if let Ok(original) = std::process::Command::new("lua5.4").arg(&input).output() {
        let generated = std::process::Command::new("lua5.4")
            .arg(&output)
            .output()
            .unwrap();
        assert_eq!(original.stdout, generated.stdout);
    }
}

const GUARD_SOURCE: &str = "local function label(name)\n  return name:upper() .. '!'\nend\nlocal rows = {}\nfor i = 1, 3 do rows[i] = label('row' .. i) end\nprint(table.concat(rows, ','), #rows)\n";

#[test]
fn antitamper_is_opt_in_and_rejects_edits() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("guard.lua");
    fs::write(&input, GUARD_SOURCE).unwrap();

    // Every level leaves it off, so the guard is only present when asked for.
    for level in ["basic", "medium", "strong", "extreme"] {
        let output = dir.path().join(format!("plain-{level}.lua"));
        Command::cargo_bin("lexify-obfuscator")
            .unwrap()
            .arg(&input)
            .arg("-o")
            .arg(&output)
            .args(["--level", level])
            .assert()
            .success();
        let code = fs::read_to_string(&output).unwrap();
        assert!(
            !code.contains("integrity check failed"),
            "level {level} enabled the guard"
        );
    }

    let guarded = dir.path().join("guarded.lua");
    Command::cargo_bin("lexify-obfuscator")
        .unwrap()
        .arg(&input)
        .arg("-o")
        .arg(&guarded)
        .args(["--level", "extreme", "--antitamper"])
        .assert()
        .success();
    let code = fs::read_to_string(&guarded).unwrap();
    assert!(code.contains("integrity check failed"), "{code}");
    assert!(!code.contains("string.char"));

    let Ok(original) = std::process::Command::new("lua5.4").arg(&input).output() else {
        return;
    };
    let generated = std::process::Command::new("lua5.4")
        .arg(&guarded)
        .output()
        .unwrap();
    assert!(generated.status.success(), "guard rejected its own output");
    assert_eq!(original.stdout, generated.stdout);

    // A trailing newline is tolerated, but an edit inside the chunk is not.
    let padded = dir.path().join("padded.lua");
    fs::write(&padded, format!("{code}\n\n")).unwrap();
    assert!(std::process::Command::new("lua5.4")
        .arg(&padded)
        .output()
        .unwrap()
        .status
        .success());

    let tampered = dir.path().join("tampered.lua");
    fs::write(&tampered, code.replacen("do\n", "do\n\n", 1)).unwrap();
    let rejected = std::process::Command::new("lua5.4")
        .arg(&tampered)
        .output()
        .unwrap();
    assert!(!rejected.status.success(), "edited file still ran");
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("integrity check failed"));

    // A debug hook is an environment change the guard also refuses.
    let hooked = std::process::Command::new("lua5.4")
        .arg("-e")
        .arg("debug.sethook(function() end, '', 1000)")
        .arg(&guarded)
        .output()
        .unwrap();
    assert!(!hooked.status.success(), "guard ignored an installed hook");
}
