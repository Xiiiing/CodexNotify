use crate::error::{CoreError, CoreResult};
use keyring::v1::Entry;

const SERVICE: &str = "com.xiiiing.codex-notify";
pub const BARK_KEY_ACCOUNT: &str = "bark-device-key";
pub const ENCRYPTION_KEY_ACCOUNT: &str = "bark-encryption-key";

fn entry(account: &str) -> CoreResult<Entry> {
    Entry::new(SERVICE, account).map_err(|error| CoreError::Credential(error.to_string()))
}

pub fn set_secret(account: &str, value: &str) -> CoreResult<()> {
    if value.is_empty() {
        return delete_secret(account);
    }
    entry(account)?
        .set_password(value)
        .map_err(|error| CoreError::Credential(error.to_string()))
}

pub fn get_secret(account: &str) -> CoreResult<Option<String>> {
    match entry(account)?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(CoreError::Credential(error.to_string())),
    }
}

pub fn delete_secret(account: &str) -> CoreResult<()> {
    match entry(account)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(CoreError::Credential(error.to_string())),
    }
}
