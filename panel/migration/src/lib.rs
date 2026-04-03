pub use sea_orm_migration::prelude::*;

mod m20260301_201645_user_session_table;
mod m20260301_201657_proxy_server_table;
mod m20260301_202241_rentals_table;
mod m20260306_231234_chat_messages;
mod m20260310_234421_top_ups;
mod m20260324_000000_coupons;
mod m20260326_105608_coupons_r_add_ip_addresses_column;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260301_201645_user_session_table::Migration),
            Box::new(m20260301_201657_proxy_server_table::Migration),
            Box::new(m20260301_202241_rentals_table::Migration),
            Box::new(m20260306_231234_chat_messages::Migration),
            Box::new(m20260310_234421_top_ups::Migration),
            Box::new(m20260324_000000_coupons::Migration),
            Box::new(m20260326_105608_coupons_r_add_ip_addresses_column::Migration),
        ]
    }
}
