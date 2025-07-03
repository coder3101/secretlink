use aes_gcm::{
    Aes256Gcm, Key, KeyInit,
    aead::{Aead, generic_array::GenericArray},
};
use axum::{
    Router,
    extract::{Form, Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use base64::Engine;
use base64::engine::general_purpose;
use dotenv::dotenv;
use miette::IntoDiagnostic;
use secret::{SecretControl, SecretController, SecretState};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, postgres::PgPoolOptions};
use templates::{
    AboutTemplate, ConsumeTemplate, GotoTemplate, HowItWorksTemplate, IndexTemplate,
    PrivacyTemplate, ResultTemplate,
};
use tracing::Level;
use tracing_subscriber::{Registry, layer::SubscriberExt, util::SubscriberInitExt};

mod secret;
mod templates;
mod traces;

#[derive(Serialize, Deserialize)]
struct GenerateInput {
    #[serde(rename = "ciperText")]
    pub ciper_text: String,
    pub iv: String,
    pub expiry: usize,
}

#[derive(Deserialize)]
struct CipherKey {
    key: String,
}

#[derive(Clone)]
struct AppState<T: SecretControl> {
    pool: PgPool,
    db: T,
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    dotenv().ok();
    let db = std::env::var("DATABASE_URL").expect("could not connect to database");
    let axiom_layer = tracing_axiom::default("secretlink").into_diagnostic()?;
    let filter = tracing_subscriber::filter::LevelFilter::from_level(Level::INFO);
    Registry::default()
        .with(filter)
        .with(axiom_layer)
        .with(tracing_subscriber::fmt::layer())
        .init();

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db)
        .await
        .into_diagnostic()?;

    tracing::info!("secretlink connected to database, starting migration");
    sqlx::migrate!().run(&pool).await.into_diagnostic()?;
    tracing::info!("database migration completed");

    let app_state = AppState {
        pool,
        db: SecretController,
    };

    let app = router(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .into_diagnostic()?;

    tracing::info!("started listening on 0.0.0.0:8080");
    axum::serve(listener, app).await.into_diagnostic()?;
    Ok(())
}

fn router(app_state: AppState<SecretController>) -> Router {
    Router::new()
        .nest(
            "/static",
            axum_static::static_router("static").with_state(()),
        )
        .route("/", get(root))
        .route("/privacy", get(privacy))
        .route("/about", get(about))
        .route("/how-it-works", get(how_it_works))
        .route("/goto/:id", get(goto::<SecretController>))
        .route("/consume/:id", get(consume::<SecretController>))
        .route("/api/generate-url", post(generate::<SecretController>))
        .with_state(app_state)
}

#[tracing::instrument]
async fn root() -> IndexTemplate {
    IndexTemplate {}
}

#[tracing::instrument]
async fn privacy() -> PrivacyTemplate {
    PrivacyTemplate {}
}

#[tracing::instrument]
async fn about() -> AboutTemplate {
    AboutTemplate {}
}

#[tracing::instrument]
async fn how_it_works() -> HowItWorksTemplate {
    HowItWorksTemplate {}
}

#[tracing::instrument(skip(state, input), err)]
async fn generate<DB: SecretControl>(
    State(state): State<AppState<DB>>,
    Form(input): Form<GenerateInput>,
) -> Result<ResultTemplate, StatusCode> {
    let exp = if input.expiry == 0 {
        None
    } else {
        Some(input.expiry as i32)
    };

    let mut tx = match state.pool.begin().await {
        Ok(tx) => tx,
        Err(err) => {
            tracing::error!(?err, "error starting transaction");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let id = state
        .db
        .add(&mut tx, input.ciper_text.as_str(), &input.iv, exp)
        .await;

    match id {
        Err(err) => {
            tracing::error!(?err, "error adding secret to database");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
        Ok(u) => {
            let url = format!("goto/{u}");
            if let Err(err) = tx.commit().await {
                tracing::error!(?err, "error committing transaction");
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
            Ok(ResultTemplate { url })
        }
    }
}

#[tracing::instrument(skip(state, q), err)]
async fn goto<DB: SecretControl>(
    State(state): State<AppState<DB>>,
    Path(id): Path<uuid::Uuid>,
    Query(q): Query<CipherKey>,
) -> Result<GotoTemplate, StatusCode> {
    let Ok(key) = general_purpose::URL_SAFE_NO_PAD.decode(q.key.as_str()) else {
        return Ok(GotoTemplate {
            state: SecretState::Invalid,
            key: q.key,
        });
    };

    if key.len() != 32 {
        return Ok(GotoTemplate {
            state: SecretState::Invalid,
            key: q.key,
        });
    }

    let mut tx = match state.pool.begin().await {
        Ok(tx) => tx,
        Err(err) => {
            tracing::error!(?err, "error starting transaction");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    match state.db.check_state(&mut tx, &id).await {
        Err(err) => {
            tracing::error!(?err, "error fetching state");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
        Ok(state) => {
            if let Err(err) = tx.commit().await {
                tracing::error!(?err, "error committing transaction");
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
            Ok(GotoTemplate { state, key: q.key })
        }
    }
}

#[tracing::instrument(skip(state, q), err)]
async fn consume<DB: SecretControl>(
    State(state): State<AppState<DB>>,
    Path(id): Path<uuid::Uuid>,
    Query(q): Query<CipherKey>,
) -> Result<ConsumeTemplate, StatusCode> {
    let mut tx = match state.pool.begin().await {
        Ok(tx) => tx,
        Err(err) => {
            tracing::error!(?err, "error starting transaction");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    match state.db.check_state(&mut tx, &id).await {
        Err(err) => {
            tracing::error!(?err, "error fetching state");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        Ok(SecretState::Expired | SecretState::Consumed) => {
            return Err(StatusCode::BAD_REQUEST);
        }
        _ => {}
    }

    let secret = state.db.get_secret_for_update(&mut tx, &id).await;
    let secret = match secret {
        Ok(s) => s,
        Err(err) => {
            tracing::error!(?err, "error fetching secret");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let key = match general_purpose::URL_SAFE_NO_PAD.decode(q.key.as_str()) {
        Ok(key) => key,
        Err(err) => {
            tracing::error!(?err, "error decoding key");
            return Err(StatusCode::BAD_REQUEST);
        }
    };
    if key.len() != 32 {
        tracing::error!("key length is not 32 bytes");
        return Err(StatusCode::BAD_REQUEST);
    }

    let key = Key::<Aes256Gcm>::from_slice(&key);
    let aes = Aes256Gcm::new(key);

    let s = match secret {
        Some(s) => s,
        None => {
            tracing::error!("secret not found");
            return Err(StatusCode::NOT_FOUND);
        }
    };

    let cipher = s.secret.unwrap_or_default();
    let iv = s.iv.unwrap_or_default();

    let nounce = match general_purpose::URL_SAFE_NO_PAD.decode(iv.as_str()) {
        Ok(nounce) => nounce,
        Err(err) => {
            tracing::error!(?err, "error decoding iv");
            return Err(StatusCode::BAD_REQUEST);
        }
    };
    if nounce.len() != 12 {
        tracing::error!("nounce length is not 12 bytes");
        return Err(StatusCode::BAD_REQUEST);
    }

    let cipher = match general_purpose::URL_SAFE_NO_PAD.decode(cipher.as_str()) {
        Ok(cipher) => cipher,
        Err(err) => {
            tracing::error!(?err, "error decoding cipher");
            return Err(StatusCode::BAD_REQUEST);
        }
    };
    if cipher.is_empty() {
        tracing::error!("cipher length is 0");
        return Err(StatusCode::BAD_REQUEST);
    }

    let nounce = GenericArray::from_slice(&nounce);
    let plain = match aes.decrypt(nounce, cipher.as_ref()) {
        Ok(plain) => plain,
        Err(err) => {
            tracing::error!(?err, "error decrypting cipher");
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    let plain = String::from_utf8_lossy(&plain);

    match state.db.consume_secret(&mut tx, &id).await {
        Ok(()) => (),
        Err(err) => {
            tracing::error!(?err, "error consuming secret");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    if let Err(err) = tx.commit().await {
        tracing::error!(?err, "error committing transaction");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    Ok(ConsumeTemplate {
        secret: plain.to_string(),
    })
}
