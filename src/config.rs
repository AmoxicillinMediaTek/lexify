#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(target_arch = "wasm32"), derive(clap::ValueEnum))]
pub enum Level {
    Basic,
    Medium,
    Strong,
    Extreme,
}
#[derive(Clone, Debug)]
pub struct Config {
    pub level: Level,
    pub minify: bool,
    pub rename: bool,
    pub encrypt_strings: bool,
    pub obfuscate_numbers: bool,
    pub obfuscate_booleans: bool,
    pub obfuscate_tables: bool,
    pub dead_code: bool,
    pub control_flow: bool,
    /// Flatten eligible statement blocks into a state-variable dispatcher.
    /// Opt-in only; no level preset enables it.
    pub cff: bool,
    /// AST-backed literal splitting (replaces raw literals with fragment concatenation).
    pub split_strings: bool,
    /// Emit fragment tables and index into them instead of inlining fragments.
    pub lookup_tables: bool,
    /// Random table keys and chunk boundaries instead of sequential keys/fixed chunks.
    pub randomize_lookup: bool,
    /// Put every fragment in one shared table rather than one table per literal.
    pub shared_lookup: bool,
    /// Maximum bytes per fragment when splitting.
    pub split_size: usize,
    /// Lower eligible blocks into a program-counter dispatch loop (a VM-shaped layout).
    /// Never enabled by a level preset: it is a deterrent, not a correctness boundary.
    pub vm: bool,
    /// Run the finished chunk inside a randomly chosen loader facade.
    pub wrapper: bool,
    /// Run the chunk with a proxy `_ENV` / `_G` that forwards global access
    /// while hiding the real global table behind a protected metatable.
    pub envwrap: bool,
    /// Allow `load`/`loadstring` in the output. The guard stays on otherwise.
    pub allow_loadstring: bool,
    /// Inject a runtime guard that fingerprints its own file and checks the
    /// environment. Opt-in only: no level preset turns it on.
    pub antitamper: bool,
    pub target: String,
    /// Pins the RNG so a transformation can be reproduced byte for byte.
    /// `None`, the default, seeds from fresh entropy instead, so repeated runs
    /// of the same input differ.
    pub seed: Option<u64>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            level: Level::Basic,
            minify: false,
            rename: false,
            encrypt_strings: false,
            obfuscate_numbers: false,
            obfuscate_booleans: false,
            obfuscate_tables: false,
            dead_code: false,
            control_flow: false,
            cff: false,
            split_strings: false,
            lookup_tables: false,
            randomize_lookup: false,
            shared_lookup: false,
            split_size: 4,
            vm: false,
            wrapper: false,
            envwrap: false,
            allow_loadstring: false,
            antitamper: false,
            target: "lua54".into(),
            seed: None,
        }
    }
}

impl Config {
    /// Level presets. Each level enables everything the levels below it enable.
    pub fn apply_level(&mut self) {
        match self.level {
            Level::Basic => {}
            Level::Medium => {
                self.minify = true;
                self.rename = true;
            }
            Level::Strong => {
                self.minify = true;
                self.rename = true;
                self.obfuscate_numbers = true;
                self.obfuscate_booleans = true;
                self.obfuscate_tables = true;
                self.control_flow = true;
            }
            Level::Extreme => {
                self.minify = true;
                self.rename = true;
                self.obfuscate_numbers = true;
                self.obfuscate_booleans = true;
                self.obfuscate_tables = true;
                self.control_flow = true;
                self.dead_code = true;
                self.encrypt_strings = true;
                // Extreme uses AST-backed literal splitting with randomized shared lookup tables.
                self.split_strings = true;
                self.lookup_tables = true;
                self.randomize_lookup = true;
                self.shared_lookup = true;
            }
        }
        if self.split_strings {
            self.split_size = self.split_size.max(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_are_cumulative() {
        let mut cfg = Config {
            level: Level::Medium,
            ..Config::default()
        };
        cfg.apply_level();
        assert!(cfg.minify && cfg.rename);
        assert!(!cfg.obfuscate_numbers && !cfg.dead_code);

        let mut cfg = Config {
            level: Level::Strong,
            ..Config::default()
        };
        cfg.apply_level();
        assert!(cfg.rename && cfg.control_flow);
        assert!(!cfg.dead_code && !cfg.split_strings);

        let mut cfg = Config {
            level: Level::Extreme,
            ..Config::default()
        };
        cfg.apply_level();
        assert!(cfg.dead_code && cfg.split_strings && cfg.lookup_tables);
        assert!(cfg.randomize_lookup && cfg.shared_lookup);
        assert!(!cfg.envwrap, "level presets must not enable env wrapping");
        assert!(
            !cfg.cff,
            "level presets must not enable control-flow flattening"
        );
    }

    #[test]
    fn cff_is_opt_in_by_default() {
        assert!(!Config::default().cff);
        for level in [Level::Basic, Level::Medium, Level::Strong, Level::Extreme] {
            let mut cfg = Config {
                level,
                ..Config::default()
            };
            cfg.apply_level();
            assert!(!cfg.cff);
        }
    }

    #[test]
    fn envwrap_is_opt_in_by_default() {
        assert!(!Config::default().envwrap);
        for level in [Level::Basic, Level::Medium, Level::Strong, Level::Extreme] {
            let mut cfg = Config {
                level,
                ..Config::default()
            };
            cfg.apply_level();
            assert!(!cfg.envwrap);
        }
    }

    #[test]
    fn split_size_never_zero() {
        let mut cfg = Config {
            level: Level::Basic,
            split_strings: true,
            split_size: 0,
            ..Config::default()
        };
        cfg.apply_level();
        assert_eq!(cfg.split_size, 1);
    }
}
