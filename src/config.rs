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
            split_strings: false,
            lookup_tables: false,
            randomize_lookup: false,
            shared_lookup: false,
            split_size: 4,
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
