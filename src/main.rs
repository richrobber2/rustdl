macro_rules! include_js {
    ($name:literal) => {
        include_str!(concat!(env!("OUT_DIR"), "/js/", $name))
    };
}

pub(crate) mod external;
pub(crate) mod local;
pub(crate) mod workflows;

#[cfg(feature = "heap-profile")]
pub(crate) mod heap_workload;

#[cfg(all(test, feature = "dev"))]
mod dev;

#[cfg(test)]
mod activity_tests;
#[cfg(test)]
mod live_events_tests;
#[cfg(test)]
mod network_policy_tests;
#[cfg(test)]
mod settings_tests;
#[cfg(test)]
mod streaming_tests;
#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;

fn main() {
    if let Err(error) = workflows::cli::run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
