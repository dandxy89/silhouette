pub mod client_manager {
    use std::collections::BTreeMap;

    use bigdecimal::{BigDecimal, num_traits::zero};

    use crate::model::ClientId;

    #[derive(Default, Debug, PartialEq, Eq)]
    pub enum ClientAccountStatus {
        #[default]
        Active,
        Locked,
    }

    #[derive(Debug)]
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

    impl Default for ClientAccount {
        fn default() -> Self {
            Self {
                available: zero(),
                held: zero(),
                status: ClientAccountStatus::default(),
            }
        }
    }

    #[derive(Default)]
    pub struct ClientAccountManager {
        pub(crate) accounts: BTreeMap<ClientId, ClientAccount>,
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

mod tx_manager {
    use std::collections::{BTreeMap, btree_map::Entry};

    use crate::{
        model::TxId,
        transaction::{Transaction, TransactionError, TransactionStatus, TxResult},
    };

    #[derive(Default)]
    pub struct TxManager {
        transactions: BTreeMap<TxId, Transaction>,
    }

    impl TxManager {
        pub fn insert(&mut self, transaction: Transaction) -> &Transaction {
            let transaction = self
                .transactions
                .entry(transaction.tx)
                .or_insert(transaction);

            &*transaction
        }

        pub fn exists(&self, tx: TxId) -> bool {
            self.transactions.contains_key(&tx)
        }

        pub fn get(&self, tx: TxId) -> Option<&Transaction> {
            self.transactions.get(&tx)
        }

        pub fn set_status(&mut self, tx: TxId, status: TransactionStatus) -> TxResult {
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
            ledger::tx_manager::{Transaction, TransactionStatus, TxManager},
            model::{CSVRecord, TxType},
            transaction::TransactionError,
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
            manager.insert(valid_record);

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
            client_manager::{ClientAccountManager, ClientAccountStatus},
            tx_manager::TxManager,
        },
        model::{CSVRecord, TxType},
        transaction::{Transaction, TransactionError, TransactionStatus, TxResult},
    };

    #[derive(Default)]
    pub struct PaymentsEngine {
        pub client_manager: ClientAccountManager,
        pub tx_manager: TxManager,
    }

    impl PaymentsEngine {
        fn process_deposit(&mut self, record: CSVRecord) -> TxResult {
            if self.tx_manager.exists(record.tx) {
                return Err(TransactionError::DuplicateTransactionId(record.tx));
            }

            let account = self.client_manager.get_or_initialise(record.client);
            if account.is_locked() {
                return Err(TransactionError::AccountLocked);
            }

            let tx = Transaction::try_from(record)?;
            account.available += &tx.amount;
            self.tx_manager.insert(tx);

            Ok(())
        }

        fn process_withdrawal(&mut self, record: CSVRecord) -> TxResult {
            if self.tx_manager.exists(record.tx) {
                return Err(TransactionError::DuplicateTransactionId(record.tx));
            }

            let account = self.client_manager.get_or_initialise(record.client);
            if account.is_locked() {
                return Err(TransactionError::AccountLocked);
            }

            if &account.available < record.amount.as_ref().unwrap_or(&zero()) {
                return Err(TransactionError::InsufficientFunds);
            }

            let tx = Transaction::try_from(record)?;
            account.available -= &tx.amount;
            self.tx_manager.insert(tx);

            Ok(())
        }

        fn process_dispute(&mut self, record: CSVRecord) -> TxResult {
            let Some(transaction) = self.tx_manager.get(record.tx) else {
                return Err(TransactionError::MissingTransaction(record.tx));
            };
            if transaction.client != record.client {
                return Err(TransactionError::InvalidClinetId);
            }
            if transaction.r#type != TxType::Deposit || !transaction.can_be_disputed(&record) {
                return Ok(());
            }

            let account = self.client_manager.get_or_initialise(record.client);
            account.available -= &transaction.amount;
            account.held += &transaction.amount;

            self.tx_manager
                .set_status(transaction.tx, TransactionStatus::Disputed)
        }

        fn process_resolve(&mut self, record: CSVRecord) -> TxResult {
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

        fn process_chargeback(&mut self, record: CSVRecord) -> TxResult {
            let Some(transaction) = self.tx_manager.get(record.tx) else {
                return Err(TransactionError::MissingTransaction(record.tx));
            };
            if transaction.client != record.client {
                return Err(TransactionError::InvalidClinetId);
            }
            if !transaction.is_disputed() || transaction.r#type != TxType::Deposit {
                return Ok(());
            }

            let account = self.client_manager.get_or_initialise(record.client);
            account.status = ClientAccountStatus::Locked;
            account.held -= &transaction.amount;

            self.tx_manager
                .set_status(transaction.tx, TransactionStatus::Chargedback)
        }

        pub fn process_csv_record(&mut self, record: CSVRecord) -> TxResult {
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
        use bigdecimal::{BigDecimal, FromPrimitive as _, num_traits::zero};

        use crate::{
            file_reader::csv_stream,
            ledger::engine::PaymentsEngine,
            model::{CSVRecord, TxType},
            transaction::TransactionError,
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

        #[test]
        fn test_withdrawal_with_insufficient_funds() {
            let test_data = r#" type,  client,  tx,  amount
deposit,  1,  1,  100.0
withdrawal,  1,  2,  200.0
"#;
            let mut payment_engine = PaymentsEngine::default();
            for (idx, record) in csv_stream(test_data.as_bytes()).enumerate() {
                let record = record.unwrap();

                let result = payment_engine.process_csv_record(record);
                if idx == 1 {
                    assert!(matches!(result, Err(TransactionError::InsufficientFunds)));
                }
            }

            let expected = BigDecimal::from_f32(100.0).unwrap();
            let total = payment_engine.client_manager.get_or_initialise(1).total();
            assert_eq!(total, expected);
        }

        #[test]
        fn test_resolve_dsputed_transaction() {
            let test_data = r#" type,  client,  tx,  amount
deposit,1,1,100.0
dispute,1,1,
dispute,1,1,
resolve,1,1,
dispute,1,2,
dispute,1,1,
resolve,1,1,
resolve,1,1,
"#;

            let mut payment_engine = PaymentsEngine::default();
            for (idx, record) in csv_stream(test_data.as_bytes()).enumerate() {
                let result = payment_engine.process_csv_record(record.unwrap());

                if idx == 4 {
                    assert!(result.is_err());
                    assert!(matches!(
                        result.unwrap_err(),
                        TransactionError::MissingTransaction(_)
                    ))
                } else {
                    assert!(result.is_ok());
                }
            }

            let expected = BigDecimal::from_f32(100.0).unwrap();
            let total = payment_engine.client_manager.get_or_initialise(1).total();
            assert_eq!(total, expected);
        }

        #[test]
        fn test_chargeback() {
            let test_data = r#" type,  client,  tx,  amount
deposit,1,1,100.0
dispute,1,1,
chargeback,1,1,
deposit,1,2,100.0
"#;

            let mut payment_engine = PaymentsEngine::default();
            for (idx, record) in csv_stream(test_data.as_bytes()).enumerate() {
                let result = payment_engine.process_csv_record(record.unwrap());

                if idx == 3 {
                    assert!(result.is_err());
                    assert!(matches!(
                        result.unwrap_err(),
                        TransactionError::AccountLocked
                    ))
                } else {
                    assert!(result.is_ok());
                }
            }

            let account = payment_engine.client_manager.get_or_initialise(1);

            let is_locked = account.is_locked();
            assert!(is_locked);

            let total = account.total();
            assert_eq!(total, zero());
        }

        #[test]
        fn test_non_matching_client_ids() {
            let test_data = r#" type,  client,  tx,  amount
deposit,1,1,100.0
dispute,2,1,
resolve,1,1,
"#;

            let mut payment_engine = PaymentsEngine::default();
            for record in csv_stream(test_data.as_bytes()) {
                let _ = payment_engine.process_csv_record(record.unwrap());
            }

            let is_disputed = payment_engine.tx_manager.is_disputed(1);
            assert!(!is_disputed);
        }

        #[test]
        fn should_not_allow_duplicate_transactions() {
            let test_data = r#" type,  client,  tx,  amount
deposit,1,1,100.0
deposit,1,1,100.0
"#;

            let mut payment_engine = PaymentsEngine::default();
            for record in csv_stream(test_data.as_bytes()) {
                let _ = payment_engine.process_csv_record(record.unwrap());
            }

            let expected = BigDecimal::from_f32(100.0).unwrap();
            let total = payment_engine.client_manager.get_or_initialise(1).total();
            assert_eq!(total, expected);
        }

        #[test]
        fn should_not_deposit_or_withdraws_if_locked() {
            let test_data = r#" type,  client,  tx,  amount
chargeback,1,1,
deposit,1,1,100.0
withdrawal,1,2,100.0
"#;

            let mut payment_engine = PaymentsEngine::default();
            for record in csv_stream(test_data.as_bytes()) {
                let _ = payment_engine.process_csv_record(record.unwrap());
            }

            let account = payment_engine.client_manager.get_or_initialise(1);
            assert!(!account.is_locked());
        }
    }
}
