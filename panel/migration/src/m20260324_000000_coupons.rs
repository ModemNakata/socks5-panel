use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create coupons table
        manager
            .create_table(
                Table::create()
                    .table("coupons")
                    .if_not_exists()
                    .col(pk_uuid("id"))
                    .col(string("code").unique_key().not_null())
                    .col(decimal("balance_amount").not_null())
                    .col(integer("max_uses").not_null())
                    .col(integer("used_count").default(0).not_null())
                    .col(boolean("is_active").default(true).not_null())
                    .col(timestamp("expires_at").null())
                    .col(timestamp("created_at").default(Expr::current_timestamp()).not_null())
                    .col(string("created_by").not_null())
                    .to_owned(),
            )
            .await?;

        // Create coupon_redemptions table
        manager
            .create_table(
                Table::create()
                    .table("coupon_redemptions")
                    .if_not_exists()
                    .col(pk_uuid("id"))
                    .col(uuid("coupon_id").not_null())
                    .col(uuid("user_id").not_null())
                    .col(timestamp("redeemed_at").default(Expr::current_timestamp()).not_null())
                    .col(decimal("amount_added").not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-coupon_redemptions-coupon_id")
                            .from("coupon_redemptions", "coupon_id")
                            .to("coupons", "id")
                            .on_delete(ForeignKeyAction::Cascade)
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-coupon_redemptions-user_id")
                            .from("coupon_redemptions", "user_id")
                            .to("user_session", "id")
                            .on_delete(ForeignKeyAction::Cascade)
                    )
                    .index(
                        Index::create()
                            .name("idx-coupon_redemptions-coupon_user")
                            .col("coupon_id")
                            .col("user_id")
                            .unique()
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("coupon_redemptions").to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table("coupons").to_owned())
            .await
    }
}
