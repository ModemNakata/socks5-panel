use actix_session::Session;
use actix_web::web::{Data, Json};
use actix_web::{Error, HttpResponse, post};
use sea_orm::{ActiveValue, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::entities::{top_ups, user_session};

#[derive(Serialize)]
struct CreateDepositRequest {
    currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    network: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notify_url: Option<String>,
}

#[derive(Deserialize)]
struct CreateDepositResponse {
    deposit_uuid: String,
    wallet_address: String,
    currency: String,
    #[serde(default)]
    checkout_page: Option<String>,
}

#[derive(Deserialize)]
struct CheckDepositResponse {
    deposit_uuid: String,
    wallet_address: String,
    amount_received: String,
    payment_status: String,
    is_finalized: bool,
    #[serde(default)]
    confirmations: Option<i32>,
    #[serde(default)]
    txid: Option<String>,
    fiat_amount: Option<String>,
}

#[derive(Serialize)]
struct PaymentProcessResponse {
    success: bool,
    payment_url: String,
    message: String,
}

#[derive(Deserialize)]
struct PaymentStatusUpdateRequest {
    deposit_uuid: String,
    payment_status: String,
    amount_received: String,
}

#[post("/api/payment/process/cryptowrap")]
pub async fn api_payment_process(
    state: Data<AppState>,
    session: Session,
) -> Result<HttpResponse, Error> {
    // Get user_id from session
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

    // Create deposit request payload
    let deposit_request = CreateDepositRequest {
        currency: "XMR".to_string(),
        network: None,
        notify_url: Some(state.notify_url.clone()),
    };

    // Call CryptoWrap API to create deposit
    let client = reqwest::Client::new();
    let create_url = format!("{}/api/v1/deposit/create", state.gateway_url);

    let response = client
        .post(&create_url)
        .header("X-API-Key", &state.gateway_api_key)
        .json(&deposit_request)
        .send()
        .await;

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                let deposit: CreateDepositResponse = resp.json().await.unwrap();

                // Insert deposit record into top_ups table
                // Generate UUID in backend for the top_up record
                let top_up_id = Uuid::new_v4();
                
                let top_up = top_ups::ActiveModel {
                    id: Set(top_up_id),
                    external_id: Set(Some(deposit.deposit_uuid.clone())),
                    user_id: Set(user_id),
                    // external_status: Set("waiting".to_string()), // Some()
                    ..Default::default()
                };

                match top_ups::Entity::insert(top_up).exec(&state.conn).await {
                    Ok(_) => {
                        // Use checkout_page from response
                        let payment_url = deposit.checkout_page.unwrap();
                        // let payment_url = deposit.checkout_page.unwrap_or_else(|| {
                        // format!("{}/checkout?uuid={}", state.gateway_url, deposit.deposit_uuid)
                        // });

                        return Ok(HttpResponse::Ok().json(PaymentProcessResponse {
                            success: true,
                            payment_url,
                            message: "Payment created successfully".to_string(),
                        }));
                    }
                    Err(e) => {
                        log::error!("Failed to insert top_up record: {}", e);
                        return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                            "success": false,
                            "error": "Failed to save deposit record"
                        })));
                    }
                }
            } else {
                let error_text = resp.text().await.unwrap_or_default();
                log::error!("Gateway API error: {}", error_text);
                return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "success": false,
                    "error": format!("Gateway error: {}", error_text)
                })));
            }
        }
        Err(e) => {
            log::error!("Failed to call gateway API: {}", e);
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Gateway connection error: {}", e)
            })));
        }
    }
}

// Notification endpoint - called by payment gateway on status change
#[post("/internal/payment_status_update")]
pub async fn payment_status_update(
    state: Data<AppState>,
    body: Json<PaymentStatusUpdateRequest>,
) -> Result<HttpResponse, Error> {
    let deposit_uuid = &body.deposit_uuid;
    // let new_status = &body.payment_status;

    // Find the top_up record by external_id
    let top_up_model = top_ups::Entity::find()
        .filter(top_ups::Column::ExternalId.eq(deposit_uuid))
        .one(&state.conn)
        .await
        .map_err(|e| {
            log::error!("Database error: {}", e);
            actix_web::error::ErrorInternalServerError("Database error")
        })?;

    let top_up_model = match top_up_model {
        Some(t) => t,
        None => {
            log::warn!("Top-up record not found for deposit_uuid: {}", deposit_uuid);
            return Ok(HttpResponse::NotFound().json(serde_json::json!({
                "success": false,
                "error": "Deposit record not found"
            })));
        }
    };

    // Check if balance has already been claimed
    if top_up_model.balance_claimed {
        return Ok(HttpResponse::Ok().json(serde_json::json!({
            "success": true,
            "message": "Balance claimed already, nothing to do"
        })));
    }

    // If status is detected or confirmed, verify with gateway first
    // Always verify with gateway to get trusted amount
    // can also verify only if received `detected` or `confirmed` ?()
    let verified = verify_deposit_status(deposit_uuid, &state).await?;
    let verified_status = verified.payment_status;
    let verified_amount = verified.fiat_amount.unwrap_or_else(|| "0".to_string()); // only fiat eur is acceptable, if null/none ->>> 0 as/in string
    // if we have detected or confirmed - it means verified_amount must be > 0
    // can add additional check for it

    // Check if we should add balance (detected or confirmed)
    if verified_status == "detected" || verified_status == "confirmed" {
        let amount: rust_decimal::Decimal = match verified_amount.parse() {
            Ok(a) => a,
            Err(e) => {
                log::error!("Failed to parse amount '{}': {}", verified_amount, e);
                return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "success": false,
                    "error": "Invalid amount format"
                })));
            }
        };

        // Get user and increase balance
        let user_uuid = Uuid::parse_str(&top_up_model.user_id).map_err(|e| {
            log::error!("Invalid user_id UUID: {}", e);
            actix_web::error::ErrorBadRequest("Invalid user_id format")
        })?;

        let user = user_session::Entity::find_by_id(user_uuid)
            .one(&state.conn)
            .await
            .map_err(|e| {
                log::error!("Database error: {}", e);
                actix_web::error::ErrorInternalServerError("Database error")
            })?;

        if let Some(user) = user {
            let new_balance = user.balance + amount;

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
                "Balance increased by {} for user {}. New balance: {}",
                amount,
                user_uuid,
                new_balance
            );

            // Update top_up with balance_claimed = true
            let mut top_up_active = top_up_model.into_active_model();
            top_up_active.balance_claimed = ActiveValue::Set(true);
            top_up_active.external_status = ActiveValue::Set(Some(verified_status));
            top_up_active.amount_paid = ActiveValue::Set(Some(verified_amount));
            top_up_active.updated_at = ActiveValue::Set(Some(chrono::Utc::now().naive_utc()));

            top_ups::Entity::update(top_up_active)
                .exec(&state.conn)
                .await
                .map_err(|e| {
                    log::error!("Failed to update top_up record: {}", e);
                    actix_web::error::ErrorInternalServerError("Failed to update deposit status")
                })?;

            return Ok(HttpResponse::Accepted().json(serde_json::json!({
                "success": true,
                "message": "Status updated successfully"
            })));
        }
        // handle is user not found?
    }

    // No need to update database if status is something else except `detected` and `confirmed`, just say that it understood the request
    // 202
    //
    // Update external_status and amount_paid only (using verified data)
    // let mut top_up_active = top_up_model.into_active_model();
    // top_up_active.external_status = ActiveValue::Set(Some(verified_status));
    // top_up_active.amount_paid = ActiveValue::Set(Some(verified_amount));
    // top_up_active.updated_at = ActiveValue::Set(Some(chrono::Utc::now().naive_utc()));

    // top_ups::Entity::update(top_up_active)
    //     .exec(&state.conn)
    //     .await
    //     .map_err(|e| {
    //         log::error!("Failed to update top_up record: {}", e);
    //         actix_web::error::ErrorInternalServerError("Failed to update deposit status")
    //     })?;

    Ok(HttpResponse::Accepted().json(serde_json::json!({
        "success": true,
        "message": "ACCEPTED."
    }))) // !#
}

async fn verify_deposit_status(
    deposit_uuid: &str,
    state: &Data<AppState>,
) -> Result<CheckDepositResponse, Error> {
    let client = reqwest::Client::new();
    let check_url = format!(
        "{}/api/v1/deposit/check?deposit_uuid={}&price_to=eur", // and convert to EUR
        state.gateway_url, deposit_uuid
    );

    let response = client
        .get(&check_url)
        .header("X-API-Key", &state.gateway_api_key)
        .send()
        .await
        .map_err(|e| {
            log::error!("Failed to call gateway check API: {}", e);
            actix_web::error::ErrorBadRequest("Failed to verify deposit status")
        })?;

    if response.status().is_success() {
        let deposit_info: CheckDepositResponse = response.json().await.unwrap();
        Ok(deposit_info)
    } else {
        log::error!("Gateway check failed: {}", response.status());
        Err(actix_web::error::ErrorBadRequest(
            "Failed to verify deposit status",
        ))
    }
}
