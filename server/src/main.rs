mod api;
mod config;
mod relay;
mod signaling;

use actix_cors::Cors;
use actix_web::{web, App, HttpServer, middleware};
use std::sync::Arc;

use config::ServerConfig;
use relay::RelayState;
use signaling::SignalingState;

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let config = ServerConfig::from_env();
    log::info!("Starting rem0te-server on port {}", config.api_port);

    let relay_state = Arc::new(RelayState::new());
    let signaling_state = Arc::new(SignalingState::new());

    // Spawn the TCP signaling server
    {
        let state = signaling_state.clone();
        let port = config.signaling_port;
        tokio::spawn(async move {
            if let Err(e) = signaling::run_tcp_server(state, port).await {
                log::error!("Signaling TCP server error: {e}");
            }
        });
    }

    // Spawn the TCP relay server
    {
        let state = relay_state.clone();
        let port = config.relay_port;
        tokio::spawn(async move {
            if let Err(e) = relay::run_relay_server(state, port).await {
                log::error!("Relay server error: {e}");
            }
        });
    }

    // Spawn WebSocket signaling server
    {
        let state = signaling_state.clone();
        let port = config.ws_port;
        tokio::spawn(async move {
            if let Err(e) = signaling::run_ws_server(state, port).await {
                log::error!("WebSocket signaling server error: {e}");
            }
        });
    }

    // HTTP API server
    let api_state = api::AppState::new(
        signaling_state.clone(),
        relay_state.clone(),
        config.jwt_secret.clone(),
    );

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .wrap(middleware::Logger::default())
            .app_data(web::Data::new(api_state.clone()))
            .configure(api::configure_routes)
    })
    .bind(("0.0.0.0", config.api_port))?
    .run()
    .await?;

    Ok(())
}
