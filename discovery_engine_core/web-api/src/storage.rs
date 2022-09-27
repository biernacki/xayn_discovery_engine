// Copyright 2022 Xayn AG
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

use std::{str::FromStr, time::Duration};

use chrono::{DateTime, Utc};
use ndarray::Array;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    FromRow,
    Pool,
    Postgres,
};
use uuid::Uuid;
use xayn_discovery_engine_ai::{
    CoiStats,
    Embedding,
    GenericError,
    NegativeCoi,
    PositiveCoi,
    UserInterests,
};

use crate::models::UserId;

#[derive(Debug, Clone)]
pub(crate) struct UserState {
    pool: Pool<Postgres>,
}

impl UserState {
    pub(crate) async fn connect(uri: &str) -> Result<Self, GenericError> {
        let opt = PgConnectOptions::from_str(uri)?;
        let pool = PgPoolOptions::new().connect_with(opt).await?;
        Ok(Self { pool })
    }

    pub(crate) async fn init_database(&self) -> Result<(), GenericError> {
        sqlx::migrate!("src/migrations").run(&self.pool).await?;
        Ok(())
    }

    pub(crate) async fn fetch(&self, id: &UserId) -> Result<UserInterests, GenericError> {
        let mut tx = self.pool.begin().await?;

        let cois = sqlx::query_as::<_, QueriedCoi>(
            "SELECT coi_id, is_positive, embedding, view_count, view_time_ms, last_view 
            FROM center_of_interest 
            WHERE user_id = $1",
        )
        .bind(id.as_ref())
        .fetch_all(&mut tx)
        .await?;

        tx.commit().await?;

        let (positive, negative): (Vec<_>, Vec<_>) =
            cois.into_iter().partition(|coi| coi.is_positive);

        // fine as we convert it to i32 when we store it in the database
        #[allow(clippy::cast_sign_loss)]
        let positive: Vec<_> = positive
            .into_iter()
            .map(|coi| PositiveCoi {
                id: coi.coi_id.into(),
                point: Embedding::from(Array::from_vec(coi.embedding)),
                stats: CoiStats {
                    view_count: coi.view_count as usize,
                    view_time: Duration::from_millis(coi.view_time_ms as u64),
                    last_view: coi.last_view.into(),
                },
            })
            .collect();

        let negative: Vec<_> = negative
            .into_iter()
            .map(|coi| NegativeCoi {
                id: coi.coi_id.into(),
                point: Embedding::from(Array::from_vec(coi.embedding)),
                last_view: coi.last_view.into(),
            })
            .collect();

        Ok(UserInterests { positive, negative })
    }

    pub(crate) async fn update_positive_cois<F>(
        &self,
        id: &UserId,
        update_cois: F,
    ) -> Result<(), GenericError>
    where
        F: Fn(&mut Vec<PositiveCoi>) -> &PositiveCoi + Send + Sync,
    {
        let mut tx = self.pool.begin().await?;

        // fine as we convert it to i32 when we store it in the database
        #[allow(clippy::cast_sign_loss)]
        let mut positive_cois: Vec<_> = sqlx::query_as::<_, QueriedCoi>(
            "SELECT coi_id, is_positive, embedding, view_count, view_time_ms, last_view 
            FROM center_of_interest 
            WHERE user_id = $1 AND is_positive 
            FOR UPDATE;",
        )
        .bind(id.as_ref())
        .fetch_all(&mut tx)
        .await?
        .into_iter()
        .map(|coi| PositiveCoi {
            id: coi.coi_id.into(),
            point: Embedding::from(Array::from_vec(coi.embedding)),
            stats: CoiStats {
                view_count: coi.view_count as usize,
                view_time: Duration::from_millis(coi.view_time_ms as u64),
                last_view: coi.last_view.into(),
            },
        })
        .collect();

        let updated_coi = update_cois(&mut positive_cois);
        let timestamp: DateTime<Utc> = updated_coi.stats.last_view.into();

        // bit casting to signed int is fine as we fetch them as signed int before bit casting them back to unsigned int
        // truncating to 64bit is fine as >292e+6 years is more then enough for this use-case
        #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
        sqlx::query(
            "INSERT INTO center_of_interest (coi_id, user_id, is_positive, embedding, view_count, view_time_ms, last_view) 
            VALUES ($1, $2, $3, $4, $5, $6, $7) 
            ON CONFLICT (coi_id) DO UPDATE SET 
                embedding = EXCLUDED.embedding, 
                view_count = EXCLUDED.view_count, 
                view_time_ms = EXCLUDED.view_time_ms, 
                last_view = EXCLUDED.last_view;",
        )
        .bind(updated_coi.id.as_ref())
        .bind(id.as_ref())
        .bind(true)
        .bind(updated_coi.point.to_vec())
        .bind(updated_coi.stats.view_count as i32)
        .bind(updated_coi.stats.view_time.as_millis() as i64)
        .bind(timestamp)
        .execute(&mut tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }

    pub(crate) async fn clear(&self) -> Result<bool, GenericError> {
        let mut tx = self.pool.begin().await?;

        let deletion = sqlx::query("DELETE FROM center_of_interest;")
            .execute(&mut tx)
            .await?;

        tx.commit().await?;

        Ok(deletion.rows_affected() > 0)
    }
}

#[derive(FromRow)]
struct QueriedCoi {
    coi_id: Uuid,
    is_positive: bool,
    embedding: Vec<f32>,
    /// The count is a `usize` stored as `i32` in database
    view_count: i32,
    /// The time is a `u64` stored as `i64` in database
    view_time_ms: i64,
    last_view: DateTime<Utc>,
}