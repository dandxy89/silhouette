use bigdecimal::{BigDecimal, RoundingMode};
use serde::{Deserialize, Deserializer, de::Error};

pub type ClientId = u16; // A1
pub type TxId = u32;

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
