use actix_web::{Error, HttpResponse, get};
use askama::Template;

#[derive(Template)]
#[template(path = "payment_process.html")]
struct PaymentProcess;

#[get("/payment/process")]
pub async fn payment_process() -> Result<HttpResponse, Error> {
    // Serve the payment processing HTML page
    let html = PaymentProcess.render().unwrap();
    Ok(HttpResponse::Ok().content_type("text/html").body(html))
}
