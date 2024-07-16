use std::{
    convert::Infallible,
    time::{Duration, Instant},
};

use axum::{
    response::{
        sse::{Event, KeepAlive},
        Sse,
    },
    routing::get,
    Router,
};
use futures::Stream;
use lipsum::lipsum;
use maud::{html, Markup};
use tokio::time::sleep;

pub fn router() -> Router {
    Router::new()
        .route("/", get(page))
        .route("/events", get(events))
}

async fn events() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = futures::stream::unfold(Instant::now(), |next| async move {
        let now = Instant::now();
        if next > now {
            sleep(next - now).await;
        }

        let mut ipsum = lipsum(rand::random::<usize>() % 10);
        ipsum.push(' ');

        Some((
            Ok(Event::default().event("lorem_ipsum").data(ipsum)),
            next + Duration::from_millis(500),
        ))
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn page() -> Markup {
    html!(html {
        head {
            script src="https://unpkg.com/htmx.org@1.9.12" integrity="sha384-ujb1lZYygJmzgSwoxRggbCHcjc0rB2XoQrxeTUQyRjrOnlCoYta87iKBWq3EsdM2" crossorigin="anonymous" {}
            script src="https://unpkg.com/htmx.org@1.9.12/dist/ext/sse.js" {}
        }
        body hx-ext="sse" sse-connect="/lorem_ipsum/events" sse-swap="lorem_ipsum" hx-swap="beforeend" {}
    })
}
