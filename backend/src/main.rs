mod domain;
mod graphql;

use async_graphql::http::GraphiQLSource;
use async_graphql_axum::GraphQL;
use axum::{response::Html, routing::get, Router};
use sqlx::PgPool;

async fn graphiql() -> Html<String> {
    Html(GraphiQLSource::build().endpoint("/graphql").finish())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let pool = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;
    sqlx::migrate!().run(&pool).await?; // applies migrations/ on startup

    let schema = graphql::build_schema(pool);

    let app = Router::new().route("/graphql", get(graphiql).post_service(GraphQL::new(schema)));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("GraphiQL at http://localhost:3000/graphql");
    axum::serve(listener, app).await?;
    Ok(())
}
