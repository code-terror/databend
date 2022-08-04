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

#[allow(clippy::module_inception)]
mod share;

pub use share::AddShareAccountReply;
pub use share::AddShareAccountReq;
pub use share::CreateShareReply;
pub use share::CreateShareReq;
pub use share::DropShareReply;
pub use share::DropShareReq;
pub use share::GetShareGrantObjectReply;
pub use share::GetShareGrantObjectReq;
pub use share::GrantShareObjectReply;
pub use share::GrantShareObjectReq;
pub use share::RemoveShareAccountReply;
pub use share::RemoveShareAccountReq;
pub use share::RevokeShareObjectReply;
pub use share::RevokeShareObjectReq;
pub use share::ShareAccountMeta;
pub use share::ShareAccountNameIdent;
pub use share::ShareGrantEntry;
pub use share::ShareGrantObject;
pub use share::ShareGrantObjectName;
pub use share::ShareGrantObjectPrivilege;
pub use share::ShareGrantObjectSeqAndId;
pub use share::ShareId;
pub use share::ShareIdToName;
pub use share::ShareIdent;
pub use share::ShareInfo;
pub use share::ShareMeta;
pub use share::ShareNameIdent;
pub use share::ShowShareReply;
pub use share::ShowShareReq;
