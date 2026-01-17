use axum::{
    http::{header, Method},
    http::HeaderValue,
};
use tower_http::cors::{CorsLayer};

pub fn create_cors_layer() -> CorsLayer {
    
    let allowed_origins = vec![
        "http://localhost:3000".parse::<HeaderValue>().unwrap(),
        "https://omnixec.com".parse::<HeaderValue>().unwrap(),  // not yet live 
    ];

    CorsLayer::new()
        .allow_origin(allowed_origins) 
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            header::ACCEPT,
            header::ORIGIN,
        ])
        .allow_credentials(true)
        .max_age(std::time::Duration::from_secs(60 * 60)) // 1 hour
}
