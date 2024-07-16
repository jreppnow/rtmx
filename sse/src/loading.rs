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
use maud::{html, Markup, Render};
use stylers::style;
use tokio::time::sleep;

pub fn router() -> Router {
    Router::new()
        .route("/", get(loading))
        .route("/events", get(loading_events))
        .route("/done", get(loading_done))
}

fn draw_loading_bar(step: usize) -> Markup {
    let style = style! {
        div {
            background-color: darkblue;
            height: 12px;
            border-width: 0px;
        }
    };

    html! {
        div.{(style)} style={(format!("width: {}%", step * 10))} sse-swap="pending" hx-swap="outerHTML" {}
    }
}

async fn loading() -> Markup {
    let body_s = style! {
        body {
            height: 100%;
            width: 100%;
            margin: 0;

            display: flex;
            flex-direction: row;
            align-items: center;
            justify-content: center;
        }
    };

    let box_s = style!(div {
        border: 1px solid;
        border-radius: 4px;
        display: flex;
        flex-direction: row;
        justify-content: start;
        align-items: center;
        padding: 4px;
        width: 80%;
    });

    html!(html {
        head {
            script src="https://unpkg.com/htmx.org@1.9.12" integrity="sha384-ujb1lZYygJmzgSwoxRggbCHcjc0rB2XoQrxeTUQyRjrOnlCoYta87iKBWq3EsdM2" crossorigin="anonymous" {}
            script src="https://unpkg.com/htmx.org@1.9.12/dist/ext/sse.js" {}
            link rel="stylesheet" href="/assets/main.css" {}
        }
        body.{(body_s)} {
            div.{(box_s)} hx-ext="sse" sse-connect="/loading/events" {
                input type="hidden"  hx-get="/loading/done" hx-trigger="sse:done" hx-swap="outerHTML" hx-target="closest div";
                {(draw_loading_bar(0))}
            }
        }
    })
}

async fn loading_events() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    struct State {
        next: Instant,
        last_value: usize,
    }

    let stream = futures::stream::unfold(
        State {
            next: Instant::now(),
            last_value: 0,
        },
        |state| async move {
            if state.last_value > 11 {
                return None;
            }

            let event = if state.last_value > 10 {
                Event::default().event("done").data("")
            } else {
                let now = Instant::now();
                if state.next > now {
                    sleep(state.next - now).await;
                }

                let mut bar = String::new();
                draw_loading_bar(state.last_value).render_to(&mut bar);

                Event::default().event("pending").data(bar)
            };

            let state = State {
                last_value: state.last_value + 1,
                next: state.next + Duration::from_millis(500),
            };

            Some((Ok(event), state))
        },
    );

    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn loading_done() -> Markup {
    let style = style! {
        p {
            font-size: 200px;
            font-family: "Apple LiGothic";
        }
    };

    html! {
        p.{(style)} {
            {("完")}
        }
    }
}
