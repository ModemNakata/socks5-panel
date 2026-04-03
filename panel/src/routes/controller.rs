use actix_session::Session;
use actix_web::web::Data;
use actix_web::web::Path;
use actix_web::{Error, HttpResponse, delete, error, get, post};
use rand::Rng;
use reqwest::Client;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use sea_orm::{ActiveValue, ColumnTrait, DbConn, EntityTrait, IntoActiveModel, QueryFilter};
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::entities::{proxy_server, rental, user_session};

const DAYS_IN_MONTH: u8 = 29;
const HOURS_IN_DAY: u8 = 24;

// creds can be generated with openssl to ensure cryptographic security
// but rust' rand is cryptographically secure
fn generate_creds() -> (String, String) {
    let mut rng = rand::thread_rng();
    let username: String = (0..8)
        .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
        .collect();
    let password: String = (0..12)
        .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
        .collect();
    (username, password)
}

fn get_user_id_from_session(session: &Session) -> Result<Uuid, Error> {
    let user_id_str = session
        .get::<String>("user_id")?
        .ok_or_else(|| error::ErrorUnauthorized("Not logged in"))?;
    Uuid::parse_str(&user_id_str).map_err(|_| error::ErrorUnauthorized("Invalid session"))
}

async fn get_user_entry_from_db(
    conn: &DbConn,
    user_uuid: Uuid,
) -> Result<user_session::Model, Error> {
    let user = user_session::Entity::find_by_id(user_uuid)
        .one(conn)
        .await
        .map_err(|e| error::ErrorInternalServerError(e))?
        .ok_or_else(|| error::ErrorUnauthorized("User not found"))?;

    Ok(user)
}

async fn add_user_to_proxy(
    codename: &str,
    domain: &str,
    controller_key: &str,
    username: &str,
    password: &str,
) -> Result<(), String> {
    let controller_url = format!("https://{}.{}/api", codename, domain);
    let client = Client::new();
    client
        .post(&format!("{}/user", controller_url))
        .json(&json!({ "username": username, "password": password }))
        .header("Authorization", format!("Bearer {}", controller_key))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    Ok(())
}
async fn remove_user_from_proxy(
    codename: &str,
    domain: &str,
    controller_key: &str,
    username: &str,
) -> Result<(), String> {
    let controller_url = format!("https://{}.{}/api", codename, domain);
    let client = Client::new();
    client
        .delete(&format!("{}/user", controller_url))
        .json(&json!({ "username": username }))
        .header("Authorization", format!("Bearer {}", controller_key))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Serialize)]
struct RentalResponse {
    id: Uuid,
    server_id: Uuid,
    server_codename: String,
    username: String,
    password: String,
    port: i32,
    country: String,
    price: String,
}
#[get("/api/rentals")]
pub async fn get_rentals(state: Data<AppState>, session: Session) -> Result<HttpResponse, Error> {
    let user_id = get_user_id_from_session(&session)?;
    let rentals = rental::Entity::find()
        .filter(rental::Column::UserId.eq(user_id))
        .filter(rental::Column::IsActive.eq(true))
        .all(&state.conn)
        .await
        .map_err(|e| error::ErrorInternalServerError(e))?;
    let mut result = Vec::new();
    for r in rentals {
        if let Some(server) = proxy_server::Entity::find_by_id(r.server_id)
            .one(&state.conn)
            .await
            .map_err(|e| error::ErrorInternalServerError(e))?
        {
            result.push(RentalResponse {
                id: r.id,
                server_id: r.server_id,
                server_codename: server.codename,
                username: r.username,
                password: r.password,
                port: server.port,
                country: server.country.clone(),
                price: server.price.to_string(),
            });
        }
    }
    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "rentals": result
    })))
}
#[post("/api/rent/{server_id}")]
pub async fn rent_server(
    state: Data<AppState>,
    session: Session,
    server_id: Path<Uuid>,
) -> Result<HttpResponse, Error> {
    let user_id = get_user_id_from_session(&session)?;

    let user = get_user_entry_from_db(&state.conn, user_id).await?;

    let server_id = server_id.into_inner();
    // TODO: Check user balance
    // + -deduct balance for 1 (first) hour
    let domain = std::env::var("DOMAIN").expect("DOMAIN not set");
    // filter added for only  ready  servers, so ppl won't be able to rent servers that are not ready
    // ^ it doesn't apply for servers / rentals that already initialized, so ppl still can stop any rent
    let server = proxy_server::Entity::find_by_id(server_id)
        .filter(proxy_server::Column::IsReady.eq(true))
        .one(&state.conn)
        .await
        .map_err(|e| error::ErrorInternalServerError(e))?
        .ok_or_else(|| error::ErrorNotFound("Server not found"))?;
    if server.slots_available <= 0 {
        return Ok(HttpResponse::Ok().json(json!({
            "success": false,
            "message": "No slots available"
        })));
    }

    // check if balance is enough to pay for first hour (pricePerMonth / 30 days / 24 hours)
    // calculate price for 1 hour that this proxy will cost
    let price_per_hour_eur =
        server.price / Decimal::from(DAYS_IN_MONTH) / Decimal::from(HOURS_IN_DAY);
    // log::info!("{:?}", price_per_hour_eur);

    // now check if user has enough balance
    if user.balance < price_per_hour_eur {
        return Ok(HttpResponse::Ok().json(json!({
            "success": false,
            "message": format!("Insufficient balance. You need at least {} EUR to rent this proxy for 1 hour.", price_per_hour_eur.round_dp(5u32))
        })));
    }

    //validate creds - username must be unique for this server, or else bad request will be triggered by controller api
    let (username, password) = generate_creds();
    if let Err(e) = add_user_to_proxy(
        &server.codename,
        &domain,
        &server.controller_key,
        &username,
        &password,
    )
    .await
    {
        return Ok(HttpResponse::Ok().json(json!({
            "success": false,
            "message": format!("Failed to create user on proxy: {}", e)
        })));
    }

    // first balance deduction of 1 hour wort of rent here
    let new_balance = user.balance - price_per_hour_eur;

    let mut user_active = user.into_active_model();
    user_active.balance = ActiveValue::Set(new_balance);

    user_session::Entity::update(user_active)
        .exec(&state.conn)
        .await
        .map_err(|e| error::ErrorInternalServerError(e))?;

    let rental_id = Uuid::new_v4();
    let new_rental = rental::ActiveModel {
        id: ActiveValue::Set(rental_id),
        user_id: ActiveValue::Set(user_id),
        server_id: ActiveValue::Set(server_id),
        username: ActiveValue::Set(username.clone()),
        password: ActiveValue::Set(password.clone()),
        ..Default::default()
    };
    rental::Entity::insert(new_rental)
        .exec(&state.conn)
        .await
        .map_err(|e| error::ErrorInternalServerError(e))?;
    let mut server_update: proxy_server::ActiveModel = server.clone().into();
    server_update.slots_available = ActiveValue::Set(server.slots_available - 1);
    server_update.proxies_rented = ActiveValue::Set(server.proxies_rented + 1);
    proxy_server::Entity::update(server_update)
        .exec(&state.conn)
        .await
        .map_err(|e| error::ErrorInternalServerError(e))?;

    // ===== NOTIFICATION
    state.tg_notificator.notify(&format!(
        "proxy rent started, server id:\n{}\nby user id:\n{}",
        server_id, user_id
    ));
    // ===== NOTIFICATION

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "message": "Server rented successfully",
        "rental_id": rental_id,
        "username": username,
        "password": password,
        "port": server.port,
        "country": server.country
    })))
}

#[delete("/api/rent/{rental_id}")]
pub async fn stop_rent(
    state: Data<AppState>,
    session: Session,
    rental_id: Path<Uuid>,
) -> Result<HttpResponse, Error> {
    let user_id = get_user_id_from_session(&session)?;
    let rental_id = rental_id.into_inner();
    let rental = rental::Entity::find_by_id(rental_id)
        .filter(rental::Column::UserId.eq(user_id))
        .filter(rental::Column::IsActive.eq(true))
        .one(&state.conn)
        .await
        .map_err(|e| error::ErrorInternalServerError(e))?
        .ok_or_else(|| error::ErrorNotFound("Rental not found"))?;
    let server = proxy_server::Entity::find_by_id(rental.server_id)
        .one(&state.conn)
        .await
        .map_err(|e| error::ErrorInternalServerError(e))?
        .ok_or_else(|| error::ErrorNotFound("Server not found"))?;
    let domain = std::env::var("DOMAIN").expect("DOMAIN not set");
    if let Err(e) = remove_user_from_proxy(
        &server.codename,
        &domain,
        &server.controller_key,
        &rental.username,
    )
    .await
    {
        return Ok(HttpResponse::Ok().json(json!({
            "success": false,
            "message": format!("Failed to remove user from proxy: {}", e)
        })));
    }
    let mut rental_update: rental::ActiveModel = rental.clone().into();
    rental_update.is_active = ActiveValue::Set(false);
    rental::Entity::update(rental_update)
        .exec(&state.conn)
        .await
        .map_err(|e| error::ErrorInternalServerError(e))?;
    let mut server_update: proxy_server::ActiveModel = server.clone().into();
    server_update.slots_available = ActiveValue::Set(server.slots_available + 1);
    server_update.proxies_rented = ActiveValue::Set(server.proxies_rented - 1);
    proxy_server::Entity::update(server_update)
        .exec(&state.conn)
        .await
        .map_err(|e| error::ErrorInternalServerError(e))?;

    // ===== NOTIFICATION
    state.tg_notificator.notify(&format!(
        "proxy rent stopped, server id:\n{}\nby user id:\n{}",
        rental.server_id, user_id
    ));
    // ===== NOTIFICATION

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "message": "Server rental stopped"
    })))
}
