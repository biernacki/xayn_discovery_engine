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

use std::{collections::HashMap, time::Duration};

use actix_web::{
    web::{self, Data, Json, Path, Query, ServiceConfig},
    HttpResponse,
    Responder,
};
use futures_util::{stream::FuturesUnordered, StreamExt};
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use tracing::error;
use xayn_discovery_engine_ai::{
    compute_coi_relevances,
    nan_safe_f32_cmp,
    system_time_now,
    utils::rank,
    PositiveCoi,
};

use crate::{
    elastic::KnnSearchParams,
    error::{
        application::WithRequestIdExt,
        common::{BadRequest, InternalError, NotEnoughInteractions},
    },
    models::{DocumentId, PersonalizedDocument, UserId, UserInteractionType},
    Error,
};

use super::{AppState, PersonalizationConfig};

pub(super) fn configure_service(config: &mut ServiceConfig) {
    let scope = web::scope("/users/{user_id}")
        .service(
            web::resource("interactions")
                .route(web::patch().to(update_interactions.error_with_request_id())),
        )
        .service(
            web::resource("personalized_documents")
                .route(web::get().to(personalized_documents.error_with_request_id())),
        );

    config.service(scope);
}

/// Represents user interaction request body.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct UpdateInteractions {
    pub(crate) documents: Vec<UserInteractionData>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct UserInteractionData {
    #[serde(rename = "id")]
    pub(crate) document_id: DocumentId,
    #[serde(rename = "type")]
    pub(crate) interaction_type: UserInteractionType,
}

async fn update_interactions(
    state: Data<AppState>,
    user_id: Path<UserId>,
    Json(interactions): Json<UpdateInteractions>,
) -> Result<impl Responder, Error> {
    state.db.user_seen(&user_id).await?;

    let ids = interactions
        .documents
        .iter()
        .map(|document| &document.document_id)
        .collect_vec();
    let documents = state.elastic.get_documents_by_ids(&ids).await?;
    let embeddings = documents
        .into_iter()
        .map(|document| (document.id, document.embedding))
        .collect::<HashMap<_, _>>();

    for document in interactions.documents {
        match document.interaction_type {
            UserInteractionType::Positive => {
                //TODO for some reason this was returning a BAD_REQUEST error????
                state
                    .db
                    .update_positive_cois(&document.document_id, &user_id, |positive_cois| {
                        state.coi.log_positive_user_reaction(
                            positive_cois,
                            &embeddings[&document.document_id],
                        )
                    })
                    .await?;
            }
        }
    }

    Ok(HttpResponse::NoContent())
}

/// Represents personalized documents query params.
#[derive(Debug, Clone, Deserialize)]
struct PersonalizedDocumentsQuery {
    count: Option<usize>,
}

impl PersonalizedDocumentsQuery {
    fn document_count(&self, config: &PersonalizationConfig) -> Result<usize, Error> {
        let count = self
            .count
            .map(|count| count.min(config.max_number_documents))
            .unwrap_or(config.default_number_documents);

        if count > 0 {
            Ok(count)
        } else {
            Err(BadRequest::from("count has to be at least 1").into())
        }
    }
}

async fn personalized_documents(
    state: Data<AppState>,
    user_id: Path<UserId>,
    options: Query<PersonalizedDocumentsQuery>,
) -> Result<impl Responder, Error> {
    let document_count = options.document_count(&state.config.personalization)?;

    state.db.user_seen(&user_id).await?;

    let user_interests = state.db.fetch_interests(&user_id).await?;

    if user_interests.is_empty() {
        return Err(NotEnoughInteractions.into());
    }

    let cois = &user_interests.positive;
    let horizon = state.coi.config().horizon();
    let coi_weights = compute_coi_weights(cois, horizon);
    let cois = cois
        .iter()
        .zip(coi_weights)
        .sorted_by(|(_, a_weight), (_, b_weight)| nan_safe_f32_cmp(b_weight, a_weight))
        .collect_vec();

    let max_cois = state
        .config
        .personalization
        .max_cois_for_knn
        .min(user_interests.positive.len());
    let cois = &cois[0..max_cois];
    let weights_sum = cois.iter().map(|(_, w)| w).sum::<f32>();

    let excluded = state.db.fetch_interacted_document_ids(&user_id).await?;

    let mut document_futures = cois
        .iter()
        .map(|(coi, weight)| async {
            // weights_sum can't be zero, because coi weights will always return some weights that are > 0
            let weight = *weight / weights_sum;
            #[allow(
                // fine as max documents count is small enough
                clippy::cast_precision_loss,
                // fine as weight should be between 0 and 1
                clippy::cast_sign_loss,
                // fine as number of neighbors is small enough
                clippy::cast_possible_truncation
            )]
            let k_neighbors = (weight * document_count as f32).ceil() as usize;

            state
                .elastic
                .get_documents_by_embedding(KnnSearchParams {
                    excluded: excluded.clone(),
                    embedding: coi.point.to_vec(),
                    size: k_neighbors,
                    k_neighbors,
                    num_candidates: document_count,
                })
                .await
        })
        .collect::<FuturesUnordered<_>>();

    let mut all_documents = Vec::new();
    let mut errors = Vec::new();

    while let Some(result) = document_futures.next().await {
        match result {
            Ok(documents) => all_documents.extend(documents),
            Err(err) => {
                error!("Error fetching document: {err}");
                errors.push(err);
            }
        };
    }

    if all_documents.is_empty() && !errors.is_empty() {
        return Err(InternalError::from_message("Fetching documents failed").into());
    }

    match state.coi.score(&all_documents, &user_interests) {
        Ok(scores) => rank(&mut all_documents, &scores),
        Err(_) => {
            return Err(NotEnoughInteractions.into());
        }
    }

    let max_docs = document_count.min(all_documents.len());
    let documents = &all_documents[0..max_docs];

    Ok(Json(PersonalizedDocumentsResponse::new(documents)))
}

/// Represents response from personalized documents endpoint.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct PersonalizedDocumentsResponse {
    /// A list of documents personalized for a specific user.
    pub(crate) documents: Vec<PersonalizedDocument>,
}

impl PersonalizedDocumentsResponse {
    pub(crate) fn new(documents: impl Into<Vec<PersonalizedDocument>>) -> Self {
        Self {
            documents: documents.into(),
        }
    }
}

/// Computes [`PositiveCoi`]s weights used to determine how many documents to fetch using each centers' embedding.
fn compute_coi_weights(cois: &[PositiveCoi], horizon: Duration) -> Vec<f32> {
    let relevances = compute_coi_relevances(cois, horizon, system_time_now())
        .into_iter()
        .map(|rel| 1.0 - (-3.0 * rel).exp())
        .collect_vec();

    let rel_sum: f32 = relevances.iter().sum();
    relevances
        .iter()
        .map(|rel| {
            let res = rel / rel_sum;
            if res.is_nan() {
                // should be ok for our use-case
                #[allow(clippy::cast_precision_loss)]
                let len = cois.len() as f32;
                // len can't be zero, because we return early if we have no positive CoIs and never compute weights
                1.0f32 / len
            } else {
                res
            }
        })
        .collect()
}