use molecule::prelude::Entity;
use serde::{de::Error, Deserialize, Deserializer, Serializer};
use serde_with::{serde_conv, DeserializeAs, SerializeAs};

pub fn from_hex<'de, D, E>(deserializer: D) -> Result<E, D::Error>
where
    D: Deserializer<'de>,
    E: TryFrom<Vec<u8>>,
    E::Error: core::fmt::Debug,
{
    String::deserialize(deserializer)
        .and_then(|string| {
            if string.len() < 2 || &string[..2].to_lowercase() != "0x" {
                return Err(Error::custom("hex string should start with 0x"));
            };
            hex::decode(&string[2..])
                .map_err(|err| Error::custom(format!("failed to decode hex: {:?}", err)))
        })
        .and_then(|vec| {
            vec.try_into().map_err(|err| {
                Error::custom(format!("failed to convert vector into type: {:?}", err))
            })
        })
}

pub fn to_hex<E, S>(e: E, serializer: S) -> Result<S::Ok, S::Error>
where
    E: AsRef<[u8]>,
    S: Serializer,
{
    serializer.serialize_str(&format!("0x{}", &hex::encode(e.as_ref())))
}

pub struct SliceHex;

impl<T> SerializeAs<T> for SliceHex
where
    T: AsRef<[u8]>,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        to_hex(source, serializer)
    }
}

impl<'de, T> DeserializeAs<'de, T> for SliceHex
where
    T: TryFrom<Vec<u8>>,
    T::Error: core::fmt::Debug,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        from_hex(deserializer)
    }
}

pub struct EntityHex;

impl<T> SerializeAs<T> for EntityHex
where
    T: Entity,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        to_hex(source.as_slice(), serializer)
    }
}

impl<'de, T> DeserializeAs<'de, T> for EntityHex
where
    T: Entity,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v: Vec<u8> = from_hex(deserializer)?;
        T::from_slice(&v).map_err(Error::custom)
    }
}

macro_rules! uint_as_hex {
    ($name:ident, $ty:ty) => {
        serde_conv!(
            pub $name,
            $ty,
            |u: &$ty| format!("0x{:x}", u),
            |hex: &str| -> Result<$ty, String> {
                let bytes = hex.as_bytes();
                if bytes.len() < 3 || &bytes[..2] != b"0x" {
                    return Err("hex string should start with 0x".to_string());
                }
                if bytes.len() > 3 && &bytes[2..3] == b"0" {
                    return Err("hex string should not start with redundant leading zeros".to_string());
                };
                <$ty>::from_str_radix(&hex[2..], 16)
                    .map_err(|err| format!("failed to parse hex: {:?}", err))
            }
        );
    };
}

uint_as_hex!(U128Hex, u128);
uint_as_hex!(U64Hex, u64);
uint_as_hex!(U32Hex, u32);
uint_as_hex!(U16Hex, u16);

#[cfg(test)]
mod tests {
    use super::*;
    use ckb_types::packed::Script;
    use ckb_types::prelude::*;
    use serde::Serialize;
    use serde_with::serde_as;

    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Foo {
        #[serde_as(as = "SliceHex")]
        slice: [u8; 4],
        #[serde_as(as = "Option<EntityHex>")]
        enity: Option<Script>,
        #[serde_as(as = "U128Hex")]
        bar_128: u128,
        #[serde_as(as = "U64Hex")]
        bar_64: u64,
        #[serde_as(as = "U32Hex")]
        bar_32: u32,
        #[serde_as(as = "U16Hex")]
        bar_16: u16,
    }

    #[test]
    fn test_serde_utils() {
        let foo = Foo {
            slice: [1, 2, 3, 4],
            enity: Some(Script::new_builder().build()),
            bar_128: 0xdeadbeef,
            bar_64: 0x123,
            bar_32: 0x10,
            bar_16: 0xa,
        };

        let json = r#"{"slice":"0x01020304","enity":"0x3500000010000000300000003100000000000000000000000000000000000000000000000000000000000000000000000000000000","bar_128":"0xdeadbeef","bar_64":"0x123","bar_32":"0x10","bar_16":"0xa"}"#;
        assert_eq!(serde_json::to_string(&foo).unwrap(), json);
        assert_eq!(serde_json::from_str::<Foo>(json).unwrap(), foo);
    }
}
