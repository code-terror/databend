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

use std::fs::File;
use std::io::Read;
use std::time::Duration;

use base64::encode_config;
use base64::URL_SAFE_NO_PAD;
use common_base::base::get_free_tcp_port;
use common_base::base::tokio;
use common_exception::ErrorCode;
use common_exception::Result;
use databend_query::servers::http::middleware::HTTPSessionEndpoint;
use databend_query::servers::http::middleware::HTTPSessionMiddleware;
use databend_query::servers::http::v1::make_final_uri;
use databend_query::servers::http::v1::make_page_uri;
use databend_query::servers::http::v1::make_state_uri;
use databend_query::servers::http::v1::query_route;
use databend_query::servers::http::v1::ExecuteStateKind;
use databend_query::servers::http::v1::HttpSession;
use databend_query::servers::http::v1::QueryResponse;
use databend_query::servers::HttpHandler;
use databend_query::users::auth::jwt::CustomClaims;
use databend_query::users::auth::jwt::EnsureUser;
use headers::Header;
use jwt_simple::algorithms::RS256KeyPair;
use jwt_simple::algorithms::RSAKeyPairLike;
use jwt_simple::claims::JWTClaims;
use jwt_simple::claims::NoCustomClaims;
use jwt_simple::prelude::Clock;
use num::ToPrimitive;
use poem::http::header;
use poem::http::Method;
use poem::http::StatusCode;
use poem::Endpoint;
use poem::EndpointExt;
use poem::Request;
use poem::Response;
use poem::Route;
use pretty_assertions::assert_eq;
use tokio::time::sleep;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;

use crate::tests::tls_constants::TEST_CA_CERT;
use crate::tests::tls_constants::TEST_CN_NAME;
use crate::tests::tls_constants::TEST_SERVER_CERT;
use crate::tests::tls_constants::TEST_SERVER_KEY;
use crate::tests::tls_constants::TEST_TLS_CA_CERT;
use crate::tests::tls_constants::TEST_TLS_CLIENT_IDENTITY;
use crate::tests::tls_constants::TEST_TLS_CLIENT_PASSWORD;
use crate::tests::tls_constants::TEST_TLS_SERVER_CERT;
use crate::tests::tls_constants::TEST_TLS_SERVER_KEY;
use crate::tests::SessionManagerBuilder;

type EndpointType = HTTPSessionEndpoint<Route>;

// TODO(youngsofun): add test for
// 1. query fail after started

#[tokio::test]
async fn test_simple_sql() -> Result<()> {
    let sql = "select * from system.tables limit 10";
    let (status, result) = post_sql(sql, 1).await?;
    assert_eq!(status, StatusCode::OK, "{:?}", result);
    assert!(result.error.is_none(), "{:?}", result.error);
    assert_eq!(result.data.len(), 10, "{:?}", result);
    assert_eq!(result.state, ExecuteStateKind::Succeeded, "{:?}", result);
    assert!(result.next_uri.is_none(), "{:?}", result);
    assert!(result.stats.scan_progress.is_some(), "{:?}", result);
    assert!(result.schema.is_some(), "{:?}", result);
    assert_eq!(
        result.schema.as_ref().unwrap().fields().len(),
        9,
        "{:?}",
        result
    );

    let sql = "show databases";
    let (status, result) = post_sql(sql, 1).await?;
    assert_eq!(status, StatusCode::OK, "{:?}", result);
    assert!(result.error.is_none(), "{:?}", result);
    assert!(result.schema.is_some(), "{:?}", result);
    assert_eq!(
        result.schema.as_ref().unwrap().fields().len(),
        1,
        "{:?}",
        result
    );
    Ok(())
}

#[tokio::test]
async fn test_return_when_finish() -> Result<()> {
    let wait_time_secs = 5;
    let sql = "create table t1(a int)";
    let ep = create_endpoint();
    let (status, result) = post_sql_to_endpoint(&ep, sql, wait_time_secs).await?;
    assert_eq!(status, StatusCode::OK, "{:?}", result);
    assert_eq!(result.state, ExecuteStateKind::Succeeded, "{:?}", result);
    for (sql, state) in [
        ("select * from numbers(1)", ExecuteStateKind::Succeeded),
        ("bad sql", ExecuteStateKind::Failed), // parse fail
        ("select cast(null as boolean)", ExecuteStateKind::Failed), // execute fail at once
        ("create table t1(a int)", ExecuteStateKind::Failed),
    ] {
        let start_time = std::time::Instant::now();
        let (status, result) = post_sql_to_endpoint(&ep, sql, wait_time_secs).await?;
        let duration = start_time.elapsed().as_secs_f64();
        let msg = || format!("{}: {:?}", sql, result);
        assert_eq!(status, StatusCode::OK, "{}", msg());
        assert_eq!(result.state, state, "{}", msg());
        // should not wait until wait_time_secs even if there is no more data
        assert!(
            duration < 1.0,
            "duration {} is too large than expect",
            msg()
        );
    }
    Ok(())
}

#[tokio::test]
async fn test_bad_sql() -> Result<()> {
    let sql = "bad sql";
    let ep = create_endpoint();
    let (status, result) = post_sql_to_endpoint(&ep, sql, 1).await?;
    assert_eq!(status, StatusCode::OK);
    assert!(result.error.is_some(), "{:?}", result);
    assert_eq!(result.data.len(), 0, "{:?}", result);
    assert!(result.next_uri.is_none(), "{:?}", result);
    assert_eq!(result.state, ExecuteStateKind::Failed, "{:?}", result);
    assert!(result.stats.scan_progress.is_none(), "{:?}", result);
    assert!(result.schema.is_none(), "{:?}", result);

    let sql = "select query_text, exception_code, exception_text, stack_trace from system.query_log where log_type=3";
    let (status, result) = post_sql_to_endpoint(&ep, sql, 1).await?;
    assert_eq!(status, StatusCode::OK, "{:?}", result);
    assert_eq!(result.data.len(), 1, "{:?}", result);
    assert_eq!(
        result.data[0][0].as_str().unwrap(),
        "bad sql",
        "{:?}",
        result
    );
    assert_eq!(
        result.data[0][1].as_u64().unwrap(),
        ErrorCode::SyntaxException("").code().to_u64().unwrap(),
        "{:?}",
        result
    );

    assert!(
        result.data[0][2]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("bad"),
        "{:?}",
        result
    );

    Ok(())
}

#[tokio::test]
async fn test_async() -> Result<()> {
    let ep = create_endpoint();
    let sql = "select sleep(0.01)";
    let json = serde_json::json!({"sql": sql.to_string(), "pagination": {"wait_time_secs": 0}});

    let (status, result) = post_json_to_endpoint(&ep, &json).await?;
    assert_eq!(status, StatusCode::OK, "{:?}", result);
    let query_id = &result.id;
    let next_uri = make_page_uri(query_id, 0);
    assert!(result.error.is_none(), "{:?}", result);
    assert_eq!(result.data.len(), 0, "{:?}", result);
    assert_eq!(result.next_uri, Some(next_uri), "{:?}", result);
    assert!(result.stats.scan_progress.is_some(), "{:?}", result);
    assert!(result.schema.is_some(), "{:?}", result);
    assert_eq!(result.state, ExecuteStateKind::Running, "{:?}", result);
    sleep(Duration::from_millis(100)).await;

    // get page, support retry
    for _ in 1..3 {
        let uri = make_page_uri(query_id, 0);

        let (status, result) = get_uri_checked(&ep, &uri).await?;
        assert_eq!(status, StatusCode::OK, "{:?}", result);
        assert!(result.error.is_none(), "{:?}", result);
        assert_eq!(result.data.len(), 1, "{:?}", result);
        assert!(result.next_uri.is_none(), "{:?}", result);
        assert!(result.schema.is_some(), "{:?}", result);
        assert!(result.stats.scan_progress.is_some(), "{:?}", result);
        assert_eq!(result.state, ExecuteStateKind::Succeeded, "{:?}", result);
    }

    // get state
    let uri = make_state_uri(query_id);
    let (status, result) = get_uri_checked(&ep, &uri).await?;
    assert_eq!(status, StatusCode::OK, "{:?}", result);
    assert!(result.error.is_none(), "{:?}", result);
    assert_eq!(result.data.len(), 0, "{:?}", result);
    assert!(result.next_uri.is_none(), "{:?}", result);
    assert!(result.schema.is_some(), "{:?}", result);
    assert!(result.stats.scan_progress.is_some(), "{:?}", result);
    assert_eq!(result.state, ExecuteStateKind::Succeeded, "{:?}", result);

    // get page not expected
    let uri = make_page_uri(query_id, 1);
    let response = get_uri(&ep, &uri).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND, "{:?}", result);
    let body = response.into_body().into_string().await.unwrap();
    assert_eq!(body, "wrong page number 1");

    // delete
    let status = delete_query(&ep, query_id).await;
    assert_eq!(status, StatusCode::OK, "{:?}", result);

    let response = get_uri(&ep, &uri).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND, "{:?}", result);

    Ok(())
}
#[tokio::test]
async fn test_buffer_size() -> Result<()> {
    let rows = 100;
    let ep = create_endpoint();
    let sql = format!("select * from numbers({})", rows);

    for buf_size in [0, 99, 100, 101] {
        let json = serde_json::json!({"sql": sql.to_string(), "pagination": {"wait_time_secs": 1, "max_rows_in_buffer": buf_size}});
        let (status, result) = post_json_to_endpoint(&ep, &json).await?;
        assert_eq!(
            result.data.len(),
            rows,
            "buf_size={}, result={:?}",
            buf_size,
            result
        );
        assert_eq!(status, StatusCode::OK, "{} {:?}", buf_size, result);
    }
    Ok(())
}

#[tokio::test]
async fn test_pagination() -> Result<()> {
    let ep = create_endpoint();
    let sql = "select * from numbers(10)";
    let json = serde_json::json!({"sql": sql.to_string(), "pagination": {"wait_time_secs": 1, "max_rows_per_page": 2}});

    let (status, result) = post_json_to_endpoint(&ep, &json).await?;
    assert_eq!(status, StatusCode::OK, "{:?}", result);
    let query_id = &result.id;
    let next_uri = make_page_uri(query_id, 1);
    assert!(result.error.is_none(), "{:?}", result);
    assert_eq!(result.data.len(), 2, "{:?}", result);
    assert_eq!(result.next_uri, Some(next_uri), "{:?}", result);
    assert!(result.stats.scan_progress.is_some(), "{:?}", result);
    assert!(result.schema.is_some(), "{:?}", result);

    for page in 0..5 {
        let msg = || format!("page {}: {:?}", page, result);
        let uri = make_page_uri(query_id, page);

        let (status, result) = get_uri_checked(&ep, &uri).await?;
        assert_eq!(status, StatusCode::OK, "{:?}", msg());
        assert!(result.error.is_none(), "{:?}", msg());
        assert_eq!(result.data.len(), 2, "{:?}", msg());
        assert!(result.schema.is_some(), "{:?}", result);
        assert!(result.stats.scan_progress.is_some(), "{:?}", msg());
        if page == 4 {
            assert_eq!(result.state, ExecuteStateKind::Succeeded, "{:?}", msg());
            assert!(result.next_uri.is_none(), "{:?}", msg());
        } else {
            assert!(result.next_uri.is_some(), "{:?}", msg());
        }
    }

    // get state
    let uri = make_state_uri(query_id);
    let (status, result) = get_uri_checked(&ep, &uri).await?;
    assert_eq!(status, StatusCode::OK);
    assert!(result.error.is_none(), "{:?}", result.error);
    assert_eq!(result.data.len(), 0, "{:?}", result);
    assert!(result.next_uri.is_none(), "{:?}", result);
    assert!(result.schema.is_some(), "{:?}", result);
    assert!(result.stats.scan_progress.is_some(), "{:?}", result);
    assert_eq!(result.state, ExecuteStateKind::Succeeded, "{:?}", result);

    // get page not expected
    let uri = make_page_uri(query_id, 6);
    let response = get_uri(&ep, &uri).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND, "{:?}", result);
    let body = response.into_body().into_string().await.unwrap();
    assert_eq!(body, "wrong page number 6");

    // delete
    let status = delete_query(&ep, query_id).await;
    assert_eq!(status, StatusCode::OK, "{:?}", result);

    let response = get_uri(&ep, &uri).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND, "{:?}", result);

    Ok(())
}

#[test]
fn test_http_session_serde() {
    {
        let json = r#"{"id": "abc"}"#;
        assert_eq!(
            serde_json::from_str::<HttpSession>(json).unwrap(),
            HttpSession::Old {
                id: "abc".to_string()
            }
        );
    }

    {
        let json = r#"{}"#;
        assert_eq!(
            serde_json::from_str::<HttpSession>(json).unwrap(),
            HttpSession::New(Default::default())
        );
    }

    {
        let json = r#"{"unexpected": ""}"#;
        assert!(serde_json::from_str::<HttpSession>(json).is_err());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "flaky, to be investigated"]
async fn test_http_session() -> Result<()> {
    let ep = create_endpoint();
    let json = serde_json::json!({"sql":  "use system", "session": {"max_idle_time": 10}});

    let (status, result) = post_json_to_endpoint(&ep, &json).await?;
    assert_eq!(status, StatusCode::OK);
    assert!(result.error.is_none(), "{:?}", result);
    assert_eq!(result.data.len(), 0, "{:?}", result);
    assert_eq!(result.next_uri, None, "{:?}", result);
    assert!(result.stats.scan_progress.is_some(), "{:?}", result);
    assert!(result.schema.is_some(), "{:?}", result);
    assert_eq!(result.state, ExecuteStateKind::Succeeded, "{:?}", result);
    let session_id = &result.session_id.unwrap();

    let json = serde_json::json!({"sql": "select database()", "session": {"id": session_id}});
    let (status, result) = post_json_to_endpoint(&ep, &json).await?;
    assert!(result.error.is_none(), "{:?}", result);
    assert_eq!(status, StatusCode::OK, "{:?}", result);
    assert_eq!(result.data.len(), 1, "{:?}", result);
    assert_eq!(result.data[0][0], "system", "{:?}", result);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_result_timeout() -> Result<()> {
    let session_manager = SessionManagerBuilder::create()
        .http_handler_result_time_out(200u64)
        .build()
        .unwrap();
    let ep = Route::new()
        .nest("/v1/query", query_route())
        .with(HTTPSessionMiddleware { session_manager });

    let sql = "select sleep(0.1)";
    let json = serde_json::json!({"sql": sql.to_string(), "pagination": {"wait_time_secs": 0}});
    let (status, result) = post_json_to_endpoint(&ep, &json).await?;
    assert_eq!(status, StatusCode::OK, "{:?}", result);
    let query_id = result.id.clone();
    let next_uri = make_page_uri(&query_id, 0);
    assert_eq!(result.next_uri, Some(next_uri.clone()), "{:?}", result);

    sleep(Duration::from_millis(110)).await;
    let response = get_uri(&ep, &next_uri).await;
    assert_eq!(response.status(), StatusCode::OK, "{:?}", result);

    sleep(std::time::Duration::from_millis(210)).await;
    let response = get_uri(&ep, &next_uri).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND, "{:?}", result);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_system_tables() -> Result<()> {
    let session_manager = SessionManagerBuilder::create().build().unwrap();
    let ep = Route::new()
        .nest("/v1/query", query_route())
        .with(HTTPSessionMiddleware { session_manager });

    let sql = "select name from system.tables where database='system' order by name";

    let (status, result) = post_sql_to_endpoint(&ep, sql, 1).await?;
    assert_eq!(status, StatusCode::OK, "{:?}", result);
    assert!(!result.data.is_empty(), "{:?}", result);

    let table_names = result
        .data
        .iter()
        .flatten()
        .map(|j| j.as_str().unwrap().to_string())
        .collect::<Vec<_>>();

    let skipped = vec![
        "credits", // slow for ci (> 1s) and maybe flaky
        "metrics", // QueryError: "Prometheus recorder is not initialized yet"
    ];
    for table_name in table_names {
        if skipped.contains(&table_name.as_str()) {
            continue;
        };
        let sql = format!("select * from system.{}", table_name);
        let (status, result) = post_sql_to_endpoint(&ep, &sql, 1).await.map_err(|e| {
            ErrorCode::UnexpectedError(format!("system.{}: {}", table_name, e.message()))
        })?;
        let error_message = format!("{}: status={:?}, result={:?}", table_name, status, result);
        assert_eq!(status, StatusCode::OK, "{}", error_message);
        assert!(result.error.is_none(), "{}", error_message);
        assert_eq!(
            result.state,
            ExecuteStateKind::Succeeded,
            "{}",
            error_message
        );
        assert!(result.stats.scan_progress.is_some(), "{:?}", result);
        assert!(result.next_uri.is_none(), "{:?}", result);
        assert!(result.schema.is_some(), "{:?}", result);
    }
    Ok(())
}

#[tokio::test]
async fn test_insert() -> Result<()> {
    let route = create_endpoint();

    let sqls = vec![
        ("create table t(a int) engine=fuse", 0),
        ("insert into t(a) values (1),(2)", 0),
        ("select * from t", 2),
    ];

    for (sql, data_len) in sqls {
        let json = serde_json::json!({"sql": sql.to_string(), "pagination": {"wait_time_secs": 3}});
        let (status, result) = post_json_to_endpoint(&route, &json).await?;
        assert_eq!(status, StatusCode::OK, "{:?}", result);
        assert!(result.error.is_none(), "{:?}", result.error);
        assert_eq!(result.data.len(), data_len, "{:?}", result);
        assert_eq!(result.state, ExecuteStateKind::Succeeded, "{:?}", result);
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_query_log() -> Result<()> {
    let session_manager = SessionManagerBuilder::create().build().unwrap();
    let ep = Route::new()
        .nest("/v1/query", query_route())
        .with(HTTPSessionMiddleware { session_manager });

    let sql = "create table t1(a int)";
    let (status, result) = post_sql_to_endpoint(&ep, sql, 1).await?;
    assert_eq!(status, StatusCode::OK, "{:?}", result);
    assert!(result.error.is_none(), "{:?}", result);
    assert!(result.next_uri.is_none(), "{:?}", result);
    assert!(result.data.is_empty(), "{:?}", result);

    let (status, result) = post_sql_to_endpoint(&ep, sql, 1).await?;
    assert_eq!(status, StatusCode::OK, "{:?}", result);
    assert!(result.error.is_some(), "{:?}", result);
    assert!(result.next_uri.is_none(), "{:?}", result);

    let sql = "select query_text, exception_code, exception_text, stack_trace  from system.query_log where log_type=3";
    let (status, result) = post_sql_to_endpoint(&ep, sql, 1).await?;
    assert_eq!(status, StatusCode::OK, "{:?}", result);
    assert_eq!(result.data.len(), 1, "{:?}", result);
    assert!(
        result.data[0][0].as_str().unwrap().contains("create table"),
        "{:?}",
        result
    );
    assert!(
        result.data[0][2]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("exist"),
        "{:?}",
        result
    );
    assert_eq!(
        result.data[0][1].as_u64().unwrap(),
        ErrorCode::TableAlreadyExists("").code().to_u64().unwrap(),
        "{:?}",
        result
    );
    assert!(
        result.data[0][3].as_str().unwrap().to_lowercase().contains(
            if std::env::var("RUST_BACKTRACE").is_ok() {
                "backtrace"
            } else {
                "<disabled>"
            }
        ),
        "{:?}",
        result
    );

    let session_manager = SessionManagerBuilder::create().build().unwrap();
    let ep = Route::new()
        .nest("/v1/query", query_route())
        .with(HTTPSessionMiddleware { session_manager });

    let sql = "select sleep(2)";
    let json = serde_json::json!({"sql": sql.to_string(), "pagination": {"wait_time_secs": 0}});
    let (status, result) = post_json_to_endpoint(&ep, &json).await?;
    assert_eq!(status, StatusCode::OK, "{:?}", result);
    assert!(result.error.is_none(), "{:?}", result);

    let response = get_uri(&ep, &result.kill_uri.as_ref().unwrap()).await;
    assert_eq!(response.status(), StatusCode::OK, "{:?}", result);

    let sql = "select query_text, exception_code, exception_text, stack_trace from system.query_log where log_type=4";
    let (status, result) = post_sql_to_endpoint(&ep, sql, 1).await?;
    assert_eq!(status, StatusCode::OK, "{:?}", result);
    assert_eq!(result.data.len(), 1, "{:?}", result);
    assert!(
        result.data[0][0].as_str().unwrap().contains("sleep"),
        "{:?}",
        result
    );
    assert!(
        result.data[0][2]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("aborted"),
        "{:?}",
        result
    );
    assert_eq!(
        result.data[0][1].as_u64().unwrap(),
        ErrorCode::AbortedQuery("").code().to_u64().unwrap(),
        "{:?}",
        result
    );
    Ok(())
}

async fn delete_query(ep: &EndpointType, query_id: &str) -> StatusCode {
    let uri = make_final_uri(query_id);
    let resp = get_uri(ep, &uri).await;
    resp.status()
}

async fn check_response(response: Response) -> Result<(StatusCode, QueryResponse)> {
    let status = response.status();
    let body = response.into_body().into_string().await.unwrap();
    let result = serde_json::from_str::<QueryResponse>(&body);
    assert!(
        result.is_ok(),
        "body ='{}', result='{:?}'",
        &body,
        result.err()
    );
    Ok((status, result?))
}

async fn get_uri(ep: &EndpointType, uri: &str) -> Response {
    let basic = headers::Authorization::basic("root", "");
    ep.call(
        Request::builder()
            .uri(uri.parse().unwrap())
            .method(Method::GET)
            .typed_header(basic)
            .finish(),
    )
    .await
    .unwrap_or_else(|err| err.as_response())
}

async fn get_uri_checked(ep: &EndpointType, uri: &str) -> Result<(StatusCode, QueryResponse)> {
    let response = get_uri(ep, uri).await;
    check_response(response).await
}

async fn post_sql(sql: &str, wait_time_secs: u64) -> Result<(StatusCode, QueryResponse)> {
    let json = serde_json::json!({"sql": sql.to_string(), "pagination": {"wait_time_secs": wait_time_secs}});
    post_json(&json).await
}

pub fn create_endpoint() -> EndpointType {
    let session_manager = SessionManagerBuilder::create().build().unwrap();
    Route::new()
        .nest("/v1/query", query_route())
        .with(HTTPSessionMiddleware { session_manager })
}

async fn post_json(json: &serde_json::Value) -> Result<(StatusCode, QueryResponse)> {
    let ep = create_endpoint();
    post_json_to_endpoint(&ep, json).await
}

async fn post_sql_to_endpoint(
    ep: &EndpointType,
    sql: &str,
    wait_time_secs: u64,
) -> Result<(StatusCode, QueryResponse)> {
    let json = serde_json::json!({"sql": sql.to_string(), "pagination": {"wait_time_secs": wait_time_secs}});
    post_json_to_endpoint(ep, &json).await
}

async fn post_json_to_endpoint(
    ep: &EndpointType,
    json: &serde_json::Value,
) -> Result<(StatusCode, QueryResponse)> {
    let uri = "/v1/query";
    let content_type = "application/json";
    let body = serde_json::to_vec(&json)?;
    let basic = headers::Authorization::basic("root", "");

    let req = Request::builder()
        .uri(uri.parse().unwrap())
        .method(Method::POST)
        .header(header::CONTENT_TYPE, content_type)
        .typed_header(basic)
        .body(body);
    let response = ep
        .call(req)
        .await
        .map_err(|e| ErrorCode::UnexpectedError(e.to_string()))?;

    check_response(response).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore]
async fn test_auth_basic() -> Result<()> {
    let user_name = "user1";
    let password = "password";

    let ep = create_endpoint();
    let sql = format!("create user {} identified by {}", user_name, password);
    post_sql_to_endpoint(&ep, &sql, 1).await?;

    let basic = headers::Authorization::basic(user_name, password);
    test_auth_post(&ep, user_name, basic).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_auth_jwt() -> Result<()> {
    let user_name = "root";

    let kid = "test_kid";
    let key_pair = RS256KeyPair::generate(2048)?.with_key_id(kid);
    let rsa_components = key_pair.public_key().to_components();
    let e = encode_config(rsa_components.e, URL_SAFE_NO_PAD);
    let n = encode_config(rsa_components.n, URL_SAFE_NO_PAD);
    let j =
        serde_json::json!({"keys": [ {"kty": "RSA", "kid": kid, "e": e, "n": n, } ] }).to_string();

    let server = MockServer::start().await;
    let json_path = "/jwks.json";
    // Create a mock on the server.
    let template = ResponseTemplate::new(200).set_body_raw(j, "application/json");
    Mock::given(method("GET"))
        .and(path(json_path))
        .respond_with(template)
        .expect(1..)
        // Mounting the mock on the mock server - it's now effective!
        .mount(&server)
        .await;
    let jwks_url = format!("http://{}{}", server.address(), json_path);

    let session_manager = SessionManagerBuilder::create()
        .jwt_key_file(jwks_url)
        .build()
        .unwrap();
    let ep = Route::new()
        .nest("/v1/query", query_route())
        .with(HTTPSessionMiddleware { session_manager });

    let now = Some(Clock::now_since_epoch());
    let claims = JWTClaims {
        issued_at: now,
        expires_at: Some(now.unwrap() + jwt_simple::prelude::Duration::from_secs(10)),
        invalid_before: now,
        audiences: None,
        issuer: None,
        jwt_id: None,
        subject: Some(user_name.to_string()),
        nonce: None,
        custom: NoCustomClaims {},
    };

    let token = key_pair.sign(claims)?;
    let bear = headers::Authorization::bearer(&token).unwrap();
    test_auth_post(&ep, user_name, bear).await?;
    Ok(())
}

async fn test_auth_post(ep: &EndpointType, user_name: &str, header: impl Header) -> Result<()> {
    let sql = "select current_user()";

    let json = serde_json::json!({"sql": sql.to_string()});

    let path = "/v1/query";
    let uri = format!("{}?wait_time_secs={}", path, 3);
    let content_type = "application/json";
    let body = serde_json::to_vec(&json)?;

    let response = ep
        .call(
            Request::builder()
                .uri(uri.parse().unwrap())
                .method(Method::POST)
                .header(header::CONTENT_TYPE, content_type)
                .typed_header(header)
                .body(body),
        )
        .await
        .unwrap();

    let (_, resp) = check_response(response).await?;
    let v = resp.data;
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].len(), 1);
    assert_eq!(
        v[0][0],
        serde_json::Value::String(format!("'{}'@'%'", user_name))
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_auth_jwt_with_create_user() -> Result<()> {
    let user_name = "user1";

    let kid = "test_kid";
    let key_pair = RS256KeyPair::generate(2048)?.with_key_id(kid);
    let rsa_components = key_pair.public_key().to_components();
    let e = encode_config(rsa_components.e, URL_SAFE_NO_PAD);
    let n = encode_config(rsa_components.n, URL_SAFE_NO_PAD);
    let j =
        serde_json::json!({"keys": [ {"kty": "RSA", "kid": kid, "e": e, "n": n, } ] }).to_string();

    let server = MockServer::start().await;
    let json_path = "/jwks.json";
    // Create a mock on the server.
    let template = ResponseTemplate::new(200).set_body_raw(j, "application/json");
    Mock::given(method("GET"))
        .and(path(json_path))
        .respond_with(template)
        .expect(1..)
        // Mounting the mock on the mock server - it's now effective!
        .mount(&server)
        .await;
    let jwks_url = format!("http://{}{}", server.address(), json_path);

    let session_manager = SessionManagerBuilder::create()
        .jwt_key_file(jwks_url)
        .build()
        .unwrap();

    let ep = Route::new()
        .nest("/v1/query", query_route())
        .with(HTTPSessionMiddleware { session_manager });

    let now = Some(Clock::now_since_epoch());
    let claims = JWTClaims {
        issued_at: now,
        expires_at: Some(now.unwrap() + jwt_simple::prelude::Duration::from_secs(10)),
        invalid_before: now,
        audiences: None,
        issuer: None,
        jwt_id: None,
        subject: Some(user_name.to_string()),
        nonce: None,
        custom: CustomClaims {
            tenant_id: None,
            ensure_user: Some(EnsureUser::default()),
        },
    };

    let token = key_pair.sign(claims)?;
    let bear = headers::Authorization::bearer(&token).unwrap();
    test_auth_post(&ep, user_name, bear).await?;
    Ok(())
}

// need to support local_addr, but axum_server do not have local_addr callback
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_http_handler_tls_server() -> Result<()> {
    let address_str = format!("127.0.0.1:{}", get_free_tcp_port());
    let mut srv = HttpHandler::create(
        SessionManagerBuilder::create()
            .http_handler_tls_server_key(TEST_SERVER_KEY)
            .http_handler_tls_server_cert(TEST_SERVER_CERT)
            .build()?,
    );

    let listening = srv.start(address_str.parse()?).await?;

    // test cert is issued for "localhost"
    let url = format!("https://{}:{}/v1/query", TEST_CN_NAME, listening.port());
    let sql = "select * from system.tables limit 10";
    let json = serde_json::json!({"sql": sql.to_string()});
    // load cert
    let mut buf = Vec::new();
    File::open(TEST_CA_CERT)?.read_to_end(&mut buf)?;
    let cert = reqwest::Certificate::from_pem(&buf).unwrap();

    // kick off
    let client = reqwest::Client::builder()
        .add_root_certificate(cert)
        .build()
        .unwrap();
    let resp = client
        .post(&url)
        .json(&json)
        .basic_auth("root", Some(""))
        .send()
        .await;
    assert!(resp.is_ok(), "{:?}", resp.err());
    let resp = resp.unwrap();
    assert!(resp.status().is_success());
    let res = resp.json::<QueryResponse>().await;
    assert!(res.is_ok(), "{:?}", res);
    let res = res.unwrap();
    assert!(!res.data.is_empty(), "{:?}", res);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_http_handler_tls_server_failed_case_1() -> Result<()> {
    let address_str = format!("127.0.0.1:{}", get_free_tcp_port());
    let mut srv = HttpHandler::create(
        SessionManagerBuilder::create()
            .http_handler_tls_server_key(TEST_SERVER_KEY)
            .http_handler_tls_server_cert(TEST_SERVER_CERT)
            .build()?,
    );

    let listening = srv.start(address_str.parse()?).await?;

    // test cert is issued for "localhost"
    let url = format!("https://{}:{}/v1/query", TEST_CN_NAME, listening.port());
    let sql = "select * from system.tables limit 10";
    let json = serde_json::json!({"sql": sql.to_string()});

    // kick off
    let client = reqwest::Client::builder().build().unwrap();
    let resp = client.post(&url).json(&json).send().await;
    assert!(resp.is_err(), "{:?}", resp.err());
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_http_service_tls_server_mutual_tls() -> Result<()> {
    let address_str = format!("127.0.0.1:{}", get_free_tcp_port());
    let mut srv = HttpHandler::create(
        SessionManagerBuilder::create()
            .http_handler_tls_server_key(TEST_TLS_SERVER_KEY)
            .http_handler_tls_server_cert(TEST_TLS_SERVER_CERT)
            .http_handler_tls_server_root_ca_cert(TEST_TLS_CA_CERT)
            .build()?,
    );
    let listening = srv.start(address_str.parse()?).await?;

    // test cert is issued for "localhost"
    let url = format!("https://{}:{}/v1/query", TEST_CN_NAME, listening.port());
    let sql = "select * from system.tables limit 10";
    let json = serde_json::json!({"sql": sql.to_string()});

    // get identity
    let mut buf = Vec::new();
    File::open(TEST_TLS_CLIENT_IDENTITY)?.read_to_end(&mut buf)?;
    let pkcs12 = reqwest::Identity::from_pkcs12_der(&buf, TEST_TLS_CLIENT_PASSWORD).unwrap();
    let mut buf = Vec::new();
    File::open(TEST_TLS_CA_CERT)?.read_to_end(&mut buf)?;
    let cert = reqwest::Certificate::from_pem(&buf).unwrap();
    // kick off
    let client = reqwest::Client::builder()
        .identity(pkcs12)
        .add_root_certificate(cert)
        .build()
        .expect("preconfigured rustls tls");
    let resp = client
        .post(&url)
        .json(&json)
        .basic_auth("root", Some(""))
        .send()
        .await;
    assert!(resp.is_ok(), "{:?}", resp.err());
    let resp = resp.unwrap();
    assert!(resp.status().is_success(), "{:?}", resp);
    let res = resp.json::<QueryResponse>().await;
    assert!(res.is_ok(), "{:?}", res);
    let res = res.unwrap();
    assert!(!res.data.is_empty(), "{:?}", res);
    Ok(())
}

// cannot connect with server unless it have CA signed identity
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_http_service_tls_server_mutual_tls_failed() -> Result<()> {
    let address_str = format!("127.0.0.1:{}", get_free_tcp_port());
    let mut srv = HttpHandler::create(
        SessionManagerBuilder::create()
            .http_handler_tls_server_key(TEST_TLS_SERVER_KEY)
            .http_handler_tls_server_cert(TEST_TLS_SERVER_CERT)
            .http_handler_tls_server_root_ca_cert(TEST_TLS_CA_CERT)
            .build()?,
    );
    let listening = srv.start(address_str.parse()?).await?;

    // test cert is issued for "localhost"
    let url = format!("https://{}:{}/v1/query", TEST_CN_NAME, listening.port());
    let sql = "select * from system.tables limit 10";
    let json = serde_json::json!({"sql": sql.to_string()});

    let mut buf = Vec::new();
    File::open(TEST_TLS_CA_CERT)?.read_to_end(&mut buf)?;
    let cert = reqwest::Certificate::from_pem(&buf).unwrap();
    // kick off
    let client = reqwest::Client::builder()
        .add_root_certificate(cert)
        .build()
        .expect("preconfigured rustls tls");
    let resp = client.post(&url).json(&json).send().await;
    assert!(resp.is_err(), "{:?}", resp.err());
    Ok(())
}

pub async fn download(ep: &EndpointType, query_id: &str) -> Response {
    let uri = format!("/v1/query/{}/download", query_id);
    let resp = get_uri(ep, &uri).await;
    resp
}

#[tokio::test]
async fn test_download() -> Result<()> {
    let ep = create_endpoint();

    let sql = "select number, number + 1 from numbers(2)";
    let (status, result) = post_sql_to_endpoint(&ep, sql, 1).await?;
    assert_eq!(status, StatusCode::OK, "{:?}", result);
    assert_eq!(result.data.len(), 2, "{:?}", result);
    assert_eq!(result.state, ExecuteStateKind::Succeeded, "{:?}", result);

    // succeeded query
    let resp = download(&ep, &result.id).await;
    assert_eq!(resp.status(), StatusCode::OK, "{:?}", resp);
    let exp = "0,1\n1,2\n";
    assert_eq!(resp.into_body().into_string().await.unwrap(), exp);

    // not exist
    let mut resp = download(&ep, "123").await;
    let exp = "not exists";
    assert!(resp.take_body().into_string().await.unwrap().contains(exp));
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{:?}", result);

    // basic check for formats
    let query_id = &result.id;
    for (fmt, formatted) in [
        ("tsv", "0\t1\n1\t2\n"),
        ("csv", "0,1\n1,2\n"),
        (
            "ndjson",
            "{\"number\":0,\"(number + 1)\":1}\n{\"number\":1,\"(number + 1)\":2}\n",
        ),
        ("values", "(0,1),(1,2)"),
    ] {
        let uri = format!("/v1/query/{}/download?format={}", query_id, fmt);
        let resp = get_uri(&ep, &uri).await;
        assert_eq!(resp.status(), StatusCode::OK, "{} {}", fmt, formatted);
        assert_eq!(
            resp.into_body().into_string().await.unwrap(),
            formatted,
            "{}",
            fmt
        );
    }
    Ok(())
}

#[tokio::test]
async fn test_download_non_select() -> Result<()> {
    let ep = create_endpoint();
    let sql = "show databases";
    let (status, result) = post_sql_to_endpoint(&ep, sql, 1).await?;
    assert_eq!(status, StatusCode::OK, "{:?}", result);
    assert!(result.error.is_none(), "{:?}", result);
    let num_row = result.data.len();

    let resp = download(&ep, &result.id).await;
    assert_eq!(resp.status(), StatusCode::OK, "{:?}", result);
    let body = resp.into_body().into_string().await.unwrap();
    assert_eq!(
        body.split('\n').filter(|x| !x.is_empty()).count(),
        num_row,
        "{:?}",
        result
    );
    Ok(())
}

#[tokio::test]
async fn test_download_failed() -> Result<()> {
    let ep = create_endpoint();
    let sql = "xxx";
    let (status, result) = post_sql_to_endpoint(&ep, sql, 1).await?;
    assert_eq!(status, StatusCode::OK, "{:?}", result);

    let mut resp = download(&ep, &result.id).await;
    let exp = "not exists";
    assert!(resp.take_body().into_string().await.unwrap().contains(exp));
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{:?}", result);

    Ok(())
}

#[tokio::test]
async fn test_download_killed() -> Result<()> {
    let ep = create_endpoint();

    // detached query can download result
    let sql = "select sleep(0.1)";
    let (status, result) = post_sql_to_endpoint(&ep, sql, 1).await?;
    assert_eq!(status, StatusCode::OK, "{:?}", result);
    assert_eq!(result.data.len(), 1, "{:?}", result);
    assert_eq!(result.state, ExecuteStateKind::Succeeded, "{:?}", result);

    let response = get_uri(&ep, &result.final_uri.unwrap()).await;
    assert_eq!(response.status(), StatusCode::OK, "{:?}", response);

    let resp = download(&ep, &result.id).await;
    assert_eq!(resp.status(), StatusCode::OK, "{:?}", resp);
    let exp = "0\n";
    assert_eq!(resp.into_body().into_string().await.unwrap(), exp);

    // killed query can not download result
    let sql = "select sleep(1)";
    let json = serde_json::json!({"sql": sql.to_string(), "pagination": {"wait_time_secs": 0}});
    let (status, result) = post_json_to_endpoint(&ep, &json).await?;
    assert_eq!(status, StatusCode::OK, "{:?}", result);
    let query_id = &result.id;

    let response = get_uri(&ep, &result.kill_uri.unwrap()).await;
    assert_eq!(response.status(), StatusCode::OK, "{:?}", response);

    let mut resp = download(&ep, query_id).await;
    let exp = "not exists";
    assert!(
        resp.take_body().into_string().await.unwrap().contains(exp),
        "{:?}",
        resp
    );
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{:?}", resp);

    Ok(())
}

#[tokio::test]
async fn test_func_object_keys() -> Result<()> {
    let route = create_endpoint();

    let sqls = vec![
        ("CREATE TABLE IF NOT EXISTS objects_test1(id TINYINT, obj OBJECT, var VARIANT) Engine=Fuse;", 0),
        ("INSERT INTO objects_test1 VALUES (1, parse_json('{\"a\": 1, \"b\": [1,2,3]}'), parse_json('{\"1\": 2}'));", 0),
        ("SELECT id, object_keys(obj), object_keys(var) FROM objects_test1;", 1),
    ];

    for (sql, data_len) in sqls {
        let json = serde_json::json!({"sql": sql.to_string(), "pagination": {"wait_time_secs": 3}});
        let (status, result) = post_json_to_endpoint(&route, &json).await?;
        assert_eq!(status, StatusCode::OK);
        assert!(result.error.is_none(), "{:?}", result.error);
        assert_eq!(result.data.len(), data_len);
        assert_eq!(result.state, ExecuteStateKind::Succeeded);
    }
    Ok(())
}
