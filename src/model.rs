use bigdecimal::BigDecimal;

pub type ClientId = u16; // A1
pub type TxId = u32;

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum TxType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(serde::Deserialize, Debug)]
pub struct CSVRecord {
    pub r#type: TxType,
    pub client: ClientId,
    pub tx: TxId,
    pub amount: Option<BigDecimal>,
}
