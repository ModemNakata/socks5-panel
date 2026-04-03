use actix_session::Session;
use actix_web::web::{Data, Json};
use actix_web::{Error, HttpRequest, HttpResponse, post};
use chrono::Utc;
use sea_orm::prelude::Decimal;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::entities::{coupon_redemptions, coupons, user_session};

#[derive(Deserialize)]
pub struct RedeemCouponRequest {
    code: String,
}

#[derive(Serialize)]
pub struct RedeemCouponResponse {
    success: bool,
    message: String,
    amount: Option<Decimal>,
    new_balance: Option<Decimal>,
}

#[post("/api/coupon/redeem")]
pub async fn redeem_coupon(
    state: Data<AppState>,
    session: Session,
    req: Json<RedeemCouponRequest>,
    http_req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let code = req.code.trim().to_uppercase();

    // Get client IP address
    let client_ip = http_req
        .peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Get user from session
    let user_id = match session.get::<String>("user_id")? {
        Some(id) => id,
        None => {
            return Ok(HttpResponse::Unauthorized().json(RedeemCouponResponse {
                success: false,
                message: "Not authenticated".to_string(),
                amount: None,
                new_balance: None,
            }));
        }
    };

    let user_uuid = match Uuid::parse_str(&user_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().json(RedeemCouponResponse {
                success: false,
                message: "Invalid user session".to_string(),
                amount: None,
                new_balance: None,
            }));
        }
    };

    // Find coupon by code
    let coupon = match coupons::Entity::find()
        .filter(coupons::Column::Code.eq(&code))
        .one(&state.conn)
        .await
    {
        Ok(Some(c)) => c,
        Ok(None) => {
            return Ok(HttpResponse::BadRequest().json(RedeemCouponResponse {
                success: false,
                message: "Invalid coupon code".to_string(),
                amount: None,
                new_balance: None,
            }));
        }
        Err(_) => {
            return Ok(
                HttpResponse::InternalServerError().json(RedeemCouponResponse {
                    success: false,
                    message: "Database error".to_string(),
                    amount: None,
                    new_balance: None,
                }),
            );
        }
    };

    // Check if coupon is active
    if !coupon.is_active {
        return Ok(HttpResponse::BadRequest().json(RedeemCouponResponse {
            success: false,
            message: "This coupon has been deactivated".to_string(),
            amount: None,
            new_balance: None,
        }));
    }

    // Check if coupon is expired
    if let Some(expires_at) = coupon.expires_at {
        if Utc::now().naive_utc() > expires_at {
            return Ok(HttpResponse::BadRequest().json(RedeemCouponResponse {
                success: false,
                message: "This coupon has expired".to_string(),
                amount: None,
                new_balance: None,
            }));
        }
    }

    // Check if coupon has reached max uses
    if coupon.used_count >= coupon.max_uses {
        return Ok(HttpResponse::BadRequest().json(RedeemCouponResponse {
            success: false,
            message: "This coupon has reached its usage limit".to_string(),
            amount: None,
            new_balance: None,
        }));
    }

    // Check if user already redeemed this coupon
    let existing_redemption = coupon_redemptions::Entity::find()
        .filter(coupon_redemptions::Column::CouponId.eq(coupon.id))
        .filter(coupon_redemptions::Column::UserId.eq(user_uuid))
        .one(&state.conn)
        .await;

    if let Ok(Some(_)) = existing_redemption {
        return Ok(HttpResponse::BadRequest().json(RedeemCouponResponse {
            success: false,
            message: "You have already redeemed this coupon".to_string(),
            amount: None,
            new_balance: None,
        }));
    }

    // Check if this IP has already redeemed this coupon
    let existing_ip_redemption = coupon_redemptions::Entity::find()
        .filter(coupon_redemptions::Column::CouponId.eq(coupon.id))
        .filter(coupon_redemptions::Column::IpAddress.eq(&client_ip))
        .one(&state.conn)
        .await;

    if let Ok(Some(_)) = existing_ip_redemption {
        return Ok(HttpResponse::BadRequest().json(RedeemCouponResponse {
            success: false,
            message: "This coupon has already been redeemed from this device".to_string(),
            amount: None,
            new_balance: None,
        }));
    }

    // Get user's current balance
    let user = match user_session::Entity::find_by_id(user_uuid)
        .one(&state.conn)
        .await
    {
        Ok(Some(u)) => u,
        Ok(None) => {
            return Ok(HttpResponse::NotFound().json(RedeemCouponResponse {
                success: false,
                message: "User not found".to_string(),
                amount: None,
                new_balance: None,
            }));
        }
        Err(_) => {
            return Ok(
                HttpResponse::InternalServerError().json(RedeemCouponResponse {
                    success: false,
                    message: "Database error".to_string(),
                    amount: None,
                    new_balance: None,
                }),
            );
        }
    };

    // Calculate new balance
    let new_balance = user.balance + coupon.balance_amount;

    // Create redemption record and update user balance in a transaction
    let redemption = coupon_redemptions::ActiveModel {
        id: sea_orm::ActiveValue::Set(Uuid::new_v4()),
        coupon_id: sea_orm::ActiveValue::Set(coupon.id),
        user_id: sea_orm::ActiveValue::Set(user_uuid),
        redeemed_at: sea_orm::ActiveValue::Set(Utc::now().naive_utc()),
        amount_added: sea_orm::ActiveValue::Set(coupon.balance_amount),
        ip_address: sea_orm::ActiveValue::Set(Some(client_ip)),
    };

    let mut user_active: user_session::ActiveModel = user.into();
    user_active.balance = sea_orm::ActiveValue::Set(new_balance);

    // Execute transaction
    let txn = match state.conn.begin().await {
        Ok(t) => t,
        Err(_) => {
            return Ok(
                HttpResponse::InternalServerError().json(RedeemCouponResponse {
                    success: false,
                    message: "Failed to start transaction".to_string(),
                    amount: None,
                    new_balance: None,
                }),
            );
        }
    };

    // Insert redemption record
    if let Err(_) = coupon_redemptions::Entity::insert(redemption)
        .exec(&txn)
        .await
    {
        let _ = txn.rollback().await;
        return Ok(
            HttpResponse::InternalServerError().json(RedeemCouponResponse {
                success: false,
                message: "Failed to create redemption record".to_string(),
                amount: None,
                new_balance: None,
            }),
        );
    }

    // Update user balance
    if let Err(_) = user_active.update(&txn).await {
        let _ = txn.rollback().await;
        return Ok(
            HttpResponse::InternalServerError().json(RedeemCouponResponse {
                success: false,
                message: "Failed to update balance".to_string(),
                amount: None,
                new_balance: None,
            }),
        );
    }

    // Save coupon data before moving
    let coupon_balance = coupon.balance_amount.clone();
    let coupon_id = coupon.id;

    // Update coupon used_count
    let mut coupon_active: coupons::ActiveModel = coupon.into();
    let current_count = coupon_active.used_count.unwrap();
    coupon_active.used_count = sea_orm::ActiveValue::Set(current_count + 1);
    if let Err(_) = coupon_active.update(&txn).await {
        let _ = txn.rollback().await;
        return Ok(
            HttpResponse::InternalServerError().json(RedeemCouponResponse {
                success: false,
                message: "Failed to update coupon usage".to_string(),
                amount: None,
                new_balance: None,
            }),
        );
    }

    // Commit transaction
    if let Err(_) = txn.commit().await {
        return Ok(
            HttpResponse::InternalServerError().json(RedeemCouponResponse {
                success: false,
                message: "Failed to commit transaction".to_string(),
                amount: None,
                new_balance: None,
            }),
        );
    }

    // ===== NOTIFICATION
    state.tg_notificator.notify(&format!(
        "coupon redeemed {} {} EUR\nby user:\n{}",
        code, coupon_balance, user_uuid
    ));
    // ===== NOTIFICATION

    // Success
    Ok(HttpResponse::Ok().json(RedeemCouponResponse {
        success: true,
        message: format!("Successfully redeemed €{} coupon!", coupon_balance),
        amount: Some(coupon_balance),
        new_balance: Some(new_balance),
    }))
}
