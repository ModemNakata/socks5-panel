use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ChatMessage::Table)
                    .if_not_exists()
                    .col(uuid(ChatMessage::Id).primary_key().extra("DEFAULT gen_random_uuid()".to_string()))
                    .col(uuid(ChatMessage::UserId).not_null())
                    .col(string(ChatMessage::SenderType).not_null()) // 'user' or 'staff'
                    .col(text(ChatMessage::Content).not_null())
                    .col(timestamp(ChatMessage::CreatedAt).default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ChatMessage::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum ChatMessage {
    Table,
    Id,
    UserId,
    SenderType,
    Content,
    CreatedAt,
}
