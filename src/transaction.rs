use bigdecimal::{BigDecimal, num_traits::zero};

use crate::model::{CSVRecord, ClientId, TxId, TxType};

pub type TxResult = Result<(), TransactionError>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransactionError {
    #[error("client ID is invalid")]
    InvalidClinetId,
    #[error("account has insufficient funds")]
    InsufficientFunds,
    #[error("account locked")]
    AccountLocked,
    #[error("missing amount")]
    MissingAmount,
    #[error("invalid amount")]
    InvalidAmount,
    #[error("{0:?} is not a storable transaction")]
    NotStorable(TxType),
    #[error("attempted operation on TxId={0} was not possible as no existing record exists")]
    MissingTransaction(TxId),
    #[error("duplicate transaction")]
    DuplicateTransactionId(TxId),
}

#[derive(Debug, PartialEq, Eq)]
pub enum TransactionStatus {
    Processed,
    Disputed,
    Resolved,
    Chargedback,
}

#[derive(Debug)]
pub struct Transaction {
    pub tx: TxId,
    pub client: ClientId,
    pub r#type: TxType,
    pub amount: BigDecimal,
    pub status: TransactionStatus,
}

impl Transaction {
    pub fn can_be_disputed(&self, record: &CSVRecord) -> bool {
        if self.client != record.client {
            return false;
        }

        matches!(
            self.status,
            TransactionStatus::Processed | TransactionStatus::Resolved
        )
    }

    pub fn is_disputed(&self) -> bool {
        matches!(self.status, TransactionStatus::Disputed)
    }
}

impl TryFrom<CSVRecord> for Transaction {
    type Error = TransactionError;

    fn try_from(value: CSVRecord) -> Result<Self, Self::Error> {
        match value.r#type {
            TxType::Deposit | TxType::Withdrawal => match value.amount {
                Some(amount) if amount < zero() => Err(TransactionError::InvalidAmount),
                Some(amount) => Ok(Transaction {
                    tx: value.tx,
                    client: value.client,
                    amount,
                    status: TransactionStatus::Processed,
                    r#type: value.r#type,
                }),
                None => Err(TransactionError::MissingAmount),
            },
            _ => Err(TransactionError::NotStorable(value.r#type)),
        }
    }
}
