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

//! Defines structured keys used by ShareApi

use common_meta_app::share::ShareAccountNameIdent;
use common_meta_app::share::ShareId;
use common_meta_app::share::ShareIdToName;
use common_meta_app::share::ShareNameIdent;
use kv_api_key::check_segment;
use kv_api_key::check_segment_absent;
use kv_api_key::check_segment_present;
use kv_api_key::decode_id;
use kv_api_key::escape;
use kv_api_key::unescape;

use crate::kv_api_key;
use crate::KVApiKey;
use crate::KVApiKeyError;
use crate::PREFIX_ID_GEN;

const PREFIX_SHARE: &str = "__fd_share";
const PREFIX_SHARE_ID: &str = "__fd_share_id";
const PREFIX_SHARE_ID_TO_NAME: &str = "__fd_share_id_to_name";
const PREFIX_SHARE_ACCOUNT_ID: &str = "__fd_share_account_id";

/// Key for share id generator
#[derive(Debug)]
pub struct ShareIdGen {}

impl KVApiKey for ShareIdGen {
    const PREFIX: &'static str = PREFIX_ID_GEN;

    fn to_key(&self) -> String {
        format!("{}/share_id", Self::PREFIX)
    }

    fn from_key(_s: &str) -> Result<Self, KVApiKeyError> {
        unimplemented!()
    }
}

/// __fd_share/<tenant>/<share_name> -> <share_id>
impl KVApiKey for ShareNameIdent {
    const PREFIX: &'static str = PREFIX_SHARE;

    fn to_key(&self) -> String {
        format!(
            "{}/{}/{}",
            Self::PREFIX,
            escape(&self.tenant),
            escape(&self.share_name),
        )
    }

    fn from_key(s: &str) -> Result<Self, KVApiKeyError> {
        let mut elts = s.split('/');

        let prefix = check_segment_present(elts.next(), 0, s)?;
        check_segment(prefix, 0, Self::PREFIX)?;

        let tenant = check_segment_present(elts.next(), 1, s)?;

        let share_name = check_segment_present(elts.next(), 2, s)?;

        check_segment_absent(elts.next(), 3, s)?;

        let tenant = unescape(tenant)?;
        let share_name = unescape(share_name)?;

        Ok(ShareNameIdent { tenant, share_name })
    }
}

/// __fd_share_id/<share_id> -> <share_meta>
impl KVApiKey for ShareId {
    const PREFIX: &'static str = PREFIX_SHARE_ID;

    fn to_key(&self) -> String {
        format!("{}/{}", Self::PREFIX, self.share_id)
    }

    fn from_key(s: &str) -> Result<Self, KVApiKeyError> {
        let mut elts = s.split('/');

        let prefix = check_segment_present(elts.next(), 0, s)?;
        check_segment(prefix, 0, Self::PREFIX)?;

        let share_id = decode_id(check_segment_present(elts.next(), 1, s)?)?;

        check_segment_absent(elts.next(), 2, s)?;

        Ok(ShareId { share_id })
    }
}

// __fd_share_account/tenant/id -> ShareAccountMeta
impl KVApiKey for ShareAccountNameIdent {
    const PREFIX: &'static str = PREFIX_SHARE_ACCOUNT_ID;

    fn to_key(&self) -> String {
        format!(
            "{}/{}/{}",
            Self::PREFIX,
            escape(&self.account),
            self.share_id,
        )
    }

    fn from_key(s: &str) -> Result<Self, KVApiKeyError> {
        let mut elts = s.split('/');

        let prefix = check_segment_present(elts.next(), 0, s)?;
        check_segment(prefix, 0, Self::PREFIX)?;

        let account = check_segment_present(elts.next(), 1, s)?;

        let share_id = decode_id(check_segment_present(elts.next(), 2, s)?)?;

        check_segment_absent(elts.next(), 3, s)?;

        let account = unescape(account)?;

        Ok(ShareAccountNameIdent { account, share_id })
    }
}

/// "__fd_share_id_to_name/<share_id> -> ShareNameIdent"
impl KVApiKey for ShareIdToName {
    const PREFIX: &'static str = PREFIX_SHARE_ID_TO_NAME;

    fn to_key(&self) -> String {
        format!("{}/{}", Self::PREFIX, self.share_id,)
    }

    fn from_key(s: &str) -> Result<Self, KVApiKeyError> {
        let mut elts = s.split('/');

        let prefix = check_segment_present(elts.next(), 0, s)?;
        check_segment(prefix, 0, Self::PREFIX)?;

        let share_id = check_segment_present(elts.next(), 1, s)?;
        let share_id = decode_id(share_id)?;

        check_segment_absent(elts.next(), 2, s)?;

        Ok(ShareIdToName { share_id })
    }
}
