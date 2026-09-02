use crate::bark;
use crate::error::CoreResult;
use crate::security::{get_secret, BARK_KEY_ACCOUNT, ENCRYPTION_KEY_ACCOUNT};
use crate::{AppSettings, EventStore, Notification};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchResult {
    pub id: i64,
    pub sent: bool,
    pub error: String,
}

pub fn dispatch_due(
    store: &EventStore,
    settings: &AppSettings,
    limit: u32,
) -> CoreResult<Vec<DispatchResult>> {
    let Some(bark_key) = get_secret(BARK_KEY_ACCOUNT)? else {
        return Ok(vec![]);
    };
    let encryption_key = if settings.encryption_enabled {
        get_secret(ENCRYPTION_KEY_ACCOUNT)?
    } else {
        None
    };
    let mut results = vec![];
    for record in store.claim_due(limit)? {
        let mut notification: Notification = serde_json::from_str(&record.payload_json)?;
        if notification.bark_id.is_empty() {
            notification.bark_id = record.event_key.clone();
        }
        match bark::send(
            &notification,
            settings,
            &bark_key,
            encryption_key.as_deref(),
        ) {
            Ok(_) => {
                store.mark_sent(record.id)?;
                results.push(DispatchResult {
                    id: record.id,
                    sent: true,
                    error: String::new(),
                });
            }
            Err(error) => {
                let message = error.to_string();
                store.mark_failed(record.id, &message, settings.retry_limit)?;
                results.push(DispatchResult {
                    id: record.id,
                    sent: false,
                    error: message,
                });
            }
        }
    }
    Ok(results)
}
