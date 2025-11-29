use std::io;

use bigdecimal::{BigDecimal, RoundingMode};
use serde::{Serialize, Serializer};

use crate::{ledger::client_manager::ClientAccountManager, model::ClientId};

fn serialise_decimal<S>(decimal: &BigDecimal, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let rounded = decimal.with_scale_round(4, RoundingMode::HalfEven);
    serializer.serialize_str(&rounded.to_string())
}

#[allow(dead_code)]
#[derive(Serialize)]
pub struct OutputRecord {
    pub client: ClientId,
    #[serde(serialize_with = "serialise_decimal")]
    pub available: BigDecimal,
    #[serde(serialize_with = "serialise_decimal")]
    pub held: BigDecimal,
    #[serde(serialize_with = "serialise_decimal")]
    pub total: BigDecimal,
    pub locked: bool,
}

pub fn write_accounts_to_stdout(clients: &ClientAccountManager) -> Result<(), csv::Error> {
    let stdout = io::stdout().lock();
    let mut csv_wtr = csv::WriterBuilder::new()
        .has_headers(true)
        .from_writer(stdout);

    let iter = clients
        .accounts
        .iter()
        .map(move |(client, account)| OutputRecord {
            client: *client,
            available: account.available.clone(),
            held: account.held.clone(),
            total: account.total(),
            locked: account.is_locked(),
        });

    for account in iter {
        csv_wtr.serialize(account)?;
    }

    csv_wtr.flush()?;

    Ok(())
}
