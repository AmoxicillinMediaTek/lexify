use assert_cmd::Command;
use std::fs;

#[test]
fn cli_transforms_and_matches_lua_behavior() {
    let dir=tempfile::tempdir().unwrap();
    let input=dir.path().join("in.lua"); let output=dir.path().join("out.lua");
    let source="local base = 3\nlocal function make(x) local captured=x+base; return function(y) return captured+y end end\nlocal f=make(4)\nif f(5)>10 then print('ok', f(1)) end\n";
    fs::write(&input,source).unwrap();
    Command::cargo_bin("lexify-obfuscator").unwrap().arg(&input).arg("-o").arg(&output).arg("--level").arg("extreme").arg("--seed").arg("7").assert().success();
    let code=fs::read_to_string(&output).unwrap();
    assert!(!code.contains("string.char")); assert!(!code.contains("loadstring")); assert!(!code.contains("load(")); assert!(code.contains("table.concat"));
    let original=dir.path().join("original.lua"); fs::write(&original,source).unwrap();
    if let Ok(a)=std::process::Command::new("lua").arg(original).output(){
        let b=std::process::Command::new("lua").arg(&output).output().unwrap();
        assert_eq!(a.stdout,b.stdout); assert_eq!(a.status.success(),b.status.success());
    }
}

#[test]
fn cli_rejects_invalid_lua() {
    let dir=tempfile::tempdir().unwrap(); let input=dir.path().join("bad.lua");
    fs::write(&input,"local = broken").unwrap();
    Command::cargo_bin("lexify-obfuscator").unwrap().arg(input).assert().failure();
}
