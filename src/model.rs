use std::fmt;

use bigdecimal::{BigDecimal, RoundingMode};
use serde::{Deserialize, Deserializer, Serialize, de::Error};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClientId(pub u16);

impl fmt::Display for ClientId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u16> for ClientId {
    fn from(value: u16) -> Self {
        ClientId(value)
    }
}

impl From<ClientId> for u16 {
    fn from(value: ClientId) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TxId(pub u32);

impl fmt::Display for TxId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u32> for TxId {
    fn from(value: u32) -> Self {
        TxId(value)
    }
}

impl From<TxId> for u32 {
    fn from(value: TxId) -> Self {
        value.0
    }
}

#[derive(Clone, serde::Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TxType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

pub fn deserialize_decimal<'de, D>(deserializer: D) -> Result<Option<BigDecimal>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<String>::deserialize(deserializer)? {
        Some(string) => {
            let string = string.trim();
            if string.is_empty() {
                return Ok(None);
            }

            let d = BigDecimal::parse_bytes(string.as_bytes(), 10)
                .ok_or_else(|| Error::custom(format!("Unable to parse to BigDecimal: {string}")))?;

            Ok(Some(d.with_scale_round(4, RoundingMode::HalfEven)))
        }
        None => Ok(None),
    }
}

#[derive(serde::Deserialize, Debug)]
pub struct CSVRecord {
    pub r#type: TxType,
    pub client: ClientId,
    pub tx: TxId,
    #[serde(default, deserialize_with = "deserialize_decimal")]
    pub amount: Option<BigDecimal>,
}
