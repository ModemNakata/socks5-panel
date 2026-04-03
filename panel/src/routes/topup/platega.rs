use actix_session::Session;
use actix_web::{Error, HttpResponse, post};
use actix_web::{get, HttpRequest, web::Data, web::Json, web::Path, web::Query};
use askama::Template;
use rust_decimal::Decimal;
use sea_orm::{ActiveValue, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::entities::{top_ups, user_session};

const PLATEGA_BASE_URL: &str = "https://app.platega.io";

const PAYMENT_METHOD_SBP: i32 = 2;
const PAYMENT_METHOD_CARD: i32 = 11;

#[derive(Debug, Serialize)]
struct CreatePaymentRequest {
    paymentMethod: i32,
    paymentDetails: PaymentDetails,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "return")]
    return_url: Option<String>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // failed_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<String>,
}

#[derive(Debug, Serialize)]
struct PaymentDetails {
    amount: f64,
    currency: String,
}

#[derive(Debug, Deserialize)]
struct CreatePaymentResponse {
    #[serde(alias = "transactionId")]
    transaction_id: String,
    redirect: String,
    status: String,
    #[serde(alias = "paymentMethod")]
    payment_method: String,
    #[serde(default)]
    return_url: Option<String>,
    #[serde(default)]
    payment_details: Option<serde_json::Value>,
    #[serde(default)]
    expires_in: Option<String>,
    #[serde(default)]
    merchant_id: Option<String>,
    #[serde(default)]
    usdt_rate: Option<f64>,
    #[serde(default)]
    crypto_amount: Option<f64>,
}

#[derive(Debug, Serialize)]
struct PaymentProcessResponse {
    success: bool,
    payment_url: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct ProcessQuery {
    method: Option<String>,
    amount: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PlategaCallback {
    id: String,
    amount: f64,
    currency: String,
    status: String,
    paymentMethod: i32,
    #[serde(default)]
    payload: Option<String>,
}

#[derive(Debug, Serialize)]
struct CallbackResponse {
    received: bool,
}

#[derive(Debug, Serialize)]
struct VerifyStatusResponse {
    status: String,
    balance_claimed: bool,
    amount_paid: Option<String>,
}

fn get_payment_method_id(method: &str) -> i32 {
    match method.to_lowercase().as_str() {
        "card" => PAYMENT_METHOD_CARD,
        _ => PAYMENT_METHOD_SBP,
    }
}

#[post("/api/payment/process/platega")]
pub async fn api_payment_process(
    state: Data<AppState>,
    session: Session,
    query: Query<ProcessQuery>,
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

    let method = query.method.clone().unwrap_or_else(|| "spb".to_string());
    let amount_str = query.amount.clone().unwrap_or_default();

    let amount: f64 = match amount_str.parse() {
        Ok(a) if a >= 1.0 => a,
        _ => {
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "error": "Invalid amount. Minimum is 1 RUB."
            })));
        }
    };

    let payment_method_id = get_payment_method_id(&method);

    //
    // create payment url at platega.io
    //

    // Create placeholder top_up record BEFORE calling Platega API
    // Generate UUID in backend to use immediately in return_url
    let transaction_internal_id = Uuid::new_v4();
    
    let top_up = top_ups::ActiveModel {
        id: Set(transaction_internal_id),
        external_id: Set(None),  // Will be updated with Platega's transaction_id after API response
        user_id: Set(user_id.clone()),
        balance_claimed: Set(false),
        ..Default::default()
    };

    match top_ups::Entity::insert(top_up).exec(&state.conn).await {
        Ok(_) => {
            log::info!(
                ">>> PLATEGA: Created placeholder top_up record with id={}",
                transaction_internal_id
            );
        }
        Err(e) => {
            log::error!("Failed to insert placeholder top_up record: {}", e);
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": "Failed to prepare payment record"
            })));
        }
    };

    let request = CreatePaymentRequest {
        paymentMethod: payment_method_id,
        paymentDetails: PaymentDetails {
            amount,
            currency: "RUB".to_string(),
        },
        description: format!("Balance top-up for user {}", user_id),
        return_url: Some(format!(
            "https://{}/verify_platega/{}",
            &state.domain,
            transaction_internal_id
        )),
        // failed_url: Some(format!("{}/payment/process?provider=platega", state.notify_url.replace("/internal/payment_status_update", ""))),
        payload: Some(user_id.clone()),
    };

    log::info!(
        ">>> PLATEGA REQUEST: Creating payment\n\
         URL: {}/transaction/process\n\
         Headers: {{\n\
         \t\"X-MerchantId\": \"{}\",\n\
         \t\"X-Secret\": \"[REDACTED]\"\n\
         }}\n\
         Body: {}",
        PLATEGA_BASE_URL,
        state.platega_merchant_id,
        serde_json::to_string_pretty(&request).unwrap_or_default()
    );

    let client = reqwest::Client::new();
    let create_url = format!("{}/transaction/process", PLATEGA_BASE_URL);

    let response = client
        .post(&create_url)
        .header("X-MerchantId", &state.platega_merchant_id)
        .header("X-Secret", &state.platega_secret)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await;

    match response {
        Ok(resp) => {
            let status = resp.status();
            let headers = resp.headers().clone();

            let body_text = resp.text().await.unwrap_or_default();

            log::info!(
                "<<< PLATEGA RESPONSE: Status={}\n\
                 Headers: {{",
                status
            );
            for (name, value) in headers.iter() {
                log::info!("\t{}: {:?}", name, value);
            }
            log::info!("}}\nBody: {}", body_text);

            if status.is_success() {
                let payment: CreatePaymentResponse = match serde_json::from_str(&body_text) {
                    Ok(p) => p,
                    Err(e) => {
                        log::error!("Failed to parse platega response: {}", e);
                        return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                            "success": false,
                            "error": "Failed to parse payment response"
                        })));
                    }
                };

                log::info!(
                    ">>> PLATEGA: Updating top_up record with external_id={} (platega transaction_id)",
                    payment.transaction_id
                );

                // Update the placeholder record with Platega's transaction_id
                let mut top_up_active = top_ups::ActiveModel {
                    id: Set(transaction_internal_id),
                    external_id: Set(Some(payment.transaction_id.clone())),
                    ..Default::default()
                };

                if let Err(e) = top_ups::Entity::update(top_up_active).exec(&state.conn).await {
                    log::error!("Failed to update top_up record with external_id: {}", e);
                    // Non-fatal, continue
                }

                log::info!("<<< PLATEGA: Returning payment_url={}", payment.redirect);

                // ===== NOTIFICATION
                state
                    .tg_notificator
                    .notify(&format!("new platega payment request..."));
                // ===== NOTIFICATION

                return Ok(HttpResponse::Ok().json(PaymentProcessResponse {
                    success: true,
                    payment_url: payment.redirect,
                    message: "Payment created successfully".to_string(),
                }));
            } else {
                log::error!("Platega API error: {}", body_text);
                return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "success": false,
                    "error": format!("Gateway error: {}", body_text)
                })));
            }
        }
        Err(e) => {
            log::error!("Failed to call platega API: {}", e);
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Gateway connection error: {}", e)
            })));
        }
    }
}

// https://socks5.website/internal/platega-callback

// this endpoint for platega.io payment gateway for russian cards and `SBP`
// post request accepts json, returns 200 OK
#[post("/internal/platega-callback")]
pub async fn platega_callback(
    state: Data<AppState>,
    req: HttpRequest,
    body: Json<PlategaCallback>,
) -> Result<HttpResponse, Error> {
    //
    // verify callback is from platega by checking headers
    //
    let headers = req.headers();

    let merchant_id = headers.get("X-MerchantId").and_then(|v| v.to_str().ok());
    let secret = headers.get("X-Secret").and_then(|v| v.to_str().ok());

    log::info!(
        "<<< PLATEGA CALLBACK RECEIVED:\n\
         Headers: X-MerchantId={:?}, X-Secret={:?}\n\
         Transaction ID: {}\n\
         Amount: {} {}\n\
         Status: {}\n\
         Payment Method: {}\n\
         Payload (user_id): {:?}",
        merchant_id,
        secret.map(|_| "[PROVIDED]"),
        body.id,
        body.amount,
        body.currency,
        body.status,
        body.paymentMethod,
        body.payload
    );

    if merchant_id != Some(state.platega_merchant_id.as_str()) {
        log::error!(
            "PLATEGA CALLBACK: Invalid X-MerchantId header. Expected={}, Got={:?}",
            state.platega_merchant_id,
            merchant_id
        );
        return Ok(HttpResponse::Unauthorized().json(CallbackResponse { received: false }));
    }

    if secret != Some(state.platega_secret.as_str()) {
        log::error!("PLATEGA CALLBACK: Invalid X-Secret header");
        return Ok(HttpResponse::Unauthorized().json(CallbackResponse { received: false }));
    }

    log::info!("PLATEGA CALLBACK: Headers verified successfully");

    let transaction_id = &body.id;
    let status = &body.status;
    let user_id = body.payload.clone();

    if user_id.is_none() {
        log::warn!(
            "Platega callback missing payload (user_id) - transaction_id={}",
            transaction_id
        );
        return Ok(HttpResponse::Ok().json(CallbackResponse { received: true }));
    }

    let user_id = user_id.unwrap();

    log::info!(
        ">>> PLATEGA: Looking up top_up record for transaction_id={}",
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
                "<<< PLATEGA: Found top_up record id={}, user_id={}, balance_claimed={}",
                t.id,
                t.user_id,
                t.balance_claimed
            );
            t
        }
        None => {
            log::warn!(
                "<<< PLATEGA: Top-up record NOT FOUND for transaction_id={}",
                transaction_id
            );
            return Ok(HttpResponse::Ok().json(CallbackResponse { received: true }));
        }
    };

    if top_up_model.balance_claimed {
        log::info!(
            "<<< PLATEGA: Balance already claimed for transaction_id={}, skipping",
            transaction_id
        );
        return Ok(HttpResponse::Ok().json(CallbackResponse { received: true }));
    }

    if status == "CONFIRMED" {
        let amount_rub = body.amount;
        let amount_eur = Decimal::from_f64_retain(amount_rub / 97.50).unwrap_or_default();
        // hardcoded rub to eur conversion

        log::info!(
            ">>> PLATEGA: Processing CONFIRMED payment\n\
             Amount: {:.2} RUB = {} EUR\n\
             User ID: {}",
            amount_rub,
            amount_eur,
            user_id
        );

        let user_uuid = Uuid::parse_str(&user_id).map_err(|e| {
            log::error!("Invalid user_id UUID: {}", e);
            actix_web::error::ErrorBadRequest("Invalid user_id format")
        })?;

        log::info!(">>> PLATEGA: Looking up user_id={}", user_uuid);

        let user = user_session::Entity::find_by_id(user_uuid)
            .one(&state.conn)
            .await
            .map_err(|e| {
                log::error!("Database error: {}", e);
                actix_web::error::ErrorInternalServerError("Database error")
            })?;

        if let Some(user) = user {
            log::info!("<<< PLATEGA: Found user, current balance={}", user.balance);

            let new_balance = user.balance + amount_eur;

            log::info!(
                ">>> PLATEGA: Updating user balance {} + {} = {}",
                user.balance,
                amount_eur,
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
                "<<< PLATEGA: User balance updated successfully. New balance: {}",
                new_balance
            );

            log::info!(">>> PLATEGA: Updating top_up record, marking balance_claimed=true");

            let mut top_up_active = top_up_model.into_active_model();
            top_up_active.balance_claimed = ActiveValue::Set(true);
            top_up_active.external_status = ActiveValue::Set(Some(status.clone()));
            top_up_active.amount_paid = ActiveValue::Set(Some(body.amount.to_string()));
            top_up_active.updated_at = ActiveValue::Set(Some(chrono::Utc::now().naive_utc()));

            top_ups::Entity::update(top_up_active)
                .exec(&state.conn)
                .await
                .map_err(|e| {
                    log::error!("Failed to update top_up record: {}", e);
                    actix_web::error::ErrorInternalServerError("Failed to update deposit status")
                })?;

            // ===== NOTIFICATION
            state
                .tg_notificator
                .notify(&format!("platega payment confirmed!"));
            // ===== NOTIFICATION

            log::info!(
                "<<< PLATEGA: CONFIRMED payment processed successfully for transaction_id={}",
                transaction_id
            );
        } else {
            log::error!("PLATEGA: User not found for user_id={}", user_id);
        }
    } else if status == "CANCELED" {
        log::info!(
            ">>> PLATEGA: Processing CANCELED payment for transaction_id={}",
            transaction_id
        );

        let mut top_up_active = top_up_model.into_active_model();
        top_up_active.external_status = ActiveValue::Set(Some("CANCELED".to_string()));
        top_up_active.updated_at = ActiveValue::Set(Some(chrono::Utc::now().naive_utc()));

        top_ups::Entity::update(top_up_active)
            .exec(&state.conn)
            .await
            .ok();

        log::info!(
            "<<< PLATEGA: CANCELED payment recorded for transaction_id={}",
            transaction_id
        );
    } else {
        log::info!(
            "<<< PLATEGA: Unhandled status '{}' for transaction_id={}, acknowledging",
            status,
            transaction_id
        );
    }

    log::info!(">>> PLATEGA: Sending 200 OK response");
    Ok(HttpResponse::Ok().json(CallbackResponse { received: true }))
}

#[derive(Template)]
#[template(path = "verify_platega.html")]
struct VerifyPlategaTemplate {
    transaction_internal_id: String,
    is_owner: bool,
}

#[get("/verify_platega/{transaction_internal_id}")]
pub async fn verify_platega(
    state: Data<AppState>,
    session: Session,
    path: Path<Uuid>,
) -> Result<HttpResponse, Error> {
    let transaction_internal_id = path.into_inner();

    // Fetch top_up record by internal ID (primary key = id)
    let top_up = top_ups::Entity::find_by_id(transaction_internal_id)
        .one(&state.conn)
        .await
        .map_err(|e| {
            log::error!("Database error: {}", e);
            actix_web::error::ErrorInternalServerError("Database error")
        })?;

    // Check if user is authenticated and owns this payment
    let session_user_id = session.get::<String>("user_id")?;
    
    // Determine if this user owns the payment
    let is_owner = match (&session_user_id, &top_up) {
        (Some(sid), Some(t)) => {
            log::info!(
                ">>> verify_platega: session_user_id={}, top_up.user_id={}, is_owner={}",
                sid,
                t.user_id,
                sid == &t.user_id
            );
            sid == &t.user_id
        }
        (None, Some(t)) => {
            log::info!(
                ">>> verify_platega: No session user_id for top_up.user_id={}",
                t.user_id
            );
            false
        }
        (_, None) => {
            log::warn!(
                ">>> verify_platega: No top_up record found for id={}",
                transaction_internal_id
            );
            false
        }
    };

    let template = VerifyPlategaTemplate {
        transaction_internal_id: transaction_internal_id.to_string(),
        is_owner,
    };

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(template.render().unwrap()))
}

#[get("/api/payment/verify_platega/status/{transaction_internal_id}")]
pub async fn api_verify_platega_status(
    state: Data<AppState>,
    session: Session,
    path: Path<Uuid>,
) -> Result<HttpResponse, Error> {
    let transaction_internal_id = path.into_inner();

    // Check user is authenticated
    let session_user_id = match session.get::<String>("user_id")? {
        Some(id) => id,
        None => {
            return Ok(HttpResponse::Unauthorized().json(serde_json::json!({
                "error": "Unauthorized"
            })));
        }
    };

    // Fetch top_up record by internal ID (primary key = id)
    let top_up = top_ups::Entity::find_by_id(transaction_internal_id)
        .one(&state.conn)
        .await
        .map_err(|e| {
            log::error!("Database error: {}", e);
            actix_web::error::ErrorInternalServerError("Database error")
        })?;

    // Verify ownership
    match top_up {
        Some(t) if t.user_id == *session_user_id => {
            if t.balance_claimed {
                // Payment completed, redirect to main page
                Ok(HttpResponse::Ok().json(VerifyStatusResponse {
                    status: "completed".to_string(),
                    balance_claimed: true,
                    amount_paid: t.amount_paid.clone(),
                }))
            } else if t.external_status == Some("CANCELED".to_string()) {
                Ok(HttpResponse::Ok().json(VerifyStatusResponse {
                    status: "canceled".to_string(),
                    balance_claimed: false,
                    amount_paid: None,
                }))
            } else {
                // Still waiting
                Ok(HttpResponse::Ok().json(VerifyStatusResponse {
                    status: "pending".to_string(),
                    balance_claimed: false,
                    amount_paid: t.amount_paid.clone(),
                }))
            }
        }
        _ => {
            // No record found or not owner
            Ok(HttpResponse::Ok().json(VerifyStatusResponse {
                status: "not_found".to_string(),
                balance_claimed: false,
                amount_paid: None,
            }))
        }
    }
}
