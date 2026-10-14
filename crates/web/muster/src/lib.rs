//! The muster's browser client: its client half (`angel_muster::client`)
//! behind the browser runtime's ABI. `crates/web/build.sh web/muster muster`
//! builds the bundle `muster-host --bundle` serves.

angel_web::export_client!(angel_muster::MusterClient);
