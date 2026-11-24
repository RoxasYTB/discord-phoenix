//! Serde models for Discord objects. All structs ignore unknown fields and
//! every integer enum has an `Unknown(_)` fallback so new API values never
//! break deserialization.

/// Declares an integer-backed enum with an `Unknown` catch-all variant.
macro_rules! int_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident : $repr:ty {
            $( $(#[$vmeta:meta])* $variant:ident = $value:expr ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )*
            Unknown($repr),
        }

        impl $name {
            pub fn value(self) -> $repr {
                match self {
                    $( Self::$variant => $value, )*
                    Self::Unknown(v) => v,
                }
            }
        }

        impl From<$repr> for $name {
            fn from(v: $repr) -> Self {
                match v {
                    $( x if x == $value => Self::$variant, )*
                    other => Self::Unknown(other),
                }
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                serde::Serialize::serialize(&self.value(), s)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                <$repr as serde::Deserialize>::deserialize(d).map(Self::from)
            }
        }
    };
}

mod channel;
mod component;
mod guild;
mod member;
mod message;
mod permissions;
mod role;
mod snowflake;
mod soundboard;
mod user;
mod voice;

pub use channel::*;
pub use component::*;
pub use guild::*;
pub use member::*;
pub use message::*;
pub use permissions::*;
pub use role::*;
pub use snowflake::*;
pub use soundboard::*;
pub use user::*;
pub use voice::*;

/// Deserialize `null` as the type's default.
pub(crate) fn null_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + serde::Deserialize<'de>,
{
    use serde::Deserialize;
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}
