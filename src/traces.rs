#![allow(unused)]
use std::collections::HashMap;

use miette::IntoDiagnostic;
use opentelemetry::{KeyValue, global, trace::TracerProvider};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    Resource,
    logs::LoggerProvider,
    metrics::{
        MeterProviderBuilder, PeriodicReader, SdkMeterProvider,
        reader::{DefaultAggregationSelector, DefaultTemporalitySelector},
    },
    runtime,
    trace::{BatchConfig, RandomIdGenerator, Sampler, Tracer},
};
use opentelemetry_semantic_conventions::{
    SCHEMA_URL,
    resource::{DEPLOYMENT_ENVIRONMENT_NAME, SERVICE_NAME, SERVICE_VERSION},
};
use tracing_core::Level;
use tracing_opentelemetry::{MetricsLayer, OpenTelemetryLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Create a Resource that captures information about the entity for which telemetry is recorded.
fn resource() -> Resource {
    Resource::from_schema_url(
        [
            KeyValue::new(SERVICE_NAME, env!("CARGO_PKG_NAME")),
            KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
            KeyValue::new(DEPLOYMENT_ENVIRONMENT_NAME, "prod"),
        ],
        SCHEMA_URL,
    )
}

// Construct MeterProvider for MetricsLayer
fn init_meter_provider(endpoint: &str, token: &str) -> miette::Result<SdkMeterProvider> {
    let mut h = HashMap::new();
    h.insert("Authorization".to_string(), format!("Basic {token}"));
    let exporter = opentelemetry_otlp::new_exporter()
        .http()
        .with_endpoint(format!("{endpoint}/v1/metrics"))
        .with_headers(h)
        .with_protocol(opentelemetry_otlp::Protocol::HttpBinary)
        .build_metrics_exporter(
            Box::new(DefaultAggregationSelector::new()),
            Box::new(DefaultTemporalitySelector::new()),
        )
        .into_diagnostic()?;

    let reader = PeriodicReader::builder(exporter, runtime::Tokio)
        .with_interval(std::time::Duration::from_secs(30))
        .build();

    let meter_provider = MeterProviderBuilder::default()
        .with_resource(resource())
        .with_reader(reader)
        .build();

    global::set_meter_provider(meter_provider.clone());

    Ok(meter_provider)
}

// Construct Tracer for OpenTelemetryLayer
fn init_tracer(endpoint: &str, token: &str) -> miette::Result<Tracer> {
    let mut h = HashMap::new();
    h.insert("Authorization".to_string(), format!("Basic {token}"));
    let provider = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_trace_config(
            opentelemetry_sdk::trace::Config::default()
                .with_sampler(Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(
                    1.0,
                ))))
                .with_id_generator(RandomIdGenerator::default())
                .with_resource(resource()),
        )
        .with_batch_config(BatchConfig::default())
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .http()
                .with_headers(h)
                .with_endpoint(format!("{endpoint}/v1/traces")),
        )
        .install_batch(runtime::Tokio)
        .into_diagnostic()?;

    global::set_tracer_provider(provider.clone());
    Ok(provider.tracer("secretlink-tracer-subscriber"))
}

fn init_logs(endpoint: &str, token: &str) -> miette::Result<LoggerProvider> {
    let mut h = HashMap::new();
    h.insert("Authorization".to_string(), format!("Basic {token}"));
    opentelemetry_otlp::new_pipeline()
        .logging()
        .with_resource(resource())
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .http()
                .with_headers(h)
                .with_endpoint(format!("{endpoint}/v1/logs")),
        )
        .install_batch(runtime::Tokio)
        .into_diagnostic()
}

// Initialize tracing-subscriber and return OtelGuard for opentelemetry-related termination processing
pub fn init_tracing_subscriber(endpoint: &str, token: &str) -> miette::Result<OtelGuard> {
    let meter_provider = init_meter_provider(endpoint, token)?;
    let tracer = init_tracer(endpoint, token)?;
    let logger = init_logs(endpoint, token)?;

    let layer = OpenTelemetryTracingBridge::new(&logger);
    let filter = tracing_subscriber::filter::LevelFilter::from_level(Level::INFO);

    tracing_subscriber::registry()
        .with(filter)
        .with(layer)
        .with(tracing_subscriber::fmt::layer())
        .with(OpenTelemetryLayer::new(tracer))
        .with(MetricsLayer::new(meter_provider.clone()))
        .init();

    Ok(OtelGuard { meter_provider })
}

pub struct OtelGuard {
    meter_provider: SdkMeterProvider,
}

impl Drop for OtelGuard {
    fn drop(&mut self) {
        if let Err(err) = self.meter_provider.shutdown() {
            eprintln!("{err:?}");
        }
        opentelemetry::global::shutdown_tracer_provider();
    }
}
