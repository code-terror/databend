// Copyright 2021 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use async_trait::async_trait;
use common_meta_api::KVApi;
use common_meta_types::AppliedState;
use common_meta_types::Cmd;
use common_meta_types::GetKVReply;
use common_meta_types::GetKVReq;
use common_meta_types::ListKVReply;
use common_meta_types::ListKVReq;
use common_meta_types::LogEntry;
use common_meta_types::MGetKVReply;
use common_meta_types::MGetKVReq;
use common_meta_types::MetaError;
use common_meta_types::MetaResultError;
use common_meta_types::TxnReply;
use common_meta_types::TxnRequest;
use common_meta_types::UpsertKVReply;
use common_meta_types::UpsertKVReq;
use tracing::debug;

use crate::meta_service::MetaNode;

/// Impl KVApi for MetaNode.
///
/// Write through raft-log.
/// Read through local state machine, which may not be consistent.
/// E.g. Read is not guaranteed to see a write.
#[async_trait]
impl KVApi for MetaNode {
    async fn upsert_kv(&self, act: UpsertKVReq) -> Result<UpsertKVReply, MetaError> {
        let ent = LogEntry {
            txid: None,
            cmd: Cmd::UpsertKV {
                key: act.key,
                seq: act.seq,
                value: act.value,
                value_meta: act.value_meta,
            },
        };
        let rst = self.write(ent).await?;

        match rst {
            AppliedState::KV(x) => Ok(x),
            _ => Err(MetaError::MetaResultError(MetaResultError::InvalidType {
                expect: "AppliedState::KV".to_string(),
                got: "other".to_string(),
            })),
        }
    }

    #[tracing::instrument(level = "debug", skip(self))]
    async fn get_kv(&self, key: &str) -> Result<GetKVReply, MetaError> {
        let res = self
            .consistent_read(GetKVReq {
                key: key.to_string(),
            })
            .await?;

        Ok(res)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    async fn mget_kv(&self, keys: &[String]) -> Result<MGetKVReply, MetaError> {
        let res = self
            .consistent_read(MGetKVReq {
                keys: keys.to_vec(),
            })
            .await?;

        Ok(res)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    async fn prefix_list_kv(&self, prefix: &str) -> Result<ListKVReply, MetaError> {
        let res = self
            .consistent_read(ListKVReq {
                prefix: prefix.to_string(),
            })
            .await?;

        Ok(res)
    }

    #[tracing::instrument(level = "debug", skip(self, txn))]
    async fn transaction(&self, txn: TxnRequest) -> Result<TxnReply, MetaError> {
        debug!(txn = display(&txn), "MetaNode::transaction()");
        let ent = LogEntry {
            txid: None,
            cmd: Cmd::Transaction(txn),
        };
        let rst = self.write(ent).await?;

        match rst {
            AppliedState::TxnReply(x) => Ok(x),
            _ => Err(MetaError::MetaResultError(MetaResultError::InvalidType {
                expect: "AppliedState::transaction".to_string(),
                got: "other".to_string(),
            })),
        }
    }
}
