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

use std::fmt::Display;

use anyerror::AnyError;
use common_exception::ErrorCode;
use serde::Deserialize;
use serde::Serialize;
use sled::transaction::UnabortableTransactionError;
use thiserror::Error;

use crate::error_context::ErrorWithContext;
use crate::MatchSeq;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, thiserror::Error)]
pub enum MetaStorageError {
    // type to represent bytes format errors
    #[error("{0}")]
    BytesError(String),

    // type to represent serialize/deserialize errors
    #[error("{0}")]
    SerdeError(String),

    /// An AnyError built from sled::Error.
    #[error(transparent)]
    SledError(AnyError),

    #[error(transparent)]
    Damaged(AnyError),

    /// Error that is related to snapshot
    #[error(transparent)]
    SnapshotError(AnyError),

    /// An internal error that inform txn to retry.
    #[error("Conflict when execute transaction, just retry")]
    TransactionConflict,

    /// An application error that cause transaction to abort.
    #[error("{0}")]
    AppError(#[from] AppError),
}

/// Output message for end users, with sensitive info stripped.
pub trait AppErrorMessage: Display {
    fn message(&self) -> String {
        self.to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, thiserror::Error)]
#[error("DatabaseAlreadyExists: `{db_name}` while `{context}`")]
pub struct DatabaseAlreadyExists {
    db_name: String,
    context: String,
}

impl DatabaseAlreadyExists {
    pub fn new(db_name: impl Into<String>, context: impl Into<String>) -> Self {
        Self {
            db_name: db_name.into(),
            context: context.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, thiserror::Error)]
#[error("CreateDatabaseWithDropTime: `{db_name}` with drop_on")]
pub struct CreateDatabaseWithDropTime {
    db_name: String,
}

impl CreateDatabaseWithDropTime {
    pub fn new(db_name: impl Into<String>) -> Self {
        Self {
            db_name: db_name.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, thiserror::Error)]
#[error("DropDbWithDropTime: drop {db_name} with drop_on time")]
pub struct DropDbWithDropTime {
    db_name: String,
}

impl DropDbWithDropTime {
    pub fn new(db_name: impl Into<String>) -> Self {
        Self {
            db_name: db_name.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, thiserror::Error)]
#[error("UndropDbWithNoDropTime: undrop {db_name} with no drop_on time")]
pub struct UndropDbWithNoDropTime {
    db_name: String,
}

impl UndropDbWithNoDropTime {
    pub fn new(db_name: impl Into<String>) -> Self {
        Self {
            db_name: db_name.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, thiserror::Error)]
#[error("UndropDbHasNoHistory: undrop {db_name} has no db id history")]
pub struct UndropDbHasNoHistory {
    db_name: String,
}

impl UndropDbHasNoHistory {
    pub fn new(db_name: impl Into<String>) -> Self {
        Self {
            db_name: db_name.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, thiserror::Error)]
#[error("TableAlreadyExists: {table_name} while {context}")]
pub struct TableAlreadyExists {
    table_name: String,
    context: String,
}

impl TableAlreadyExists {
    pub fn new(table_name: impl Into<String>, context: impl Into<String>) -> Self {
        Self {
            table_name: table_name.into(),
            context: context.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, thiserror::Error)]
#[error("CreateTableWithDropTime: create {table_name} with drop time")]
pub struct CreateTableWithDropTime {
    table_name: String,
}

impl CreateTableWithDropTime {
    pub fn new(table_name: impl Into<String>) -> Self {
        Self {
            table_name: table_name.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, thiserror::Error)]
#[error("UndropTableAlreadyExists: undrop {table_name} already exists")]
pub struct UndropTableAlreadyExists {
    table_name: String,
}

impl UndropTableAlreadyExists {
    pub fn new(table_name: impl Into<String>) -> Self {
        Self {
            table_name: table_name.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, thiserror::Error)]
#[error("UndropTableWithNoDropTime: undrop {table_name} with no drop_on time")]
pub struct UndropTableWithNoDropTime {
    table_name: String,
}

impl UndropTableWithNoDropTime {
    pub fn new(table_name: impl Into<String>) -> Self {
        Self {
            table_name: table_name.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, thiserror::Error)]
#[error("DropTableWithDropTime: drop {table_name} with drop_on time")]
pub struct DropTableWithDropTime {
    table_name: String,
}

impl DropTableWithDropTime {
    pub fn new(table_name: impl Into<String>) -> Self {
        Self {
            table_name: table_name.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, thiserror::Error)]
#[error("UndropTableHasNoHistory: undrop {table_name} has no table id history")]
pub struct UndropTableHasNoHistory {
    table_name: String,
}

impl UndropTableHasNoHistory {
    pub fn new(table_name: impl Into<String>) -> Self {
        Self {
            table_name: table_name.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, thiserror::Error)]
#[error("TableVersionMismatched: {table_id} expect `{expect}` but `{curr}`  while `{context}`")]
pub struct TableVersionMismatched {
    table_id: u64,
    expect: MatchSeq,
    curr: u64,
    context: String,
}

impl TableVersionMismatched {
    pub fn new(table_id: u64, expect: MatchSeq, curr: u64, context: impl Into<String>) -> Self {
        Self {
            table_id,
            expect,
            curr,
            context: context.into(),
        }
    }
}

#[derive(Error, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[error("UnknownDatabase: `{db_name}` while `{context}`")]
pub struct UnknownDatabase {
    db_name: String,
    context: String,
}

impl UnknownDatabase {
    pub fn new(db_name: impl Into<String>, context: impl Into<String>) -> Self {
        Self {
            db_name: db_name.into(),
            context: context.into(),
        }
    }
}

#[derive(Error, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[error("UnknownDatabaseId: `{db_id}` while `{context}`")]
pub struct UnknownDatabaseId {
    db_id: u64,
    context: String,
}

impl UnknownDatabaseId {
    pub fn new(db_id: u64, context: String) -> UnknownDatabaseId {
        Self { db_id, context }
    }
}

#[derive(Error, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[error("UnknownTable: `{table_name}` while `{context}`")]
pub struct UnknownTable {
    table_name: String,
    context: String,
}

impl UnknownTable {
    pub fn new(table_name: impl Into<String>, context: impl Into<String>) -> Self {
        Self {
            table_name: table_name.into(),
            context: context.into(),
        }
    }
}

#[derive(Error, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[error("UnknownTableId: `{table_id}` while `{context}`")]
pub struct UnknownTableId {
    table_id: u64,
    context: String,
}

impl UnknownTableId {
    pub fn new(table_id: u64, context: impl Into<String>) -> UnknownTableId {
        Self {
            table_id,
            context: context.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, thiserror::Error)]
#[error("ShareAlreadyExists: {share_name} while {context}")]
pub struct ShareAlreadyExists {
    share_name: String,
    context: String,
}

impl ShareAlreadyExists {
    pub fn new(share_name: impl Into<String>, context: impl Into<String>) -> Self {
        Self {
            share_name: share_name.into(),
            context: context.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, thiserror::Error)]
#[error("UnknownShare: {share_name} while {context}")]
pub struct UnknownShare {
    share_name: String,
    context: String,
}

impl UnknownShare {
    pub fn new(share_name: impl Into<String>, context: impl Into<String>) -> Self {
        Self {
            share_name: share_name.into(),
            context: context.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, thiserror::Error)]
#[error("UnknownShareID: {share_id} while {context}")]
pub struct UnknownShareId {
    share_id: u64,
    context: String,
}

impl UnknownShareId {
    pub fn new(share_id: u64, context: impl Into<String>) -> Self {
        Self {
            share_id,
            context: context.into(),
        }
    }
}

#[derive(Error, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum AppError {
    #[error(transparent)]
    TableVersionMismatched(#[from] TableVersionMismatched),

    #[error(transparent)]
    TableAlreadyExists(#[from] TableAlreadyExists),

    #[error(transparent)]
    CreateTableWithDropTime(#[from] CreateTableWithDropTime),

    #[error(transparent)]
    UndropTableAlreadyExists(#[from] UndropTableAlreadyExists),

    #[error(transparent)]
    UndropTableWithNoDropTime(#[from] UndropTableWithNoDropTime),

    #[error(transparent)]
    DropTableWithDropTime(#[from] DropTableWithDropTime),

    #[error(transparent)]
    UndropTableHasNoHistory(#[from] UndropTableHasNoHistory),

    #[error(transparent)]
    DatabaseAlreadyExists(#[from] DatabaseAlreadyExists),

    #[error(transparent)]
    CreateDatabaseWithDropTime(#[from] CreateDatabaseWithDropTime),

    #[error(transparent)]
    DropDbWithDropTime(#[from] DropDbWithDropTime),

    #[error(transparent)]
    UndropDbWithNoDropTime(#[from] UndropDbWithNoDropTime),

    #[error(transparent)]
    UndropDbHasNoHistory(#[from] UndropDbHasNoHistory),

    #[error(transparent)]
    UnknownDatabase(#[from] UnknownDatabase),

    #[error(transparent)]
    UnknownDatabaseId(#[from] UnknownDatabaseId),

    #[error(transparent)]
    UnknownTable(#[from] UnknownTable),

    #[error(transparent)]
    UnknownTableId(#[from] UnknownTableId),

    #[error(transparent)]
    ShareAlreadyExists(#[from] ShareAlreadyExists),

    #[error(transparent)]
    UnknownShare(#[from] UnknownShare),

    #[error(transparent)]
    UnknownShareId(#[from] UnknownShareId),
}

impl AppErrorMessage for UnknownDatabase {
    fn message(&self) -> String {
        format!("Unknown database '{}'", self.db_name)
    }
}

impl AppErrorMessage for DatabaseAlreadyExists {
    fn message(&self) -> String {
        format!("Database '{}' already exists", self.db_name)
    }
}

impl AppErrorMessage for CreateDatabaseWithDropTime {
    fn message(&self) -> String {
        format!("Create database '{}' with drop time", self.db_name)
    }
}

impl AppErrorMessage for UndropDbHasNoHistory {
    fn message(&self) -> String {
        format!("Undrop database '{}' has no id history", self.db_name)
    }
}

impl AppErrorMessage for UnknownTable {
    fn message(&self) -> String {
        format!("Unknown table '{}'", self.table_name)
    }
}

impl AppErrorMessage for UnknownTableId {}
impl AppErrorMessage for UnknownDatabaseId {}
impl AppErrorMessage for TableVersionMismatched {}

impl AppErrorMessage for TableAlreadyExists {
    fn message(&self) -> String {
        format!("Table '{}' already exists", self.table_name)
    }
}

impl AppErrorMessage for CreateTableWithDropTime {
    fn message(&self) -> String {
        format!("Create Table '{}' with drop time", self.table_name)
    }
}

impl AppErrorMessage for UndropTableAlreadyExists {
    fn message(&self) -> String {
        format!("Undrop Table '{}' already exists", self.table_name)
    }
}

impl AppErrorMessage for UndropTableHasNoHistory {
    fn message(&self) -> String {
        format!("Undrop Table '{}' has no table id list", self.table_name)
    }
}

impl AppErrorMessage for ShareAlreadyExists {
    fn message(&self) -> String {
        format!("Share '{}' already exists", self.share_name)
    }
}

impl AppErrorMessage for UnknownShare {
    fn message(&self) -> String {
        format!("Unknown share '{}'", self.share_name)
    }
}

impl AppErrorMessage for UnknownShareId {
    fn message(&self) -> String {
        format!("Unknown share id '{}'", self.share_id)
    }
}

impl AppErrorMessage for UndropTableWithNoDropTime {
    fn message(&self) -> String {
        format!("Undrop table '{}' with no drop_on time", self.table_name)
    }
}

impl AppErrorMessage for DropTableWithDropTime {
    fn message(&self) -> String {
        format!("Drop table '{}' with drop_on time", self.table_name)
    }
}

impl AppErrorMessage for UndropDbWithNoDropTime {
    fn message(&self) -> String {
        format!("Undrop db '{}' with no drop_on time", self.db_name)
    }
}

impl AppErrorMessage for DropDbWithDropTime {
    fn message(&self) -> String {
        format!("Drop db '{}' with drop_on time", self.db_name)
    }
}

impl From<AppError> for ErrorCode {
    fn from(app_err: AppError) -> Self {
        match app_err {
            AppError::UnknownDatabase(err) => ErrorCode::UnknownDatabase(err.message()),
            AppError::UnknownDatabaseId(err) => ErrorCode::UnknownDatabaseId(err.message()),
            AppError::UnknownTableId(err) => ErrorCode::UnknownTableId(err.message()),
            AppError::UnknownTable(err) => ErrorCode::UnknownTable(err.message()),
            AppError::DatabaseAlreadyExists(err) => ErrorCode::DatabaseAlreadyExists(err.message()),
            AppError::CreateDatabaseWithDropTime(err) => {
                ErrorCode::CreateDatabaseWithDropTime(err.message())
            }
            AppError::UndropDbHasNoHistory(err) => ErrorCode::UndropDbHasNoHistory(err.message()),
            AppError::UndropTableWithNoDropTime(err) => {
                ErrorCode::UndropTableWithNoDropTime(err.message())
            }
            AppError::DropTableWithDropTime(err) => ErrorCode::DropTableWithDropTime(err.message()),
            AppError::DropDbWithDropTime(err) => ErrorCode::DropDbWithDropTime(err.message()),
            AppError::UndropDbWithNoDropTime(err) => {
                ErrorCode::UndropDbWithNoDropTime(err.message())
            }
            AppError::TableAlreadyExists(err) => ErrorCode::TableAlreadyExists(err.message()),
            AppError::CreateTableWithDropTime(err) => {
                ErrorCode::CreateTableWithDropTime(err.message())
            }
            AppError::UndropTableAlreadyExists(err) => {
                ErrorCode::UndropTableAlreadyExists(err.message())
            }
            AppError::UndropTableHasNoHistory(err) => {
                ErrorCode::UndropTableHasNoHistory(err.message())
            }
            AppError::TableVersionMismatched(err) => {
                ErrorCode::TableVersionMismatched(err.message())
            }
            AppError::ShareAlreadyExists(err) => ErrorCode::ShareAlreadyExists(err.message()),
            AppError::UnknownShare(err) => ErrorCode::UnknownShare(err.message()),
            AppError::UnknownShareId(err) => ErrorCode::UnknownShareId(err.message()),
        }
    }
}

pub type MetaStorageResult<T> = std::result::Result<T, MetaStorageError>;

impl From<MetaStorageError> for ErrorCode {
    fn from(e: MetaStorageError) -> Self {
        match e {
            MetaStorageError::AppError(app_err) => app_err.into(),
            _ => ErrorCode::MetaStorageError(e.to_string()),
        }
    }
}

impl From<std::string::FromUtf8Error> for MetaStorageError {
    fn from(error: std::string::FromUtf8Error) -> Self {
        MetaStorageError::BytesError(format!(
            "Bad bytes, cannot parse bytes with UTF8, cause: {}",
            error
        ))
    }
}

// from serde error to MetaStorageError::SerdeError
impl From<serde_json::Error> for MetaStorageError {
    fn from(error: serde_json::Error) -> MetaStorageError {
        MetaStorageError::SerdeError(format!("serde json se/de error: {:?}", error))
    }
}

impl From<sled::Error> for MetaStorageError {
    fn from(e: sled::Error) -> MetaStorageError {
        MetaStorageError::SledError(AnyError::new(&e))
    }
}

impl From<ErrorWithContext<sled::Error>> for MetaStorageError {
    fn from(e: ErrorWithContext<sled::Error>) -> MetaStorageError {
        MetaStorageError::SledError(AnyError::new(&e.err).add_context(|| e.context))
    }
}

pub trait ToMetaStorageError<T, E, CtxFn>
where E: Display + Send + Sync + 'static
{
    /// Wrap the error value with MetaError. It is lazily evaluated:
    /// only when an error does occur.
    ///
    /// `err_code_fn` is one of the MetaError builder function such as `MetaError::Ok`.
    /// `context_fn` builds display_text for the MetaError.
    fn map_error_to_meta_storage_error<ErrFn, D>(
        self,
        err_code_fn: ErrFn,
        context_fn: CtxFn,
    ) -> MetaStorageResult<T>
    where
        ErrFn: FnOnce(String) -> MetaStorageError,
        D: Display,
        CtxFn: FnOnce() -> D;
}

impl<T, E, CtxFn> ToMetaStorageError<T, E, CtxFn> for std::result::Result<T, E>
where E: Display + Send + Sync + 'static
{
    fn map_error_to_meta_storage_error<ErrFn, D>(
        self,
        make_exception: ErrFn,
        context_fn: CtxFn,
    ) -> MetaStorageResult<T>
    where
        ErrFn: FnOnce(String) -> MetaStorageError,
        D: Display,
        CtxFn: FnOnce() -> D,
    {
        self.map_err(|error| {
            let err_text = format!("meta storage error: {}, cause: {}", context_fn(), error);
            make_exception(err_text)
        })
    }
}

impl From<UnabortableTransactionError> for MetaStorageError {
    fn from(error: UnabortableTransactionError) -> Self {
        match error {
            UnabortableTransactionError::Storage(e) => {
                MetaStorageError::SledError(AnyError::new(&e))
            }
            UnabortableTransactionError::Conflict => MetaStorageError::TransactionConflict,
        }
    }
}
