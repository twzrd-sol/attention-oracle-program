use crate::db::Db;
use metrics_exporter_prometheus::PrometheusHandle;

#[derive(Clone)]
pub struct AppState {
    pub pool: Db,
    pub metrics: PrometheusHandle,
}
