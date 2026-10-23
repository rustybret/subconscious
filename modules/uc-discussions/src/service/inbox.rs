#![forbid(unsafe_code)]

use chrono::{SecondsFormat, Utc};
use cortexkit_role_agent_inbox::{
    neutralize,
    post::{Content, PostReply, PostRequest},
    read::{Item, ItemState, ListReply, ListRequest, ReadReply, ReadRequest},
    withdraw::{WithdrawReply, WithdrawRequest, WithdrawState},
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use uuid::Uuid;

use crate::{service::ServiceError, Storage};

#[derive(Clone)]
pub struct InboxService {
    storage: Storage,
}

impl InboxService {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    pub fn post(&self, req: PostRequest) -> Result<PostReply, ServiceError> {
        req.validate()
            .map_err(|e| ServiceError::InvalidRequest(e.message.clone()))?;

        let target_agent = req
            .notice
            .target_agent
            .clone()
            .unwrap_or_else(|| "default".to_owned());

        let (scope_ref, scope_epoch) = req
            .scope
            .as_ref()
            .map(|s| (Some(s.scope_ref.clone()), Some(s.epoch as i64)))
            .unwrap_or((None, None));
        let flow_id = req.flow_id.clone();

        let attention = serde_json::to_string(&req.notice.attention)
            .unwrap_or_else(|_| "\"silent\"".to_owned())
            .trim_matches('"')
            .to_owned();

        let raw_text = match &req.notice.content {
            Content::Text(t) => t.text.clone(),
            Content::Template(tmpl) => serde_json::to_string(&tmpl.template.slots).unwrap_or_default(),
        };
        let rendered_body = neutralize(&raw_text);

        let mut connection = self.storage.lock_connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

        let existing: Option<(String, String)> = transaction
            .query_row(
                "SELECT item_id, state FROM agent_inbox_items WHERE target_agent = ?1 AND idempotency_key = ?2",
                params![target_agent, req.notice.idempotency_key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((item_id, _state)) = existing {
            transaction.commit()?;
            return Ok(PostReply::accepted(item_id).with_replayed(true));
        }

        if let Some(replace_key) = req.notice.replace_key.as_deref() {
            transaction.execute(
                "UPDATE agent_inbox_items SET state = 'retired', updated_at = ?1
                 WHERE target_agent = ?2 AND replace_key = ?3 AND state = 'pending'",
                params![now_timestamp(), target_agent, replace_key],
            )?;
        }

        let item_id = format!("item-{}", Uuid::new_v4());
        let data_json = req
            .notice
            .data
            .as_ref()
            .and_then(|d| serde_json::to_string(d).ok());

        let now = now_timestamp();
        transaction.execute(
            "INSERT INTO agent_inbox_items (
                item_id, idempotency_key, replace_key, scope_ref, scope_epoch, flow_id,
                target_agent, attention, state, body, data_json, deliver_after_ms,
                expires_at_ms, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pending', ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                item_id,
                req.notice.idempotency_key,
                req.notice.replace_key,
                scope_ref,
                scope_epoch,
                flow_id,
                target_agent,
                attention,
                rendered_body,
                data_json,
                req.notice.deliver_after_ms.map(|t| t as i64),
                req.notice.expires_at_ms.map(|t| t as i64),
                now,
                now
            ],
        )?;
        transaction.commit()?;

        Ok(PostReply::accepted(item_id))
    }

    pub fn withdraw(&self, req: WithdrawRequest) -> Result<WithdrawReply, ServiceError> {
        let mut connection = self.storage.lock_connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

        let (where_clause, key_param) = match &req {
            WithdrawRequest::Idempotency(k) => ("idempotency_key = ?1", &k.idempotency_key),
            WithdrawRequest::Replace(k) => ("replace_key = ?1", &k.replace_key),
        };

        let query = format!(
            "SELECT item_id, state FROM agent_inbox_items WHERE {where_clause} ORDER BY rowid DESC LIMIT 1"
        );
        let row: Option<(String, String)> = transaction
            .query_row(&query, params![key_param], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;

        let Some((item_id, state)) = row else {
            return Ok(WithdrawReply::new(WithdrawState::NotFound));
        };

        if state == "pending" {
            transaction.execute(
                "UPDATE agent_inbox_items SET state = 'retired', updated_at = ?1 WHERE item_id = ?2",
                params![now_timestamp(), item_id],
            )?;
            transaction.commit()?;
            Ok(WithdrawReply::new(WithdrawState::Withdrawn))
        } else {
            Ok(WithdrawReply::new(WithdrawState::AlreadyDelivered))
        }
    }

    pub fn list(&self, _req: ListRequest) -> Result<ListReply, ServiceError> {
        let connection = self.storage.lock_connection()?;
        let mut statement = connection.prepare(
            "SELECT item_id, state, body, data_json FROM agent_inbox_items
             WHERE state != 'retired' ORDER BY rowid ASC",
        )?;
        let items = statement
            .query_map([], |row| {
                let item_id: String = row.get(0)?;
                let state_str: String = row.get(1)?;
                let body: String = row.get(2)?;
                let data_json: Option<String> = row.get(3)?;
                let state = match state_str.as_str() {
                    "pending" => ItemState::Pending,
                    "in_flight" => ItemState::InFlight,
                    "landed" => ItemState::Landed,
                    "unconfirmed" => ItemState::Unconfirmed,
                    _ => ItemState::Retired,
                };
                let mut item = Item::new(item_id, state, body);
                if let Some(json_str) = data_json {
                    if let Ok(data_val) = serde_json::from_str(&json_str) {
                        item = item.with_data(data_val);
                    }
                }
                Ok(item)
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(ServiceError::from)?;
        Ok(ListReply::new(items))
    }

    pub fn read(&self, req: ReadRequest) -> Result<ReadReply, ServiceError> {
        let connection = self.storage.lock_connection()?;
        let row: Option<(String, String, Option<String>)> = connection
            .query_row(
                "SELECT state, body, data_json FROM agent_inbox_items WHERE item_id = ?1",
                params![req.item_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let item = row.map(|(state_str, body, data_json)| {
            let state = match state_str.as_str() {
                "pending" => ItemState::Pending,
                "in_flight" => ItemState::InFlight,
                "landed" => ItemState::Landed,
                "unconfirmed" => ItemState::Unconfirmed,
                _ => ItemState::Retired,
            };
            let mut item = Item::new(req.item_id, state, body);
            if let Some(json_str) = data_json {
                if let Ok(data_val) = serde_json::from_str(&json_str) {
                    item = item.with_data(data_val);
                }
            }
            item
        });
        Ok(ReadReply::new(item))
    }
}

fn now_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Nanos, true)
}

#[cfg(test)]
pub(crate) mod tests {
    use cortexkit_role_agent_inbox::post::{Attention, Content, Notice, PostRequest, Scope};
    use cortexkit_role_agent_inbox::read::{ListRequest, ReadRequest};
    use cortexkit_role_agent_inbox::withdraw::WithdrawRequest;

    use super::*;

    #[test]
    fn inbox_post_idempotency_and_read_lifecycle() {
        let storage = Storage::open_in_memory().expect("open storage");
        let service = InboxService::new(storage);

        let notice = Notice::new(
            "key-1",
            Content::text("Hello Agent"),
            Attention::Silent,
        );
        let req = PostRequest::new(Scope::new("scope-1", 1), notice);
        let post_res = service.post(req.clone()).expect("post notice");
        let item_id = post_res.item_id.expect("item_id returned");
        assert_eq!(post_res.replayed, None);

        // Replay returns same item_id with replayed=true
        let replay_res = service.post(req).expect("replay post");
        assert_eq!(replay_res.item_id.as_deref(), Some(item_id.as_str()));
        assert_eq!(replay_res.replayed, Some(true));

        // Read item
        let read_res = service.read(ReadRequest::new(&item_id)).expect("read notice");
        let item = read_res.item.expect("item found");
        assert_eq!(item.item_id, item_id);
        assert!(item.body.contains("Hello Agent"));

        // List items
        let list_res = service.list(ListRequest::default()).expect("list items");
        assert_eq!(list_res.items.len(), 1);

        // Withdraw item
        let withdraw_res = service
            .withdraw(WithdrawRequest::idempotency("key-1"))
            .expect("withdraw notice");
        assert_eq!(withdraw_res.state.as_str(), "withdrawn");

        // After withdraw, list does not include it
        let list_after = service.list(ListRequest::default()).expect("list items");
        assert_eq!(list_after.items.len(), 0);
    }
}
