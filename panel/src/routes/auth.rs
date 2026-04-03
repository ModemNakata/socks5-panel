use actix_session::Session;
use actix_web::web::{Data, Json};
use actix_web::{Error, HttpResponse, get, http::header, post};
use chrono::Utc;
use openssl::rand::rand_bytes;
use sea_orm::prelude::Decimal;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::entities::user_session;

fn generate_secret_key() -> String {
    let mut random_bytes = [0u8; 32];
    rand_bytes(&mut random_bytes).unwrap();
    format!("SK_{}", hex::encode(random_bytes).to_uppercase())
}

#[post("/api/auth/register")]
async fn register(state: Data<AppState>, session: Session) -> Result<HttpResponse, Error> {
    let secret_key = generate_secret_key();
    let now = Utc::now().naive_utc();

    let user_uuid = Uuid::new_v4();

    let user = user_session::ActiveModel {
        id: sea_orm::ActiveValue::Set(user_uuid),
        secret_key: sea_orm::ActiveValue::Set(secret_key), // hash it with blake3 optionally?
        // or use some long hashing algo like scrypt/bcrypt or argon2
        // use sha-512 with X iterations /.using
        balance: sea_orm::ActiveValue::Set(Decimal::from(0)),
        created_at: sea_orm::ActiveValue::Set(now),
    };

    let result = user_session::Entity::insert(user)
        .exec(&state.conn)
        .await
        .unwrap();

    let session_id = result.last_insert_id.to_string();

    session.insert("user_id", session_id)?;

    // redirect is managed by session_manager.js
    // Ok(HttpResponse::Found()
    //     .append_header((header::LOCATION, "/d"))
    //     .finish())

    // ===== NOTIFICATION
    state
        .tg_notificator
        .notify(&format!("new registration, user id:\n{}", user_uuid));
    // ===== NOTIFICATION

    Ok(HttpResponse::Ok().into())
}

#[derive(Serialize)]
struct SecretKeyResponse {
    success: bool,
    secret_key: Option<String>,
    // user_id: Option<String>,
}

#[get("/api/auth/secret-key")]
async fn get_secret_key(state: Data<AppState>, session: Session) -> Result<HttpResponse, Error> {
    let user_id = session.get::<String>("user_id")?;

    match user_id {
        Some(user_id) => {
            if let Ok(uuid) = Uuid::parse_str(&user_id) {
                if let Some(user) = user_session::Entity::find_by_id(uuid)
                    .one(&state.conn)
                    .await
                    .unwrap()
                {
                    return Ok(HttpResponse::Ok().json(SecretKeyResponse {
                        success: true,
                        secret_key: Some(user.secret_key),
                        // user_id: Some(user_id),
                    }));
                }
            }
        }
        None => {}
    }

    Ok(HttpResponse::Ok().json(SecretKeyResponse {
        success: false,
        secret_key: None,
        // user_id: None,
    }))
}

#[derive(Deserialize)]
struct LoginRequest {
    secret_key: String,
}

#[derive(Serialize)]
struct LoginResponse {
    success: bool,
    error: Option<String>,
}

#[post("/api/auth/login")]
async fn login_with_sk(
    state: Data<AppState>,
    session: Session,
    req: Json<LoginRequest>,
) -> Result<HttpResponse, Error> {
    let sk = req.secret_key.trim();

    // Find user by secret key
    let user = user_session::Entity::find()
        .filter(user_session::Column::SecretKey.eq(sk))
        .one(&state.conn)
        .await
        .unwrap();

    match user {
        Some(user) => {
            session.insert("user_id", user.id.to_string())?;
            Ok(HttpResponse::Ok().json(LoginResponse {
                success: true,
                error: None,
            }))
        }
        None => Ok(HttpResponse::BadRequest().json(LoginResponse {
            success: false,
            error: Some("Invalid secret key".to_string()),
        })),
    }
}

#[post("/api/auth/logout")]
async fn logout(_state: Data<AppState>, session: Session) -> Result<HttpResponse, Error> {
    // Clear the session
    session.purge();

    // Redirect to transit endpoint which will generate a new secret key
    Ok(HttpResponse::Found()
        .append_header((header::LOCATION, "/"))
        .finish())
}
