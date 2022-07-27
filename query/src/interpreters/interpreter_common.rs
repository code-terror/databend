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

use std::io;
use std::sync::Arc;

use chrono::TimeZone;
use chrono::Utc;
use common_exception::ErrorCode;
use common_exception::Result;
use common_meta_types::GrantObject;
use common_meta_types::StageFile;
use common_meta_types::StageType;
use common_meta_types::UserStageInfo;
use common_tracing::tracing::warn;
use futures::TryStreamExt;
use regex::Regex;

use crate::sessions::QueryContext;
use crate::storages::stage::StageSource;

pub async fn validate_grant_object_exists(
    ctx: &Arc<QueryContext>,
    object: &GrantObject,
) -> Result<()> {
    let tenant = ctx.get_tenant();

    match &object {
        GrantObject::Table(catalog_name, database_name, table_name) => {
            let catalog = ctx.get_catalog(catalog_name)?;
            if !catalog
                .exists_table(tenant.as_str(), database_name, table_name)
                .await?
            {
                return Err(common_exception::ErrorCode::UnknownTable(format!(
                    "table {}.{} not exists",
                    database_name, table_name,
                )));
            }
        }
        GrantObject::Database(catalog_name, database_name) => {
            let catalog = ctx.get_catalog(catalog_name)?;
            if !catalog
                .exists_database(tenant.as_str(), database_name)
                .await?
            {
                return Err(common_exception::ErrorCode::UnknownDatabase(format!(
                    "database {} not exists",
                    database_name,
                )));
            }
        }
        GrantObject::Global => (),
    }

    Ok(())
}

pub async fn list_files(
    ctx: &Arc<QueryContext>,
    stage: &UserStageInfo,
    path: &str,
    pattern: &str,
) -> Result<Vec<StageFile>> {
    match stage.stage_type {
        StageType::Internal => list_files_from_meta_api(ctx, stage, path, pattern).await,
        StageType::External => list_files_from_dal(ctx, stage, path, pattern).await,
    }
}

/// List files from DAL in recursive way.
///
/// - If input path is a dir, we will list it recursively.
/// - Or, we will append the file itself, and try to list `path/`.
/// - If not exist, we will try to list `path/` too.
pub async fn list_files_from_dal(
    ctx: &Arc<QueryContext>,
    stage: &UserStageInfo,
    path: &str,
    pattern: &str,
) -> Result<Vec<StageFile>> {
    let op = StageSource::get_op(ctx, stage).await?;
    let mut files = Vec::new();

    // - If the path itself is a dir, return directly.
    // - Otherwise, return a path suffix by `/`
    // - If other errors happen, we will ignore them by returning None.
    let dir_path = match op.object(path).metadata().await {
        Ok(meta) if meta.mode().is_dir() => Some(path.to_string()),
        Ok(meta) if !meta.mode().is_dir() => {
            files.push((path.to_string(), meta));

            Some(format!("{path}/"))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Some(format!("{path}/")),
        Err(e) => return Err(e.into()),
        _ => None,
    };

    // Check the if this dir valid and list it recursively.
    if let Some(dir) = dir_path {
        match op.object(&dir).metadata().await {
            Ok(_) => {
                let mut ds = op.batch().walk_top_down(&dir)?;
                while let Some(de) = ds.try_next().await? {
                    if de.mode().is_file() {
                        let path = de.path().to_string();
                        let meta = de.metadata().await?;
                        files.push((path, meta));
                    }
                }
            }
            Err(e) => warn!("ignore listing {path}/, because: {:?}", e),
        };
    }

    let regex = if !pattern.is_empty() {
        Some(Regex::new(pattern).map_err(|e| {
            ErrorCode::SyntaxException(format!(
                "Pattern format invalid, got:{}, error:{:?}",
                pattern, e
            ))
        })?)
    } else {
        None
    };

    let matched_files = files
        .iter()
        .filter(|(name, _meta)| {
            if let Some(regex) = &regex {
                regex.is_match(name)
            } else {
                true
            }
        })
        .cloned()
        .map(|(name, meta)| StageFile {
            path: name,
            size: meta.content_length(),
            md5: meta.content_md5().map(str::to_string),
            last_modified: meta
                .last_modified()
                .map_or(Utc::now(), |t| Utc.timestamp(t.unix_timestamp(), 0)),
            creator: None,
        })
        .collect::<Vec<StageFile>>();
    Ok(matched_files)
}

pub async fn list_files_from_meta_api(
    ctx: &Arc<QueryContext>,
    stage: &UserStageInfo,
    path: &str,
    pattern: &str,
) -> Result<Vec<StageFile>> {
    let tenant = ctx.get_tenant();
    let user_mgr = ctx.get_user_manager();
    let prefix = stage.get_prefix();

    if stage.number_of_files == 0 {
        // try to sync files from dal
        if let Ok(files) = list_files_from_dal(ctx, stage, &prefix, "").await {
            for file in files.iter() {
                let mut file = file.clone();
                // In internal stage, files with `/stage/<stage_name>/` prefix will be ignored.
                // TODO: prefix of internal stage should be as root path.
                file.path = file
                    .path
                    .trim_start_matches(&prefix.trim_start_matches('/'))
                    .to_string();
                let _ = user_mgr.add_file(&tenant, &stage.stage_name, file).await;
            }
        }
    }

    let regex = if !pattern.is_empty() {
        Some(Regex::new(pattern).map_err(|e| {
            ErrorCode::SyntaxException(format!(
                "Pattern format invalid, got:{}, error:{:?}",
                pattern, e
            ))
        })?)
    } else {
        None
    };

    let files = user_mgr
        .list_files(&tenant, &stage.stage_name)
        .await?
        .iter()
        .filter(|file| {
            let name = format!("{}{}", prefix, file.path);
            if path.ends_with('/') {
                name.starts_with(path)
            } else {
                name.starts_with(&format!("{path}/")) || name == path
            }
        })
        .filter(|file| {
            if let Some(regex) = &regex {
                regex.is_match(&file.path)
            } else {
                true
            }
        })
        .cloned()
        .collect::<Vec<_>>();
    Ok(files)
}
