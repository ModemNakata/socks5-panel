use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Rental::Table)
                    .if_not_exists()
                    .col(uuid(Rental::Id).primary_key())
                    .col(uuid(Rental::UserId).not_null())
                    .col(uuid(Rental::ServerId).not_null())
                    .col(string(Rental::Username).not_null())
                    .col(string(Rental::Password).not_null())
                    .col(boolean(Rental::IsActive).not_null().default(true))
                    .col(timestamp(Rental::CreatedAt).default(Expr::current_timestamp()))
                    .col(timestamp(Rental::UpdatedAt).default(Expr::current_timestamp())) // e.g. last charged (hourly billing)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Rental::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Rental {
    Table,
    Id,
    UserId,
    ServerId,
    Username,
    Password,
    IsActive,
    CreatedAt,
    UpdatedAt,
}
