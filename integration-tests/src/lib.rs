// Copyright 2023 Xayn AG
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, version 3.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Provide various utilities for writing integration tests (mainly for web-api).
//!
//! As this is for testing many of the functions here will panic on failure instead
//! propagating an error and then panicking. We still use the `Panic` error type to
//! make it easier to change error handling in the future.
//!
//! Code in this module hard codes the dummy username and password used by local only
//! integration testing.

use std::{
    future::Future,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::{Arc, Once},
    time::Duration,
};

use anyhow::bail;
use once_cell::sync::Lazy;
use reqwest::{Client, Request, Response, StatusCode, Url};
use scopeguard::{guard_on_success, OnSuccess, ScopeGuard};
use serde::de::DeserializeOwned;
use toml::Table;
use xayn_ai_test_utils::{env::clear_env, error::Panic};
use xayn_web_api::{config, start, AppHandle, Application};

/// Absolute path to the root of the project as determined by `just`.
pub static PROJECT_ROOT: Lazy<PathBuf> =
    Lazy::new(|| just(&["_test-project-root"]).unwrap().into());

/// Runs `just` with given arguments returning `stdout` as string.
///
/// If just outputs non utf-8 bytes or can't be called or fails this
/// will panic.
///
/// This will capture stdout, but not stderr so warnings, errors, traces
/// and similar will be printed like normal. In case it fails it will also
/// print the previously captured stdout.
pub fn just(args: &[&str]) -> Result<String, anyhow::Error> {
    let Output { status, stdout, .. } = Command::new("just")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()?;

    let output = String::from_utf8(stdout)?;
    if status.success() {
        Ok(output)
    } else {
        bail!("Cmd `just` failed! Output:\n{output}Just Exit Status: {status}");
    }
}

pub async fn send_assert(client: &Client, req: Request, expected: StatusCode) -> Response {
    let method = req.method().clone();
    let target = req.url().clone();
    let response = client.execute(req).await.unwrap();
    let status = response.status();
    if status != expected {
        let bytes = response.bytes().await.unwrap();
        let text = String::from_utf8_lossy(&bytes);
        panic!("Failed to {method} {target}, status {status} instead of {expected}. Body:\n{text}");
    }
    response
}

pub async fn send_assert_json<O>(client: &Client, req: Request, expected: StatusCode) -> O
where
    O: DeserializeOwned,
{
    let method = req.method().clone();
    let target = req.url().clone();
    let response = send_assert(client, req, expected).await;
    let bytes = response.bytes().await.unwrap();
    match serde_json::from_slice::<O>(&bytes) {
        Ok(out) => out,
        Err(err) => {
            let text = String::from_utf8_lossy(&bytes);
            panic!("Failed to decode body of {method} {target}, error: {err}\nbody: {text}")
        }
    }
}

/// Convenience helper for setting config options.
///
/// The paths must at any point refer to a table.
/// Setting array elements is not supported.
///
/// Automatically inserts empty tables as necessary.
///
/// Works with both `Table` and `&mut Table`.
///
/// ```
/// # use integration_tests::set_config_option;
/// # use toml::{toml, Table};
///
/// let mut config = Table::default();
/// set_config_option!( for config =>
///     [storage.postgres]
///     base_url = 0;
///
///     [storage.elastic]
///     url = "hy";
///     index = vec![1,2,3];
///
///     [embedding]
///     directory = "../assets/smbert_v0003";
/// );
///
/// assert_eq!(config, toml! {
///     [storage.postgres]
///     base_url = 0
///
///     [storage.elastic]
///     url = "hy"
///     index = [1,2,3]
///
///     [embedding]
///     directory = "../assets/smbert_v0003"
/// })
/// ```
#[macro_export]
macro_rules! set_config_option {
    (for $config:ident => $(
        [$($key:ident).+]
        $($key_last:ident = $value:expr;)*
    )* $(;)?) => {$(
        let path = [$(stringify!($key)),+];
        let mut current_base: &mut Table = &mut $config;
        for sub_table_key in path {
            current_base = current_base.entry(sub_table_key.to_owned())
                .or_insert_with(|| Table::default().into())
                .as_table_mut()
                .unwrap();
        }
        $(
            current_base.insert(stringify!($key_last).to_owned(), $value.into());
        )*
    )*};
}

const APP_STOP_TIMEOUT: Duration = Duration::from_secs(1);

/// Wrapper around integration test code which makes sure they run in a semi-isolated context.
///
/// Before anything this function assures two things:
/// - the environment is cleared
/// - if not on CI the necessary services are started (Elastic Search, Postgres)
///
/// Then for each test:
///
/// - a elastic search index is created
/// - a postgres db is created
/// - a service based on `A: Application` is started on it's own port
/// - the config is pre-populated with the elastic search, embedding and postgres info
///   - you can update it using the `configure` callback
/// - the service info including an url to the application is passed to the test
pub async fn test_app<A, F>(
    configure: impl FnOnce(&mut Table),
    test: impl FnOnce(Arc<Client>, Arc<Url>, Services) -> F,
) where
    F: Future<Output = Result<(), Panic>>,
    A: Application + 'static,
{
    let services = setup_web_dev_test_context().await.unwrap();

    let handle = start_test_application::<A>(&services, configure).await;
    let client = Client::new();

    test(Arc::new(client), Arc::new(handle.url()), services.clone())
        .await
        .unwrap();

    handle.stop_and_wait(APP_STOP_TIMEOUT).await.unwrap();
}

/// Like `test_app` but runs two applications in the same test context.
pub async fn test_two_apps<A1, A2, F>(
    configure_first: impl FnOnce(&mut Table),
    configure_second: impl FnOnce(&mut Table),
    test: impl FnOnce(Arc<Client>, Arc<Url>, Arc<Url>, Services) -> F,
) where
    F: Future<Output = Result<(), Panic>>,
    A1: Application + 'static,
    A2: Application + 'static,
{
    let services = setup_web_dev_test_context().await.unwrap();
    let first_handle = start_test_application::<A1>(&services, configure_first).await;
    let second_handle = start_test_application::<A2>(&services, configure_second).await;
    test(
        Arc::new(Client::new()),
        Arc::new(first_handle.url()),
        Arc::new(second_handle.url()),
        services.clone(),
    )
    .await
    .unwrap();
    let (res1, res2) = tokio::join!(
        first_handle.stop_and_wait(APP_STOP_TIMEOUT),
        second_handle.stop_and_wait(APP_STOP_TIMEOUT),
    );
    res1.expect("first application to not fail during shutdown");
    res2.expect("second application to not fail during shutdown");
}

pub fn unchanged_config(_: &mut Table) {}

pub async fn start_test_application<A>(
    services: &Services,
    configure: impl FnOnce(&mut Table),
) -> AppHandle
where
    A: Application + 'static,
{
    let (es_url, es_index) = services.elastic_search.as_str().rsplit_once('/').unwrap();

    let mut config = Table::default();

    set_config_option!( for config =>
        [storage.postgres]
        base_url = services.postgres.as_str();

        [storage.elastic]
        url = es_url;
        index_name = es_index;

        [embedding]
        directory = "../assets/smbert_v0003";
    );

    configure(&mut config);

    let args = &[
        "integration-test",
        "--bind-to",
        "127.0.0.1:0",
        "--config",
        &format!("inline:{config}"),
    ];

    let config = config::load_with_args([0u8; 0], args);

    start::<A>(config).await.unwrap()
}

/// Generates an ID for the test.
///
/// The format is `YYMMDD_HHMMSS_RRRR` where `RRRR` is a random (16bit) 0 padded hex number.
fn generate_test_id() -> Result<String, anyhow::Error> {
    just(&["_test-generate-id"])
}

#[derive(Clone, Debug)]
pub struct Services {
    /// Id of the test.
    pub id: String,
    /// Uri to a postgres db for this test.
    pub postgres: Url,
    /// Uri to a elastic search db for this test.
    pub elastic_search: Url,
}

/// Creates a postgres db and elastic search index for running a web-dev integration test.
///
/// A uris usable for accessing the dbs are returned.
async fn setup_web_dev_test_context(
) -> Result<ScopeGuard<Services, impl FnOnce(Services), OnSuccess>, anyhow::Error> {
    clear_env();
    start_test_service_containers().unwrap();

    let id = generate_test_id()?;

    let out = just(&["_test-create-dbs", &id])?;
    let mut postgres = None;
    let mut elastic_search = None;
    for line in out.lines() {
        if let Some(url) = line.trim().strip_prefix("PG_URL=") {
            postgres = Some(url.parse().unwrap());
        } else if let Some(url) = line.trim().strip_prefix("ES_URL=") {
            elastic_search = Some(url.parse().unwrap());
        }
    }

    let uris = Services {
        id,
        postgres: postgres.unwrap(),
        elastic_search: elastic_search.unwrap(),
    };

    Ok(guard_on_success(uris, move |uris| {
        just(&["_test-drop-dbs", &uris.id]).unwrap();
    }))
}

/// Start service containers.
///
/// Does nothing on CI where they have to be started from the outside.
fn start_test_service_containers() -> Result<(), anyhow::Error> {
    static ONCE: Once = Once::new();
    let mut res = Ok(());
    ONCE.call_once(|| {
        if !std::env::var("CI")
            .map(|value| value == "true")
            .unwrap_or_default()
        {
            res = just(&["web-dev-up"]).map(drop);
        }
    });
    res
}

#[cfg(test)]
mod tests {
    use regex::Regex;
    use toml::toml;

    use super::*;

    #[test]
    fn test_random_id_generation_has_expected_format() -> Result<(), Panic> {
        let regex = Regex::new("^t[0-9]{6}_[0-9]{6}_[0-9a-f]{4}$")?;
        for _ in 0..100 {
            let id = generate_test_id().unwrap();
            assert!(
                regex.is_match(&id),
                "id does not have expected format: {id:?}",
            );
        }
        Ok(())
    }

    #[test]
    fn test_set_config_option_works() {
        let mut config = Table::default();
        set_config_option!( for config =>
            [storage.postgres]
            base_url = 0;

            [storage.elastic]
            url = "hy";
            index = vec![1,2,3];

            [embedding]
            directory = "../assets/smbert_v0003";
        );

        assert_eq!(
            config,
            toml! {
                [storage.postgres]
                base_url = 0

                [storage.elastic]
                url = "hy"
                index = [1,2,3]

                [embedding]
                directory = "../assets/smbert_v0003"
            }
        )
    }

    #[test]
    fn test_set_config_option_works_with_mut_ref() {
        let mut config = &mut Table::default();
        set_config_option!( for config =>
            [t]

            [storage.postgres]
            base_url = 0;

            [storage.elastic]
            url = "hy";

            [storage.elastic]
            index = vec![1,2,3];

            [embedding]
            directory = "../assets/smbert_v0003";
        );

        assert_eq!(
            config,
            &mut toml! {
                [t]
                [storage.postgres]
                base_url = 0

                [storage.elastic]
                url = "hy"
                index = [1,2,3]

                [embedding]
                directory = "../assets/smbert_v0003"
            }
        )
    }
}