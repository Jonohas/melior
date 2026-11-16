//! Logging and trace export.
//!
//! Logs go to standard output, filtered by `RUST_LOG`. If an OTLP endpoint is
//! configured, the spans of the use cases (join, offer, login, ...) are also
//! exported as traces over gRPC, for example to Jaeger.

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// Used when `RUST_LOG` is unset: the WebRTC crates are chatty at `info`.
pub const DEFAULT_LOG_FILTER: &str = "warn,webrtc_ice=error,melior_sfu=info,melior_application=info,melior_interfaces=info,melior_infrastructure=info";

const SERVICE_NAME: &str = "melior-sfu";

#[derive(thiserror::Error, Debug)]
pub enum TelemetryError {
    #[error("cannot set up trace export: {0}")]
    Exporter(String),

    #[error("logging was already initialised")]
    AlreadyInitialised,
}

/// Keeps trace export alive. Call [`Telemetry::shutdown`] before the process
/// exits, or the last spans are lost.
pub struct Telemetry {
    provider: Option<SdkTracerProvider>,
}

impl Telemetry {
    /// Sends what is still buffered.
    pub fn shutdown(self) {
        if let Some(provider) = self.provider
            && let Err(error) = provider.shutdown()
        {
            tracing::warn!(%error, "trace export did not shut down cleanly");
        }
    }
}

/// Installs the global logger and, if `otlp_endpoint` is given, trace export to it.
/// Must be called once, inside the async runtime.
pub fn init(otlp_endpoint: Option<&str>, instance_id: &str) -> Result<Telemetry, TelemetryError> {
    // An empty `RUST_LOG` (easy to end up with in a compose file) means "not set",
    // not "log nothing".
    let log_filter = std::env::var("RUST_LOG")
        .ok()
        .filter(|filter| !filter.trim().is_empty())
        .and_then(|filter| EnvFilter::try_new(filter).ok())
        .unwrap_or_else(|| EnvFilter::new(DEFAULT_LOG_FILTER));
    let logs = tracing_subscriber::fmt::layer().with_filter(log_filter);

    let provider =
        otlp_endpoint.map(|endpoint| tracer_provider(endpoint, instance_id)).transpose()?;
    // Only this workspace's spans are exported: the libraries underneath would
    // bury them, and their span names are not part of what this service promises.
    let traces = provider.as_ref().map(|provider| {
        tracing_opentelemetry::layer().with_tracer(provider.tracer(SERVICE_NAME)).with_filter(
            Targets::new()
                .with_target("melior_application", Level::INFO)
                .with_target("melior_interfaces", Level::INFO)
                .with_target("melior_infrastructure", Level::INFO),
        )
    });

    tracing_subscriber::registry()
        .with(logs)
        .with(traces)
        .try_init()
        .map_err(|_| TelemetryError::AlreadyInitialised)?;
    Ok(Telemetry { provider })
}

fn tracer_provider(endpoint: &str, instance_id: &str) -> Result<SdkTracerProvider, TelemetryError> {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
        .map_err(|error| TelemetryError::Exporter(error.to_string()))?;
    let resource = Resource::builder()
        .with_service_name(SERVICE_NAME)
        .with_attribute(opentelemetry::KeyValue::new("service.instance.id", instance_id.to_owned()))
        .build();
    Ok(SdkTracerProvider::builder().with_batch_exporter(exporter).with_resource(resource).build())
}
