use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// Effects tracked in the AIRL IR type system.
///
/// Effects describe the side effects a function or expression may have.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Effect {
    /// No side effects. Pure functions cannot call non-pure functions.
    Pure,
    /// Reads from a named resource (e.g. `"fs"`, `"net"`).
    Read {
        /// Name of the resource being read.
        resource: String,
    },
    /// Writes to a named resource (e.g. `"fs"`, `"net"`, `"stdout"`).
    Write {
        /// Name of the resource being written.
        resource: String,
    },
    /// Allocates memory on the heap.
    Allocate,
    /// General I/O (catch-all; subsumes `Read` and `Write`).
    IO,
    /// Can fail with a named error type.
    Fail {
        /// Name of the error type (e.g. `"IOError"`, `"ParseError"`).
        error_type: String,
    },
    /// May not terminate (used for infinite loops).
    Diverge,
}

impl Effect {
    /// Parse an effect from a JSON string representation.
    ///
    /// Simple effects like "Pure", "IO", "Allocate", "Diverge" are parsed directly.
    /// Parameterized effects like "Read(memory)", "Write(file)", "Fail(IOError)"
    /// are also supported.
    pub fn from_effect_str(s: &str) -> Effect {
        let s = s.trim();
        match s {
            "Pure" => Effect::Pure,
            "IO" => Effect::IO,
            "Allocate" => Effect::Allocate,
            "Diverge" => Effect::Diverge,
            _ => {
                if let Some(inner) = strip_parens(s, "Read") {
                    Effect::Read {
                        resource: inner.to_string(),
                    }
                } else if let Some(inner) = strip_parens(s, "Write") {
                    Effect::Write {
                        resource: inner.to_string(),
                    }
                } else if let Some(inner) = strip_parens(s, "Fail") {
                    Effect::Fail {
                        error_type: inner.to_string(),
                    }
                } else {
                    // Treat unrecognized as IO for forward compatibility
                    Effect::IO
                }
            }
        }
    }

    /// Convert an effect to its string representation.
    pub fn to_effect_str(&self) -> String {
        match self {
            Effect::Pure => "Pure".into(),
            Effect::IO => "IO".into(),
            Effect::Allocate => "Allocate".into(),
            Effect::Diverge => "Diverge".into(),
            Effect::Read { resource } => format!("Read({resource})"),
            Effect::Write { resource } => format!("Write({resource})"),
            Effect::Fail { error_type } => format!("Fail({error_type})"),
        }
    }
}

fn strip_parens<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if let Some(rest) = s.strip_prefix(prefix) {
        if rest.starts_with('(') && rest.ends_with(')') {
            Some(&rest[1..rest.len() - 1])
        } else {
            None
        }
    } else {
        None
    }
}

impl Serialize for Effect {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_effect_str())
    }
}

impl<'de> Deserialize<'de> for Effect {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Effect::from_effect_str(&s))
    }
}

impl fmt::Display for Effect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_effect_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_effects() {
        assert_eq!(Effect::from_effect_str("Pure"), Effect::Pure);
        assert_eq!(Effect::from_effect_str("IO"), Effect::IO);
        assert_eq!(Effect::from_effect_str("Allocate"), Effect::Allocate);
        assert_eq!(Effect::from_effect_str("Diverge"), Effect::Diverge);
    }

    #[test]
    fn test_parameterized_effects() {
        assert_eq!(
            Effect::from_effect_str("Read(memory)"),
            Effect::Read {
                resource: "memory".into()
            }
        );
        assert_eq!(
            Effect::from_effect_str("Fail(IOError)"),
            Effect::Fail {
                error_type: "IOError".into()
            }
        );
    }

    #[test]
    fn test_serde_roundtrip() {
        let effects = vec![
            Effect::Pure,
            Effect::IO,
            Effect::Read {
                resource: "fs".into(),
            },
        ];
        for e in effects {
            let json = serde_json::to_string(&e).unwrap();
            let parsed: Effect = serde_json::from_str(&json).unwrap();
            assert_eq!(e, parsed);
        }
    }
}
