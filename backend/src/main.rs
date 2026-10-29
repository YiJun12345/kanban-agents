mod agent;
mod domain;
mod events;
mod git;
mod graphql;
mod tasks;

use async_graphql::http::GraphiQLSource;
use async_graphql_axum::{GraphQL, GraphQLSubscription};
use axum::{response::Html, routing::get, Router};
use sqlx::PgPool;
use tokio::sync::broadcast;

async fn graphiql() -> Html<String> {
    Html(
        GraphiQLSource::build()
            .endpoint("/graphql")
            .subscription_endpoint("/graphql/ws")
            .finish(),
    )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let pool = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;
    sqlx::migrate!().run(&pool).await?; // applies migrations/ on startup

    // Events channel: agents and mutations send, subscriptions receive
    let (events, _) = broadcast::channel(256);
    let schema = graphql::build_schema(pool, events);

    let app = Router::new()
        .route("/graphql", get(graphiql).post_service(GraphQL::new(schema.clone())))
        .route_service("/graphql/ws", GraphQLSubscription::new(schema));

    // PORT lets the e2e tests run a second backend next to your dev one
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".into());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    println!("GraphiQL at http://localhost:{port}/graphql");
    axum::serve(listener, app).await?;
    Ok(())
}
