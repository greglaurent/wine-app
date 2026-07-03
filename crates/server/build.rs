//! Build hook: rebuild the server whenever a migration changes, so
//! `sqlx::migrate!` (in `db::run_migrations`) re-embeds the latest SQL.
//! (Migrations are *applied* at runtime against the DB -- never at build time.)

fn main() {
    println!("cargo:rerun-if-changed=../../migrations");
}
