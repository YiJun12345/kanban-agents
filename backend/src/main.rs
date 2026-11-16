mod agent;
mod claude;
mod domain;
mod events;
mod git;
mod graphql;
mod journal;
mod ledger;
mod review;
mod tasks;

use async_graphql::http::GraphiQLSource;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse, GraphQLSubscription};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::{response::Html, routing::get, Router};
use sqlx::PgPool;
use tokio::sync::broadcast;

use graphql::{AppSchema, Bearer};

/// Run one GraphQL request. The worker token, if the request has one, is handed to the
/// resolvers as request data; they decide whether an operation needs it.
async fn run_graphql(
    State(schema): State<AppSchema>,
    headers: HeaderMap,
    request: GraphQLRequest,
) -> GraphQLResponse {
    let token = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(|token| token.trim().to_string());
    schema.execute(request.into_inner().data(Bearer(token))).await.into()
}

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
    ledger::recover(&pool, &events).await?; // no built-in runner survives a restart

    // Every 10 seconds, end the claims of workers that have gone silent
    {
        let (pool, events) = (pool.clone(), events.clone());
        tokio::spawn(async move {
            let mut every = tokio::time::interval(std::time::Duration::from_secs(10));
            loop {
                every.tick().await;
                if let Err(e) = ledger::expire(&pool, &events, ledger::LEASE).await {
                    tracing::error!("could not expire claims: {e:#}");
                }
            }
        });
    }

    let agents = agent::Agents::new(claude::ClaudeConfig::from_env());
    if let Some(problem) = agents.claude.problem() {
        println!("Claude agent is off: {problem}");
    }
    let schema = graphql::build_schema(pool, events, agents);

    let app = Router::new()
        .route("/graphql", get(graphiql).post(run_graphql))
        .route_service("/graphql/ws", GraphQLSubscription::new(schema.clone()))
        .with_state(schema);

    // PORT lets the e2e tests run a second backend next to your dev one
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".into());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    println!("GraphiQL at http://localhost:{port}/graphql");
    axum::serve(listener, app).await?;
    Ok(())
}
