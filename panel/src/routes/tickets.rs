use actix_session::Session;
use actix_web::web::{Data, Json, Path, Query};
use actix_web::{Error, HttpResponse, get, http::header, post};
use askama::Template;
use sea_orm::{ActiveValue, ColumnTrait, EntityTrait, Order, QueryFilter, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use uuid::Uuid;

use crate::AppState;
use crate::entities::{chat_message, user_session};

// ============================================================================
// TODO: Upgrade to WebSockets for real-time chat
//
// Current implementation uses HTTP polling from client-side (setInterval).
// To upgrade to WebSockets:
// 1. Use actix-ws crate (already in Cargo.toml)
// 2. Create a ChatServer actor that manages connections
// 3. Broadcast new messages to connected users in real-time
// 4. Remove client-side polling from chat.js and staff_chat.js
//
// Benefits of WebSockets:
// - Instant message delivery (no polling delay)
// - Lower server load (no repeated HTTP requests)
// - Real-time typing indicators
// - Better scalability for many concurrent users
//
// See chat examples in actix-ws crate documentation.
// ============================================================================

#[derive(Template)]
#[template(path = "chat.html")]
struct ChatTemplate;

#[derive(Template)]
#[template(path = "staff.html")]
struct StaffTemplate;

#[derive(Template)]
#[template(path = "staff_chat.html")]
struct StaffChatTemplate;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatMessageResponse {
    id: String,
    sender_type: String,
    content: String,
    created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatListItem {
    user_id: String,
    last_message: String,
    last_time: String,
    message_count: i64,
}

fn get_staff_uuid() -> String {
    env::var("STAFF_UUID").unwrap_or_default()
}

// ============================================================================
// REST API Endpoints
// ============================================================================

#[derive(Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

impl<T> ApiResponse<T> {
    fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    fn err(msg: &str) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.to_string()),
        }
    }
}

#[get("/support")]
async fn ticket(state: Data<AppState>, session: Session) -> Result<HttpResponse, Error> {
    let user_id = session.get::<String>("user_id")?;

    match user_id {
        Some(user_id) => {
            if let Ok(uuid) = Uuid::parse_str(&user_id) {
                let _user_entry = user_session::Entity::find_by_id(uuid)
                    .one(&state.conn)
                    .await
                    .unwrap();

                let html = ChatTemplate.render().unwrap();
                return Ok(HttpResponse::Ok().content_type("text/html").body(html));
            }
            session.purge();
        }
        None => {}
    }

    Ok(HttpResponse::Found()
        .append_header((header::LOCATION, "/"))
        .finish())
}

#[get("/staff/chats")]
async fn staff(_state: Data<AppState>, session: Session) -> Result<HttpResponse, Error> {
    let user_id = session.get::<String>("user_id")?;

    match user_id {
        Some(user_id) => {
            let staff_uuid = get_staff_uuid();

            if user_id != staff_uuid {
                return Ok(HttpResponse::Unauthorized().finish());
            }

            let html = StaffTemplate.render().unwrap();
            return Ok(HttpResponse::Ok().content_type("text/html").body(html));
        }
        None => {}
    }

    Ok(HttpResponse::Found()
        .append_header((header::LOCATION, "/"))
        .finish())
}

#[get("/staff/chat/{user_id}")]
async fn staff_chat(
    _state: Data<AppState>,
    session: Session,
    _user_id: Path<String>,
) -> Result<HttpResponse, Error> {
    let user_id = session.get::<String>("user_id")?;

    match user_id {
        Some(user_id) => {
            let staff_uuid = get_staff_uuid();

            if user_id != staff_uuid {
                return Ok(HttpResponse::Unauthorized().finish());
            }

            let html = StaffChatTemplate.render().unwrap();
            return Ok(HttpResponse::Ok().content_type("text/html").body(html));
        }
        None => {}
    }

    Ok(HttpResponse::Found()
        .append_header((header::LOCATION, "/"))
        .finish())
}

// GET /api/chat/messages?user_id=xxx&after_id=xxx - get messages after last known message
#[get("/api/chat/messages")]
async fn get_chat_messages(
    state: Data<AppState>,
    session: Session,
    params: Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, Error> {
    let session_user_id = session.get::<String>("user_id")?;
    let staff_uuid = get_staff_uuid();

    let user_id = params.get("user_id").cloned();
    let after_id = params.get("after_id").cloned();

    let Some(session_id) = session_user_id else {
        return Ok(HttpResponse::Unauthorized().json(ApiResponse::<()>::err("Not authenticated")));
    };

    let target_user_id = match user_id {
        Some(uid) => {
            if session_id != staff_uuid && session_id != uid {
                return Ok(
                    HttpResponse::Unauthorized().json(ApiResponse::<()>::err("Access denied"))
                );
            }
            uid
        }
        None => {
            if session_id == staff_uuid {
                return Ok(HttpResponse::BadRequest()
                    .json(ApiResponse::<()>::err("user_id required for staff")));
            }
            session_id
        }
    };

    let uuid = match Uuid::parse_str(&target_user_id) {
        Ok(u) => u,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::err("Invalid user_id")));
        }
    };

    let messages = match &after_id {
        Some(after_id_str) => {
            // Fetch messages newer than the given ID
            if let Ok(after_uuid) = Uuid::parse_str(after_id_str) {
                // Get the timestamp of the after_id message, then fetch newer
                let after_msg = chat_message::Entity::find_by_id(after_uuid)
                    .one(&state.conn)
                    .await
                    .unwrap_or(None);

                if let Some(after_msg) = after_msg {
                    chat_message::Entity::find()
                        .filter(chat_message::Column::UserId.eq(uuid))
                        .filter(chat_message::Column::CreatedAt.gt(after_msg.created_at))
                        .order_by(chat_message::Column::CreatedAt, Order::Asc)
                        .limit(50)
                        .all(&state.conn)
                        .await
                        .unwrap_or_default()
                } else {
                    // Message not found, return latest
                    chat_message::Entity::find()
                        .filter(chat_message::Column::UserId.eq(uuid))
                        .order_by(chat_message::Column::CreatedAt, Order::Desc)
                        .limit(50)
                        .all(&state.conn)
                        .await
                        .unwrap_or_default()
                }
            } else {
                // Invalid after_id, return latest
                chat_message::Entity::find()
                    .filter(chat_message::Column::UserId.eq(uuid))
                    .order_by(chat_message::Column::CreatedAt, Order::Desc)
                    .limit(50)
                    .all(&state.conn)
                    .await
                    .unwrap_or_default()
            }
        }
        None => {
            // No after_id, return latest messages
            chat_message::Entity::find()
                .filter(chat_message::Column::UserId.eq(uuid))
                .order_by(chat_message::Column::CreatedAt, Order::Desc)
                .limit(50)
                .all(&state.conn)
                .await
                .unwrap_or_default()
        }
    };

    let mut response: Vec<ChatMessageResponse> = messages
        .into_iter()
        .map(|m| ChatMessageResponse {
            id: m.id.to_string(),
            sender_type: m.sender_type,
            content: m.content,
            created_at: m.created_at.format("%Y-%m-%dT%H:%M:%S").to_string(),
        })
        .collect();

    // If fetching new messages (after_id provided), don't reverse - they're already in order
    if after_id.is_some() {
        // New messages, keep ascending order
    } else {
        response.reverse();
    }

    Ok(HttpResponse::Ok().json(ApiResponse::ok(response)))
}

// POST /api/chat/message - user sends a message
#[post("/api/chat/message")]
async fn send_chat_message(
    state: Data<AppState>,
    session: Session,
    body: Json<SendMessageRequest>,
) -> Result<HttpResponse, Error> {
    let session_user_id = session.get::<String>("user_id")?;

    let Some(user_id) = session_user_id else {
        return Ok(HttpResponse::Unauthorized().json(ApiResponse::<()>::err("Not authenticated")));
    };

    let user_uuid = match Uuid::parse_str(&user_id) {
        Ok(u) => u,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::err("Invalid user_id")));
        }
    };

    let content = body.content.trim().to_string();
    if content.is_empty() {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::err("Empty message")));
    }

    if content.len() > 2000 {
        return Ok(
            HttpResponse::BadRequest().json(ApiResponse::<()>::err("Message too long (max 2000)"))
        );
    }

    let msg = chat_message::ActiveModel {
        id: ActiveValue::Set(Uuid::new_v4()),
        user_id: ActiveValue::Set(user_uuid),
        sender_type: ActiveValue::Set("user".to_string()),
        content: ActiveValue::Set(content.clone()),
        created_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
    };

    chat_message::Entity::insert(msg)
        .exec(&state.conn)
        .await
        .unwrap();

    // ===== NOTIFICATION
    state.tg_notificator.notify(&format!(
        "support mirror:\n{}\n\nfrom user:{}",
        content, user_uuid
    ));
    // ===== NOTIFICATION

    Ok(HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
        "message": "Message sent"
    }))))
}

#[derive(Deserialize)]
struct SendMessageRequest {
    content: String,
}

// GET /api/staff/chats - list all users with chat messages
#[get("/api/staff/chats")]
async fn get_staff_chats(state: Data<AppState>, session: Session) -> Result<HttpResponse, Error> {
    let user_id = session.get::<String>("user_id")?;

    let Some(session_id) = user_id else {
        return Ok(HttpResponse::Unauthorized().json(ApiResponse::<()>::err("Not authenticated")));
    };

    if session_id != get_staff_uuid() {
        return Ok(HttpResponse::Unauthorized().json(ApiResponse::<()>::err("Access denied")));
    }

    let messages = chat_message::Entity::find()
        .order_by(chat_message::Column::CreatedAt, Order::Desc)
        .all(&state.conn)
        .await
        .unwrap_or_default();

    let mut chats_map: HashMap<String, (String, String, i64, chrono::NaiveDateTime)> =
        HashMap::new();
    for msg in messages {
        let user_id_str = msg.user_id.to_string();
        let entry = chats_map.entry(user_id_str.clone()).or_insert((
            msg.content.chars().take(50).collect(),
            msg.created_at.format("%Y-%m-%d %H:%M").to_string(),
            0i64,
            msg.created_at,
        ));
        entry.2 += 1;
    }

    let mut chat_list: Vec<ChatListItem> = chats_map
        .into_iter()
        .map(
            |(user_id, (last_message, last_time, message_count, _))| ChatListItem {
                user_id,
                last_message,
                last_time,
                message_count,
            },
        )
        .collect();

    chat_list.sort_by(|a, b| b.last_time.cmp(&a.last_time));

    Ok(HttpResponse::Ok().json(ApiResponse::ok(chat_list)))
}

// POST /api/staff/chats/{user_id}/message - staff sends message to user
#[post("/api/staff/chats/{user_id}/message")]
async fn staff_send_message(
    state: Data<AppState>,
    session: Session,
    user_id: Path<String>,
    body: Json<SendMessageRequest>,
) -> Result<HttpResponse, Error> {
    let session_user_id = session.get::<String>("user_id")?;

    let Some(session_id) = session_user_id else {
        return Ok(HttpResponse::Unauthorized().json(ApiResponse::<()>::err("Not authenticated")));
    };

    if session_id != get_staff_uuid() {
        return Ok(HttpResponse::Unauthorized().json(ApiResponse::<()>::err("Access denied")));
    }

    let user_id = user_id.into_inner();
    let user_uuid = match Uuid::parse_str(&user_id) {
        Ok(u) => u,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::err("Invalid user_id")));
        }
    };

    let content = body.content.trim().to_string();
    if content.is_empty() {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::err("Empty message")));
    }

    if content.len() > 2000 {
        return Ok(
            HttpResponse::BadRequest().json(ApiResponse::<()>::err("Message too long (max 2000)"))
        );
    }

    let msg = chat_message::ActiveModel {
        id: ActiveValue::Set(Uuid::new_v4()),
        user_id: ActiveValue::Set(user_uuid),
        sender_type: ActiveValue::Set("staff".to_string()),
        content: ActiveValue::Set(content),
        created_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
    };

    chat_message::Entity::insert(msg)
        .exec(&state.conn)
        .await
        .unwrap();

    Ok(HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
        "message": "Message sent"
    }))))
}
