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
        .arg("--seed")
        .arg("7")
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
                "--seed",
                "1",
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
        .args(["--seed", "1", "--level", "extreme"])
        .assert()
        .success();
    let generated = fs::read(&output_a).unwrap();
    assert_eq!(
        generated,
        fs::read(&output_b).unwrap(),
        "seeded output should be deterministic"
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
