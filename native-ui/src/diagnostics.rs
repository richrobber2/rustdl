use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Diagnostics {
    pub ok: bool,
    pub detail: String,
    pub data: Metrics,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub uptime_seconds: f64,
    pub processors: u32,
    pub load1: Option<f64>,
    pub load5: Option<f64>,
    pub load15: Option<f64>,
    pub memory_total_bytes: i64,
    pub memory_available_bytes: i64,
    pub storage_total_bytes: i64,
    pub storage_available_bytes: i64,
    pub battery_level: i32,
    pub battery_status: String,
    pub battery_temperature_c: Option<f64>,
    pub thermal_status: i32,
    pub available_sources: u32,
    pub total_sources: u32,
}

pub fn bytes(value: i64) -> String {
    if value < 0 {
        "Unavailable".to_owned()
    } else {
        format!("{:.1} GiB", value as f64 / 1_073_741_824.0)
    }
}

pub fn metric(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| format!("{value:.1}"))
        .unwrap_or_else(|| "Unavailable".to_owned())
}
