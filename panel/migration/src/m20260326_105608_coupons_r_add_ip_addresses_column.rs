use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add ip_address column to coupon_redemptions table
        manager
            .alter_table(
                Table::alter()
                    .table("coupon_redemptions")
                    .add_column(string("ip_address").null())
                    .to_owned(),
            )
            .await?;

        // // Drop the old unique index
        // manager
        //     .drop_index(
        //         Index::drop()
        //             .table("coupon_redemptions")
        //             .name("idx-coupon_redemptions-coupon_user")
        //             .to_owned(),
        //     )
        //     .await?;

        // Create new unique index on (coupon_id, ip_address) to prevent same IP from redeeming same coupon
        manager
            .create_index(
                Index::create()
                    .name("idx-coupon_redemptions-coupon_ip")
                    .table("coupon_redemptions")
                    .col("coupon_id")
                    .col("ip_address")
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop the new unique index
        manager
            .drop_index(
                Index::drop()
                    .table("coupon_redemptions")
                    .name("idx-coupon_redemptions-coupon_ip")
                    .to_owned(),
            )
            .await?;

        // // Restore the old unique index on (coupon_id, user_id)
        // manager
        //     .create_index(
        //         Index::create()
        //             .name("idx-coupon_redemptions-coupon_user")
        //             .table("coupon_redemptions")
        //             .col("coupon_id")
        //             .col("user_id")
        //             .unique()
        //             .to_owned(),
        //     )
        //     .await?;

        // Remove ip_address column
        manager
            .alter_table(
                Table::alter()
                    .table("coupon_redemptions")
                    .drop_column("ip_address")
                    .to_owned(),
            )
            .await
    }
}
