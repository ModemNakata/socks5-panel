use actix_web::web::Data;
use actix_web::{Error, HttpResponse, get};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::AppState;
use crate::entities::proxy_server;
use crate::entities::proxy_server::Entity as ProxyServer;

#[get("/api/servers")]
pub async fn get_servers(state: Data<AppState>) -> Result<HttpResponse, Error> {
    let servers = ProxyServer::find()
        .filter(proxy_server::Column::IsReady.eq(true))
        .filter(proxy_server::Column::SlotsAvailable.gt(0))
        .all(&state.conn)
        .await
        .unwrap();

    let json: Vec<serde_json::Value> = servers
        .iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id.to_string(),
                "country": s.country,
                "codename": s.codename,
                "price": s.price.to_string(),
                "speed": s.speed,
                "slots_available": s.slots_available,
                "proxies_rented": s.proxies_rented,
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(json))
}
