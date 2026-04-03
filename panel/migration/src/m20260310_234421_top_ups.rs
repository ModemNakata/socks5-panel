use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("top_ups")
                    .if_not_exists()
                    .col(pk_uuid("id"))
                    .col(string("external_id").null()) // Platega's transaction_id, null initially
                    .col(string("user_id").not_null())
                    .col(string("external_status").null()) // default `waiting` (on external), but will be null here
                    .col(string("amount_paid").null())
                    .col(
                        date_time("created_at")
                            .default(Expr::current_timestamp())
                            .not_null(),
                    )
                    .col(date_time("updated_at").null())
                    .col(boolean("balance_claimed").default(false))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("top_ups").to_owned())
            .await
    }
}
