use actix_session::Session;
use actix_web::web::Data;
use actix_web::{Error, HttpResponse, get, http::header};
use askama::Template;
// use rust_decimal::prelude::*;
use sea_orm::EntityTrait;
use uuid::Uuid;

// use std::time::Duration;
// use tokio::time::sleep;
// sleep(Duration::from_secs(2)).await;
//

use crate::AppState;
use crate::entities::user_session;

#[derive(Template)]
#[template(path = "index.html")]
struct Index {
    balance: String,
}

#[derive(Template)]
#[template(path = "transit.html")]
struct Transit;

#[get("/")]
async fn transit(state: Data<AppState>, session: Session) -> Result<HttpResponse, Error> {
    if let Some(user_id) = session.get::<String>("user_id")? {
        if let Ok(uuid) = Uuid::parse_str(&user_id) {
            let exists = user_session::Entity::find_by_id(uuid)
                .one(&state.conn)
                .await
                .unwrap()
                .is_some();

            if exists {
                return Ok(HttpResponse::Found()
                    .append_header((header::LOCATION, "/d"))
                    .finish());
            }
        }
        session.purge();
    }

    let html = Transit.render().unwrap();
    Ok(HttpResponse::Ok().content_type("text/html").body(html))
}

#[get("/d")]
async fn home(state: Data<AppState>, session: Session) -> Result<HttpResponse, Error> {
    // test notifier
    // state.tg_notificator.notify("new visit!");

    let user_id = session.get::<String>("user_id")?;

    match user_id {
        Some(user_id) => {
            if let Ok(uuid) = Uuid::parse_str(&user_id) {
                let user = user_session::Entity::find_by_id(uuid)
                    .one(&state.conn)
                    .await
                    .unwrap();

                if let Some(user) = user {
                    let html = Index {
                        balance: user.balance.round_dp(5u32).to_string(),
                    }
                    .render()
                    .unwrap();
                    return Ok(HttpResponse::Ok().content_type("text/html").body(html));
                }
            }
            session.purge();
        }
        None => {}
    }

    Ok(HttpResponse::Found()
        .append_header((header::LOCATION, "/"))
        .finish())
}

#[derive(Template)]
#[template(path = "tos.html")]
struct Tos;

#[get("/tos")]
async fn tos() -> Result<HttpResponse, Error> {
    let html = Tos.render().unwrap();
    Ok(HttpResponse::Ok().content_type("text/html").body(html))
}
