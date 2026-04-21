use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! define_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        #[allow(missing_docs)]
        pub struct $name(pub String);

        impl $name {
            /// Construct a new ID from any string-like value.
            pub fn new(s: impl Into<String>) -> Self {
                Self(s.into())
            }

            /// Get the underlying string slice.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_string())
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }
    };
}

define_id!(
    /// Unique identifier for an IR node.
    NodeId
);
define_id!(
    /// Unique identifier for a type definition.
    TypeId
);
define_id!(
    /// Unique identifier for a function.
    FuncId
);
define_id!(
    /// Unique identifier for a module.
    ModuleId
);
define_id!(
    /// Symbol (interned string) used for names and identifiers.
    Symbol
);
