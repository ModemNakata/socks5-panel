use actix_files as fs;
use actix_session::{SessionMiddleware, config::PersistentSession, storage::CookieSessionStore};
use actix_web::cookie::{Key, time::Duration};
use actix_web::middleware::Logger;
use actix_web::{App, HttpServer, web};
use dotenvy::dotenv;
use sea_orm::{Database, DatabaseConnection};
use std::env;
use tg_notify::Notifier;

mod dash;
mod entities;
mod routes;

#[derive(Clone)]
pub struct AppState {
    conn: DatabaseConnection,
    pub gateway_url: String,
    pub gateway_api_key: String,
    pub notify_url: String,
    pub platega_merchant_id: String,
    pub platega_secret: String,
    pub platega_callback_url: String,
    pub trocador_callback_url: String,
    pub tg_notificator: Notifier,
    pub domain: String,
}

#[actix_web::main]
async fn start() -> std::io::Result<()> {
    dotenv().ok();
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("debug"));

    let db_url = env::var("DB_URL").unwrap();
    let conn = Database::connect(&db_url).await.unwrap();

    let cookie_key = env::var("COOKIE_ENCRYPTED").unwrap();
    let key = Key::from(cookie_key.as_bytes());

    let gateway_url = env::var("CRYPTO_PAYMENT_GATEWAY_URL").unwrap();
    let gateway_api_key = env::var("CRYPTO_PAYMENT_GATEWAY_API_KEY").unwrap();

    let notify_url = env::var("CW_NOTIFY_URL").expect("CW_NOTIFY_URL must be set");

    let platega_merchant_id =
        env::var("PLATEGA_MERCHANT_ID").expect("PLATEGA_MERCHANT_ID must be set");
    let platega_secret = env::var("PLATEGA_SECRET").expect("PLATEGA_SECRET must be set");
    let platega_callback_url =
        env::var("PLATEGA_CALLBACK_URL").expect("PLATEGA_CALLBACK_URL must be set");

    let trocador_callback_url =
        env::var("TROCADOR_CALLBACK_URL").expect("TROCADOR_CALLBACK_URL must be set");

    let tg_bot_token = env::var("TG_BOT_TOKEN").expect("TG_BOT_TOKEN must be set");
    let tg_chat_id = env::var("TG_CHAT_ID").expect("TG_CHAT_ID must be set");

    let tg_notificator = Notifier::new(tg_bot_token, tg_chat_id);

    let domain = env::var("DOMAIN").expect("DOMAIN must be set");

    let state = AppState {
        conn,
        gateway_url,
        gateway_api_key,
        notify_url,
        platega_merchant_id,
        platega_secret,
        platega_callback_url,
        trocador_callback_url,
        tg_notificator,
        domain,
    };

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), key.clone())
                    .session_lifecycle(
                        PersistentSession::default().session_ttl(Duration::days(365)),
                    )
                    .build(),
            )
            .app_data(web::Data::new(state.clone()))
            .configure(init)
    })
    .bind(("0.0.0.0", 1337))?
    .run()
    .await
}

fn init(cfg: &mut web::ServiceConfig) {
    cfg.service(dash::transit);
    cfg.service(dash::home);
    cfg.service(routes::auth::register);
    cfg.service(routes::auth::get_secret_key);
    cfg.service(routes::auth::login_with_sk);
    cfg.service(routes::auth::logout);
    cfg.service(routes::api::get_servers);

    cfg.service(routes::controller::rent_server);
    cfg.service(routes::controller::stop_rent);
    cfg.service(routes::controller::get_rentals);

    cfg.service(routes::tickets::ticket);
    cfg.service(routes::tickets::staff);
    cfg.service(routes::tickets::staff_chat);
    cfg.service(routes::tickets::get_chat_messages);
    cfg.service(routes::tickets::send_chat_message);
    cfg.service(routes::tickets::get_staff_chats);
    cfg.service(routes::tickets::staff_send_message);

    //
    cfg.service(routes::topup::payment::payment_process);

    cfg.service(routes::topup::cryptowrap::api_payment_process);
    cfg.service(routes::topup::cryptowrap::payment_status_update);

    cfg.service(routes::topup::platega::api_payment_process);
    cfg.service(routes::topup::platega::platega_callback);
    cfg.service(routes::topup::platega::verify_platega);
    cfg.service(routes::topup::platega::api_verify_platega_status);

    cfg.service(routes::coupon::redeem_coupon);

    // cfg.service(routes::topup::trocador::api_payment_process);
    // cfg.service(routes::topup::trocador::trocador_callback);

    cfg.service(dash::tos);

    cfg.service(fs::Files::new("/assets", "assets/.").show_files_listing());
    cfg.service(fs::Files::new("/dev", ".").show_files_listing());
}

pub fn main() {
    let result = start();
    if let Some(err) = result.err() {
        println!("Error: {err}");
    }
}
