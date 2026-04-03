use actix_session::Session;
use actix_web::{Error, HttpResponse, post};
use actix_web::{web::Data, web::Json};
use rust_decimal::Decimal;
use sea_orm::{ActiveValue, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::entities::{top_ups, user_session};

const TROCADOR_BASE_URL: &str = "https://trocador.app";

#[derive(Debug, Deserialize, Serialize)]
struct CreateTransactionResponse {
    #[serde(alias = "ID")]
    id: String,
    #[serde(alias = "url")]
    url: String,
}

#[derive(Debug, Serialize)]
struct PaymentProcessResponse {
    success: bool,
    payment_url: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct TrocadorStatusResponse {
    status: String,
    #[serde(default)]
    address_from: Option<String>,
    #[serde(default)]
    amount_from: Option<String>,
    #[serde(default)]
    ticker_from: Option<String>,
    #[serde(default)]
    amount_to: Option<String>,
    #[serde(default)]
    ticker_to: Option<String>,
    #[serde(default)]
    hash_to: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TrocadorWebhook {
    id: String,
    status: String,
    #[serde(default)]
    address_from: Option<String>,
    #[serde(default)]
    amount_from: Option<String>,
    #[serde(default)]
    ticker_from: Option<String>,
    #[serde(default)]
    amount_to: Option<String>,
    #[serde(default)]
    ticker_to: Option<String>,
    #[serde(default)]
    hash_to: Option<String>,
}

#[derive(Debug, Serialize)]
struct WebhookResponse {
    received: bool,
}

async fn verify_transaction_with_trocador(transaction_id: &str) -> Result<TrocadorStatusResponse, String> {
    let status_url = format!("{}/anonpay/status/{}", TROCADOR_BASE_URL, transaction_id);
    
    let client = reqwest::Client::new();
    let response = client
        .get(&status_url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!("Trocador status API returned: {}", response.status()));
    }

    let body = response.text().await.map_err(|e| e.to_string())?;
    
    log::info!(
        ">>> TROCADOR VERIFY: Status check\n\
         Transaction ID: {}\n\
         Body: {}",
        transaction_id,
        body
    );

    serde_json::from_str(&body).map_err(|e| format!("Failed to parse response: {}", e))
}

#[post("/api/payment/process/trocador")]
pub async fn api_payment_process(
    state: Data<AppState>,
    session: Session,
) -> Result<HttpResponse, Error> {
    let user_id = session.get::<String>("user_id")?;

    let user_id = match user_id {
        Some(id) => id,
        None => {
            return Ok(HttpResponse::Unauthorized().json(serde_json::json!({
                "success": false,
                "error": "Not authenticated"
            })));
        }
    };

    let xmr_address = std::env::var("TROCADOR_XMR_ADDRESS").unwrap_or_default();
    if xmr_address.is_empty() {
        log::error!("TROCADOR_XMR_ADDRESS environment variable not set");
        return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
            "success": false,
            "error": "Payment provider not configured"
        })));
    }

    let callback_url = state.trocador_callback_url.clone();

    let mut url = format!("{}/anonpay/?", TROCADOR_BASE_URL);
    url.push_str(&format!(
        "ticker_to=xmr&network_to=Mainnet&address={}&direct=False&fiat_equiv=EUR&description={}&webhook={}&editable=True",
        urlencoding::encode(&xmr_address),
        urlencoding::encode(&format!("Balance top-up for user {}", user_id)),
        urlencoding::encode(&callback_url)
    ));

    log::info!(
        ">>> TROCADOR: Creating indirect payment\n\
         URL: {}\n\
         User ID: {}",
        url,
        user_id
    );

    let client = reqwest::Client::new();
    let response = client.get(&url).send().await;

    match response {
        Ok(resp) => {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();

            log::info!(
                "<<< TROCADOR RESPONSE: Status={}\n\
                 Body: {}",
                status,
                body_text
            );

            if status.is_success() {
                let tx: CreateTransactionResponse = match serde_json::from_str(&body_text) {
                    Ok(t) => t,
                    Err(e) => {
                        log::error!("Failed to parse trocador response: {}", e);
                        return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                            "success": false,
                            "error": "Failed to parse payment response"
                        })));
                    }
                };

                log::info!(
                    ">>> TROCADOR: Creating top_up record for transaction_id={}, user_id={}",
                    tx.id,
                    user_id
                );

                let top_up = top_ups::ActiveModel {
                    external_id: Set(tx.id.clone()),
                    user_id: Set(user_id),
                    ..Default::default()
                };

                if let Err(e) = top_ups::Entity::insert(top_up).exec(&state.conn).await {
                    log::error!("Failed to insert top_up record: {}", e);
                    return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                        "success": false,
                        "error": "Failed to save deposit record"
                    })));
                }

                log::info!("<<< TROCADOR: Returning payment_url={}", tx.url);

                // ===== NOTIFICATION
                state
                    .tg_notificator
                    .notify(&format!("trocador payment request..."));
                // ===== NOTIFICATION

                return Ok(HttpResponse::Ok().json(PaymentProcessResponse {
                    success: true,
                    payment_url: tx.url,
                    message: "Payment created successfully".to_string(),
                }));
            } else {
                log::error!("Trocador API error: {}", body_text);
                return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "success": false,
                    "error": format!("Gateway error: {}", body_text)
                })));
            }
        }
        Err(e) => {
            log::error!("Failed to call trocador API: {}", e);
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Gateway connection error: {}", e)
            })));
        }
    }
}

// Trocador webhook callback
// https://trocador.app/anonpay/?...&webhook=<callback_url>
#[post("/internal/trocador-callback")]
pub async fn trocador_callback(
    state: Data<AppState>,
    body: Json<TrocadorWebhook>,
) -> Result<HttpResponse, Error> {
    log::info!(
        "<<< TROCADOR CALLBACK RECEIVED:\n\
         Transaction ID: {}\n\
         Status: {}\n\
         Amount from: {} {}\n\
         Amount to: {} {}\n\
         Hash: {:?}",
        body.id,
        body.status,
        body.amount_from.as_ref().unwrap_or(&"N/A".to_string()),
        body.ticker_from.as_ref().unwrap_or(&"N/A".to_string()),
        body.amount_to.as_ref().unwrap_or(&"N/A".to_string()),
        body.ticker_to.as_ref().unwrap_or(&"N/A".to_string()),
        body.hash_to
    );

    let transaction_id = &body.id;
    let status = &body.status;

    log::info!(
        ">>> TROCADOR: Looking up top_up record for transaction_id={}",
        transaction_id
    );

    let top_up_model = top_ups::Entity::find()
        .filter(top_ups::Column::ExternalId.eq(transaction_id))
        .one(&state.conn)
        .await
        .map_err(|e| {
            log::error!("Database error: {}", e);
            actix_web::error::ErrorInternalServerError("Database error")
        })?;

    let top_up_model = match top_up_model {
        Some(t) => {
            log::info!(
                "<<< TROCADOR: Found top_up record id={}, user_id={}, balance_claimed={}",
                t.id,
                t.user_id,
                t.balance_claimed
            );
            t
        }
        None => {
            log::warn!(
                "<<< TROCADOR: Top-up record NOT FOUND for transaction_id={}",
                transaction_id
            );
            return Ok(HttpResponse::Ok().json(WebhookResponse { received: true }));
        }
    };

    if top_up_model.balance_claimed {
        log::info!(
            "<<< TROCADOR: Balance already claimed for transaction_id={}, skipping",
            transaction_id
        );
        return Ok(HttpResponse::Ok().json(WebhookResponse { received: true }));
    }

    // Statuses: anonpaynew, waiting, confirming, sending, finished, paid partially, failed, expired, halted, refunded
    if status == "finished" || status == "confirming" || status == "sending" {
        log::info!(
            ">>> TROCADOR: Received {} webhook, verifying with Trocador API...",
            status
        );

        let verified = match verify_transaction_with_trocador(transaction_id).await {
            Ok(v) => v,
            Err(e) => {
                log::error!(
                    "<<< TROCADOR: Failed to verify transaction {}: {}",
                    transaction_id,
                    e
                );
                return Ok(HttpResponse::Ok().json(WebhookResponse { received: true }));
            }
        };

        let allowed_statuses = ["finished", "confirming", "sending"];
        if !allowed_statuses.contains(&verified.status.as_str()) {
            log::warn!(
                "<<< TROCADOR: Verified status '{}' not in allowed list {:?}",
                verified.status,
                allowed_statuses
            );
            return Ok(HttpResponse::Ok().json(WebhookResponse { received: true }));
        }

        let amount_from = verified
            .amount_from
            .as_ref()
            .and_then(|a| a.parse::<Decimal>().ok())
            .unwrap_or_default();

        if amount_from.is_zero() {
            log::warn!(
                "<<< TROCADOR: Verified amount is 0 for transaction_id={}, skipping",
                transaction_id
            );
            return Ok(HttpResponse::Ok().json(WebhookResponse { received: true }));
        }

        log::info!(
            "<<< TROCADOR: Verification passed. Status={}, Amount={} {}",
            verified.status,
            verified.amount_from.as_ref().unwrap_or(&"N/A".to_string()),
            verified.ticker_from.as_ref().unwrap_or(&"N/A".to_string())
        );

        let user_id = &top_up_model.user_id;

        log::info!(
            ">>> TROCADOR: Processing payment\n\
             Status: {}\n\
             Amount received: {} {}\n\
             User ID: {}",
            status,
            amount_from,
            verified.ticker_from.as_ref().unwrap_or(&"N/A".to_string()),
            user_id
        );

        let user_uuid = Uuid::parse_str(user_id).map_err(|e| {
            log::error!("Invalid user_id UUID: {}", e);
            actix_web::error::ErrorBadRequest("Invalid user_id format")
        })?;

        log::info!(">>> TROCADOR: Looking up user_id={}", user_uuid);

        let user = user_session::Entity::find_by_id(user_uuid)
            .one(&state.conn)
            .await
            .map_err(|e| {
                log::error!("Database error: {}", e);
                actix_web::error::ErrorInternalServerError("Database error")
            })?;

        if let Some(user) = user {
            log::info!("<<< TROCADOR: Found user, current balance={}", user.balance);

            let new_balance = user.balance + amount_from;

            log::info!(
                ">>> TROCADOR: Updating user balance {} + {} = {}",
                user.balance,
                amount_from,
                new_balance
            );

            let mut user_active = user.into_active_model();
            user_active.balance = ActiveValue::Set(new_balance);

            user_session::Entity::update(user_active)
                .exec(&state.conn)
                .await
                .map_err(|e| {
                    log::error!("Failed to update user balance: {}", e);
                    actix_web::error::ErrorInternalServerError("Failed to update balance")
                })?;

            log::info!(
                "<<< TROCADOR: User balance updated successfully. New balance: {}",
                new_balance
            );

            log::info!(">>> TROCADOR: Updating top_up record, marking balance_claimed=true");

            let mut top_up_active = top_up_model.into_active_model();
            top_up_active.balance_claimed = ActiveValue::Set(true);
            top_up_active.external_status = ActiveValue::Set(Some(status.clone()));
            top_up_active.amount_paid = ActiveValue::Set(body.amount_from.clone());
            top_up_active.updated_at = ActiveValue::Set(Some(chrono::Utc::now().naive_utc()));

            top_ups::Entity::update(top_up_active)
                .exec(&state.conn)
                .await
                .map_err(|e| {
                    log::error!("Failed to update top_up record: {}", e);
                    actix_web::error::ErrorInternalServerError("Failed to update deposit status")
                })?;

            log::info!(
                "<<< TROCADOR: FINISHED payment processed successfully for transaction_id={}",
                transaction_id
            );

            // ===== NOTIFICATION
            state
                .tg_notificator
                .notify(&format!("trocador payment received!",));
            // ===== NOTIFICATION
            //
        } else {
            log::error!("TROCADOR: User not found for user_id={}", user_id);
        }
    } else if status == "failed"
        || status == "expired"
        || status == "halted"
        || status == "refunded"
    {
        log::info!(
            ">>> TROCADOR: Payment {} for transaction_id={}",
            status.to_uppercase(),
            transaction_id
        );

        let mut top_up_active = top_up_model.into_active_model();
        top_up_active.external_status = ActiveValue::Set(Some(status.to_uppercase()));
        top_up_active.updated_at = ActiveValue::Set(Some(chrono::Utc::now().naive_utc()));

        top_ups::Entity::update(top_up_active)
            .exec(&state.conn)
            .await
            .ok();

        log::info!(
            "<<< TROCADOR: {} payment recorded for transaction_id={}",
            status,
            transaction_id
        );
    } else {
        log::info!(
            "<<< TROCADOR: Status '{}' for transaction_id={}, acknowledging",
            status,
            transaction_id
        );
    }

    log::info!(">>> TROCADOR: Sending 200 OK response");
    Ok(HttpResponse::Ok().json(WebhookResponse { received: true }))
}
