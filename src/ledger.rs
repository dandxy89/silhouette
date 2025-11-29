use crate::model::{TxId, TxType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionError {
    MissingAmount,
    InvalidAmount,
    NotStorable(TxType),
    MissingTransaction(TxId),
    AccountLocked,
    InsufficientFunds,
}

impl std::fmt::Display for TransactionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InsufficientFunds => write!(f, "account has InsufficientFunds"),
            Self::AccountLocked => write!(f, "account locked"),
            Self::MissingAmount => write!(f, "missing amount"),
            Self::InvalidAmount => write!(f, "invalid amount"),
            Self::NotStorable(kind) => {
                write!(f, "{kind:?} is not a storable transaction")
            }
            Self::MissingTransaction(tx) => {
                write!(
                    f,
                    "Attempted operation on TxId={tx} was not possible as no existing record exists"
                )
            }
        }
    }
}

impl std::error::Error for TransactionError {}

pub mod client_manager {
    use std::collections::BTreeMap;

    use bigdecimal::BigDecimal;

    use crate::model::ClientId;

    #[derive(Default, Debug, PartialEq, Eq)]
    pub enum ClientAccountStatus {
        #[default]
        Active,
        Locked,
    }

    #[derive(Debug, Default)]
    pub struct ClientAccount {
        pub available: BigDecimal,
        pub held: BigDecimal,
        pub status: ClientAccountStatus,
    }

    impl ClientAccount {
        pub fn total(&self) -> BigDecimal {
            &self.available + &self.held
        }

        pub fn is_locked(&self) -> bool {
            matches!(self.status, ClientAccountStatus::Locked)
        }
    }

    #[derive(Default)]
    pub struct ClientAccountManager {
        accounts: BTreeMap<ClientId, ClientAccount>,
    }

    impl ClientAccountManager {
        pub fn get_or_initialise(&mut self, client: ClientId) -> &mut ClientAccount {
            // A2: If Client doesn't exist simply add a Default record
            self.accounts.entry(client).or_default()
        }

        #[cfg(test)]
        pub fn client_count(&self) -> usize {
            self.accounts.len()
        }
    }

    #[cfg(test)]
    mod test {
        use bigdecimal::num_traits::zero;

        use crate::ledger::client_manager::{ClientAccountManager, ClientAccountStatus};

        #[test]
        fn test_get_or_initialise() {
            let mut manager = ClientAccountManager::default();
            let test_client = 1;

            let account_state = manager.get_or_initialise(test_client);

            assert_eq!(account_state.available, zero());
            assert_eq!(account_state.held, zero());
            assert_eq!(account_state.status, ClientAccountStatus::Active);
            assert_eq!(account_state.total(), zero());
        }
    }
}

pub mod tx_manager {
    use std::collections::{BTreeMap, btree_map::Entry};

    use bigdecimal::{BigDecimal, num_traits::zero};

    use crate::{
        ledger::TransactionError,
        model::{CSVRecord, ClientId, TxId, TxType},
    };

    #[derive(Debug, PartialEq, Eq)]
    pub enum TransactionStatus {
        Processed,
        Disputed,
        Resolved,
        Chargedback,
    }

    pub struct Transaction {
        pub tx: TxId,
        pub client: ClientId,
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
                    }),
                    None => Err(TransactionError::MissingAmount),
                },
                _ => Err(TransactionError::NotStorable(value.r#type)),
            }
        }
    }

    #[derive(Default)]
    pub struct TxManager {
        transactions: BTreeMap<TxId, Transaction>,
    }

    impl TxManager {
        pub fn store(&mut self, transaction: Transaction) -> &Transaction {
            let transaction = self
                .transactions
                .entry(transaction.tx)
                .or_insert(transaction);

            &*transaction
        }

        pub fn get(&self, tx: TxId) -> Option<&Transaction> {
            self.transactions.get(&tx)
        }

        pub fn set_status(
            &mut self,
            tx: TxId,
            status: TransactionStatus,
        ) -> Result<(), TransactionError> {
            if let Entry::Occupied(mut e) = self.transactions.entry(tx) {
                e.get_mut().status = status;
                Ok(())
            } else {
                Err(TransactionError::MissingTransaction(tx))
            }
        }

        #[cfg(test)]
        pub fn is_disputed(&self, tx: TxId) -> bool {
            self.transactions
                .get(&tx)
                .is_some_and(|tx| tx.status == TransactionStatus::Disputed)
        }

        #[cfg(test)]
        pub fn tx_count(&self) -> usize {
            self.transactions.len()
        }
    }

    #[cfg(test)]
    mod test {
        use bigdecimal::{BigDecimal, FromPrimitive as _};

        use crate::{
            ledger::{
                TransactionError,
                tx_manager::{Transaction, TransactionStatus, TxManager},
            },
            model::{CSVRecord, TxType},
        };

        #[test]
        fn test_tx_manager_handles_storage_correctly() {
            let mut manager = TxManager::default();

            let valid_record = CSVRecord {
                r#type: TxType::Deposit,
                client: 1,
                tx: 1,
                amount: BigDecimal::from_f32(1.1),
            };
            let valid_record = Transaction::try_from(valid_record).unwrap();
            manager.store(valid_record);

            manager.set_status(1, TransactionStatus::Disputed).unwrap();
            assert!(manager.is_disputed(1));

            let invalid_record = CSVRecord {
                r#type: TxType::Deposit,
                client: 1,
                tx: 2,
                amount: None,
            };

            let tx = Transaction::try_from(invalid_record);
            assert!(matches!(tx, Err(TransactionError::MissingAmount)));
        }
    }
}

pub mod engine {
    use bigdecimal::num_traits::zero;

    use crate::{
        ledger::{
            TransactionError,
            client_manager::{ClientAccountManager, ClientAccountStatus},
            tx_manager::{Transaction, TransactionStatus, TxManager},
        },
        model::{CSVRecord, TxType},
    };

    #[derive(Default)]
    pub struct PaymentsEngine {
        pub client_manager: ClientAccountManager,
        pub tx_manager: TxManager,
    }

    impl PaymentsEngine {
        fn process_deposit(&mut self, record: CSVRecord) -> Result<(), TransactionError> {
            let account = self.client_manager.get_or_initialise(record.client);
            if account.is_locked() {
                return Err(TransactionError::AccountLocked);
            }

            let tx = Transaction::try_from(record)?;

            account.available += &tx.amount;
            self.tx_manager.store(tx);

            Ok(())
        }

        fn process_withdrawal(&mut self, record: CSVRecord) -> Result<(), TransactionError> {
            let account = self.client_manager.get_or_initialise(record.client);
            if &account.available < record.amount.as_ref().unwrap_or(&zero()) {
                return Err(TransactionError::InsufficientFunds);
            }

            let tx = Transaction::try_from(record)?;

            account.available -= &tx.amount;
            self.tx_manager.store(tx);

            Ok(())
        }

        fn process_dispute(&mut self, record: CSVRecord) -> Result<(), TransactionError> {
            let Some(transaction) = self.tx_manager.get(record.tx) else {
                return Err(TransactionError::MissingTransaction(record.tx));
            };

            if !transaction.can_be_disputed(&record) {
                return Ok(());
            }

            let account = self.client_manager.get_or_initialise(record.client);

            account.available -= &transaction.amount;
            account.held += &transaction.amount;

            self.tx_manager
                .set_status(transaction.tx, TransactionStatus::Disputed)
        }

        fn process_resolve(&mut self, record: CSVRecord) -> Result<(), TransactionError> {
            let Some(transaction) = self.tx_manager.get(record.tx) else {
                return Err(TransactionError::MissingTransaction(record.tx));
            };

            if !transaction.is_disputed() {
                return Ok(());
            }

            let account = self.client_manager.get_or_initialise(record.client);

            account.available += &transaction.amount;
            account.held -= &transaction.amount;

            self.tx_manager
                .set_status(transaction.tx, TransactionStatus::Resolved)
        }

        fn process_chargeback(&mut self, record: CSVRecord) -> Result<(), TransactionError> {
            let account = self.client_manager.get_or_initialise(record.client);

            account.status = ClientAccountStatus::Locked;

            let Some(transaction) = self.tx_manager.get(record.tx) else {
                return Err(TransactionError::MissingTransaction(record.tx));
            };

            account.held -= &transaction.amount;

            self.tx_manager
                .set_status(transaction.tx, TransactionStatus::Chargedback)
        }

        pub fn process_csv_record(&mut self, record: CSVRecord) -> Result<(), TransactionError> {
            match record.r#type {
                TxType::Deposit => self.process_deposit(record),
                TxType::Withdrawal => self.process_withdrawal(record),
                TxType::Dispute => self.process_dispute(record),
                TxType::Resolve => self.process_resolve(record),
                TxType::Chargeback => self.process_chargeback(record),
            }
        }
    }

    #[cfg(test)]
    mod test {
        use bigdecimal::{BigDecimal, FromPrimitive as _};

        use crate::{
            ledger::engine::PaymentsEngine,
            model::{CSVRecord, TxType},
        };

        #[test]
        fn test_deposits_and_withdrawls() {
            let mut payment_engine = PaymentsEngine::default();

            let valid_deposit = CSVRecord {
                r#type: TxType::Deposit,
                client: 1,
                tx: 1,
                amount: BigDecimal::from_f32(1.1),
            };
            let valid_withdraw = CSVRecord {
                r#type: TxType::Withdrawal,
                client: 1,
                tx: 2,
                amount: BigDecimal::from_f32(1.1),
            };

            payment_engine.process_csv_record(valid_deposit).unwrap();
            payment_engine.process_csv_record(valid_withdraw).unwrap();

            assert_eq!(2, payment_engine.tx_manager.tx_count());
            assert_eq!(1, payment_engine.client_manager.client_count());
        }
    }
}
