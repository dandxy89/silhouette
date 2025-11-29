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
        pub avaliable: BigDecimal,
        pub held: BigDecimal,
        pub state: ClientAccountStatus,
    }

    impl ClientAccount {
        pub fn total(&self) -> BigDecimal {
            &self.avaliable + &self.held
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

            assert_eq!(account_state.avaliable, zero());
            assert_eq!(account_state.held, zero());
            assert_eq!(account_state.state, ClientAccountStatus::Active);
            assert_eq!(account_state.total(), zero());
        }
    }
}

pub mod tx_manager {
    use std::collections::{BTreeMap, btree_map::Entry};

    use bigdecimal::{BigDecimal, num_traits::zero};

    use crate::model::{CSVRecord, TxId, TxType};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum TransactionError {
        MissingAmount,
        InvalidAmount,
        NotStorable(TxType),
        MissingTransatcion(TxId),
    }

    impl std::fmt::Display for TransactionError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::MissingAmount => write!(f, "missing amount"),
                Self::InvalidAmount => write!(f, "invalid amount"),
                Self::NotStorable(kind) => {
                    write!(f, "{kind:?} is not a storable transaction")
                }
                Self::MissingTransatcion(tx) => {
                    write!(
                        f,
                        "Attempted operation on TxId={tx} was not possible as no existing record exists"
                    )
                }
            }
        }
    }

    impl std::error::Error for TransactionError {}

    #[derive(Debug, PartialEq, Eq)]
    pub enum TransactionStatus {
        Processed,
        Disputed,
        Resolved,
        Chargedback,
    }

    pub struct Transaction {
        pub tx: TxId,
        pub amount: BigDecimal,
        pub status: TransactionStatus,
    }

    impl TryFrom<CSVRecord> for Transaction {
        type Error = TransactionError;

        fn try_from(value: CSVRecord) -> Result<Self, Self::Error> {
            match value.r#type {
                TxType::Deposit | TxType::Withdrawal => match value.amount {
                    Some(amount) if amount < zero() => Err(TransactionError::InvalidAmount),
                    Some(amount) => Ok(Transaction {
                        tx: value.tx,
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
        pub fn store(&mut self, record: CSVRecord) -> Result<(), TransactionError> {
            let storable = Transaction::try_from(record)?;
            self.transactions.entry(storable.tx).or_insert(storable);
            Ok(())
        }

        pub fn exists(&self, tx: TxId) -> bool {
            self.transactions.contains_key(&tx)
        }

        pub fn is_disputed(&self, tx: TxId) -> bool {
            self.transactions
                .get(&tx)
                .is_some_and(|tx| tx.status == TransactionStatus::Disputed)
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
                Err(TransactionError::MissingTransatcion(tx))
            }
        }
    }

    #[cfg(test)]
    mod test {
        use bigdecimal::{BigDecimal, FromPrimitive};

        use crate::{
            ledger::tx_manager::{TransactionError, TransactionStatus, TxManager},
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
            assert!(manager.store(valid_record).is_ok());

            manager.set_status(1, TransactionStatus::Disputed).unwrap();
            assert!(manager.is_disputed(1));

            let invalid_record = CSVRecord {
                r#type: TxType::Deposit,
                client: 1,
                tx: 2,
                amount: None,
            };
            match manager.store(invalid_record) {
                Err(TransactionError::MissingAmount) => (),
                _ => panic!("Incorrect Error status"),
            }
        }
    }
}
