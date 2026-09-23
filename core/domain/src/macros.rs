macro_rules! string_type {
    ($name:ident, $validator:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);
        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
                let value = value.into();
                $validator(&value)?;
                Ok(Self(value))
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl std::str::FromStr for $name {
            type Err = DomainError;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}
macro_rules! integer_type {
    ($name:ident, $ty:ty, $min:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name($ty);
        impl $name {
            pub fn new(value: $ty) -> Result<Self, DomainError> {
                if !($min..=<$ty>::MAX).contains(&value) {
                    return Err(DomainError::InvalidInteger {
                        primitive: stringify!($name),
                    });
                }
                Ok(Self(value))
            }
            pub const fn get(self) -> $ty {
                self.0
            }
        }
        impl TryFrom<$ty> for $name {
            type Error = DomainError;
            fn try_from(value: $ty) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }
    };
}
macro_rules! canonical_enum {
    ($name:ident { $($variant:ident => $wire:literal,)* }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name { $($variant,)* }
        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)*];
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $wire,)* }
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}
macro_rules! canonical_fsm {
    ($name:ident { $($from:ident => $to:ident,)* }) => {
        impl $name {
            pub const TRANSITIONS: &'static [(Self, Self)] = &[$((Self::$from, Self::$to),)*];
            pub fn is_valid_transition(self, to: Self) -> bool {
                Self::TRANSITIONS.contains(&(self, to))
            }
            pub fn validate_transition(self, to: Self) -> Result<(), DomainError> {
                if self.is_valid_transition(to) { Ok(()) } else {
                    Err(DomainError::InvalidStateTransition {
                        machine: stringify!($name), from: self.as_str(), to: to.as_str(),
                    })
                }
            }
        }
    };
}
macro_rules! domain_struct {
    ($name:ident { $($field:ident: $ty:ty,)* }) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name { $(pub(crate) $field: $ty,)* }
        impl $name {
            // One argument per canonical field; no parallel handwritten field inventory.
            #[allow(clippy::too_many_arguments)]
            pub fn new($($field: $ty,)*) -> Result<Self, DomainError> {
                let value = Self { $($field,)* };
                value.validate()?;
                Ok(value)
            }
            $(pub fn $field(&self) -> &$ty { &self.$field })*
        }
    };
}
