use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProxyServer::Table)
                    .if_not_exists()
                    .col(
                        uuid(ProxyServer::Id)
                            .primary_key()
                            .not_null()
                            .primary_key()
                            .extra("DEFAULT gen_random_uuid()".to_string()),
                    )
                    .col(string(ProxyServer::Country).not_null())
                    // .col(string(ProxyServer::CountryCode).not_null())
                    // .col(string(ProxyServer::Codename).unique_key().not_null())
                    // not unique for testing
                    .col(string(ProxyServer::Codename).not_null())
                    .col(string(ProxyServer::ControllerKey).not_null())
                    .col(decimal(ProxyServer::Price).not_null()) // 0 for free (promo) servers
                    .col(integer(ProxyServer::Speed).not_null()) // bandwidth ...
                    // slots available int // e.g. proxies can be bought from this server
                    .col(integer(ProxyServer::SlotsAvailable).not_null().default(0))
                    // .col(integer(ProxyServer::TotalSlots).not_null().default(0)) // actually this value doesn't participate in calculations and can be always 0
                    // slots taken int // e.g. proxies in use / rented
                    .col(integer(ProxyServer::ProxiesRented).not_null().default(0))
                    .col(integer(ProxyServer::Port).not_null())
                    // promo servers has max 1 slot available for 1 user (backend checks for unique ip to prevent abuse too)
                    .col(boolean(ProxyServer::IsPromo).not_null().default(false))
                    .col(boolean(ProxyServer::IsReady).not_null().default(true))
                    .col(timestamp(ProxyServer::CreatedAt).default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ProxyServer::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum ProxyServer {
    Table,
    Id,
    Country,
    // CountryCode,
    Codename,
    ControllerKey,
    // TotalSlots, // MaxProxies
    // ^ no need for this field actually
    SlotsAvailable,
    ProxiesRented,
    IsPromo,
    IsReady, // (hide or make public this proxy, e.g. enable renting of this proxy or disable it easily)
    Price,
    Speed,
    Port,
    CreatedAt,
}
