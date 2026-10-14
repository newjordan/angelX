//! The Delve's browser client: its client half (`angel_delve::client`)
//! behind the browser runtime's ABI. `crates/web/build.sh web/delve delve`
//! builds the bundle, with the Delve's sounds and music from `sounds.list`.

angel_web::export_client!(angel_delve::client::DelveClient);
