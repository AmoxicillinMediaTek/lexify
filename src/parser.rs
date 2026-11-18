use crate::ast::Parsed;
/// Parse Lua/Luau source using full_moon and surface parser errors.
pub fn parse(source: &str) -> Result<Parsed, String> {
    let version=full_moon::ast::LuaVersion::new();
    full_moon::parse_fallible(source,version).into_result().map(|ast|Parsed{_ast:ast}).map_err(|errors|errors.into_iter().map(|e|format!("{e:#?}")).collect::<Vec<_>>().join("; "))
}
