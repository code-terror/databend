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

use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use common_exception::Result;
use common_macros::MallocSizeOf;
use common_meta_types::UserInfo;
use futures::channel::oneshot::Sender;
use parking_lot::RwLock;

use crate::sessions::QueryContextShared;
use crate::Config;

#[derive(MallocSizeOf)]
pub struct SessionContext {
    #[ignore_malloc_size_of = "insignificant"]
    conf: Config,
    abort: AtomicBool,
    current_catalog: RwLock<String>,
    current_database: RwLock<String>,
    current_tenant: RwLock<String>,
    #[ignore_malloc_size_of = "insignificant"]
    current_user: RwLock<Option<UserInfo>>,
    #[ignore_malloc_size_of = "insignificant"]
    client_host: RwLock<Option<SocketAddr>>,
    #[ignore_malloc_size_of = "insignificant"]
    io_shutdown_tx: RwLock<Option<Sender<Sender<()>>>>,
    #[ignore_malloc_size_of = "insignificant"]
    query_context_shared: RwLock<Option<Arc<QueryContextShared>>>,
}

impl SessionContext {
    pub fn try_create(conf: Config) -> Result<Self> {
        Ok(SessionContext {
            conf,
            abort: Default::default(),
            current_user: Default::default(),
            current_tenant: Default::default(),
            client_host: Default::default(),
            current_catalog: RwLock::new("default".to_string()),
            current_database: RwLock::new("default".to_string()),
            io_shutdown_tx: Default::default(),
            query_context_shared: Default::default(),
        })
    }

    // Get abort status.
    pub fn get_abort(&self) -> bool {
        self.abort.load(Ordering::Relaxed) as bool
    }

    // Set abort status.
    pub fn set_abort(&self, v: bool) {
        self.abort.store(v, Ordering::Relaxed);
    }

    // Get current catalog name.
    pub fn get_current_catalog(&self) -> String {
        let lock = self.current_catalog.read();
        lock.clone()
    }

    // Set current catalog.
    pub fn set_current_catalog(&self, catalog_name: String) {
        let mut lock = self.current_catalog.write();
        *lock = catalog_name
    }

    // Get current database.
    pub fn get_current_database(&self) -> String {
        let lock = self.current_database.read();
        lock.clone()
    }

    // Set current database.
    pub fn set_current_database(&self, db: String) {
        let mut lock = self.current_database.write();
        *lock = db
    }

    pub fn get_current_tenant(&self) -> String {
        if self.conf.query.management_mode {
            let lock = self.current_tenant.read();
            if !lock.is_empty() {
                return lock.clone();
            }
        }
        self.conf.query.tenant_id.clone()
    }

    pub fn set_current_tenant(&self, tenant: String) {
        let mut lock = self.current_tenant.write();
        *lock = tenant;
    }

    // Get current user
    pub fn get_current_user(&self) -> Option<UserInfo> {
        let lock = self.current_user.read();
        lock.clone()
    }

    // Set the current user after authentication
    pub fn set_current_user(&self, user: UserInfo) {
        let mut lock = self.current_user.write();
        *lock = Some(user);
    }

    pub fn get_client_host(&self) -> Option<SocketAddr> {
        let lock = self.client_host.read();
        *lock
    }

    pub fn set_client_host(&self, sock: Option<SocketAddr>) {
        let mut lock = self.client_host.write();
        *lock = sock
    }

    pub fn set_io_shutdown_tx(&self, tx: Option<Sender<Sender<()>>>) {
        let mut lock = self.io_shutdown_tx.write();
        *lock = tx
    }

    //  Take the io_shutdown_tx and the self.io_shuttdown_tx is None.
    pub fn take_io_shutdown_tx(&self) -> Option<Sender<Sender<()>>> {
        let mut lock = self.io_shutdown_tx.write();
        lock.take()
    }

    pub fn get_current_query_id(&self) -> Option<String> {
        let lock = self.query_context_shared.read();
        lock.as_ref().map(|ctx| ctx.init_query_id.read().clone())
    }

    pub fn get_query_context_shared(&self) -> Option<Arc<QueryContextShared>> {
        let lock = self.query_context_shared.read();
        lock.clone()
    }

    pub fn set_query_context_shared(&self, ctx: Option<Arc<QueryContextShared>>) {
        let mut lock = self.query_context_shared.write();
        *lock = ctx
    }

    //  Take the context_shared.
    pub fn take_query_context_shared(&self) -> Option<Arc<QueryContextShared>> {
        let mut lock = self.query_context_shared.write();
        lock.take()
    }
}
