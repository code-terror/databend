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

use std::sync::Arc;

use common_base::base::ProgressValues;
pub use common_catalog::table_context::ProcessInfo;
use common_contexts::DalMetrics;

use crate::sessions::Session;
use crate::sessions::SessionContext;
use crate::sessions::SessionType;

impl Session {
    pub fn process_info(self: &Arc<Self>) -> ProcessInfo {
        let session_ctx = self.session_ctx.clone();
        self.to_process_info(&session_ctx)
    }

    fn to_process_info(self: &Arc<Self>, status: &SessionContext) -> ProcessInfo {
        let mut memory_usage = 0;

        if let Some(shared) = &status.get_query_context_shared() {
            if let Ok(runtime) = shared.try_get_runtime() {
                let runtime_tracker = runtime.get_tracker();
                let runtime_memory_tracker = runtime_tracker.get_memory_tracker();
                memory_usage = runtime_memory_tracker.get_memory_usage();
            }
        }

        ProcessInfo {
            id: self.id.clone(),
            typ: self.get_type().to_string(),
            state: self.process_state(status),
            database: status.get_current_database(),
            user: status.get_current_user(),
            settings: self.get_settings(),
            client_address: status.get_client_host(),
            session_extra_info: self.process_extra_info(status),
            memory_usage,
            dal_metrics: Session::query_dal_metrics(status),
            scan_progress_value: Session::query_scan_progress_value(status),
            mysql_connection_id: self.mysql_connection_id,
        }
    }

    fn process_state(self: &Arc<Self>, status: &SessionContext) -> String {
        match status.get_query_context_shared() {
            _ if status.get_abort() => String::from("Aborting"),
            None => String::from("Idle"),
            Some(_) => String::from("Query"),
        }
    }

    fn process_extra_info(self: &Arc<Self>, status: &SessionContext) -> Option<String> {
        match self.get_type() {
            SessionType::FlightRPC => Session::rpc_extra_info(status),
            _ => Session::query_extra_info(status),
        }
    }

    fn rpc_extra_info(status: &SessionContext) -> Option<String> {
        status
            .get_query_context_shared()
            .map(|_| String::from("Partial cluster query stage"))
    }

    fn query_extra_info(status: &SessionContext) -> Option<String> {
        status
            .get_query_context_shared()
            .as_ref()
            .map(|context_shared| context_shared.get_query_str())
    }

    fn query_dal_metrics(status: &SessionContext) -> Option<DalMetrics> {
        status
            .get_query_context_shared()
            .as_ref()
            .map(|context_shared| context_shared.dal_ctx.get_metrics().as_ref().clone())
    }

    fn query_scan_progress_value(status: &SessionContext) -> Option<ProgressValues> {
        status
            .get_query_context_shared()
            .as_ref()
            .map(|context_shared| context_shared.scan_progress.get_values())
    }
}
