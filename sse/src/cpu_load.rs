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
        .route("/", get(page))
        .route("/events", get(events))
}

#[derive(Clone, Copy, Debug)]
enum Color {
    Off,
    Okay,
    Warning,
    Critical,
}

impl From<Option<usize>> for Color {
    fn from(value: Option<usize>) -> Self {
        match value {
            None => Self::Off,
            Some(..6) => Self::Okay,
            Some(6..8) => Self::Warning,
            Some(8..) => Self::Critical,
        }
    }
}

fn loading_bar(color: Color) -> Markup {
    let base = style!(
        div {
            width: 12px;
            height: 48px;
            border-radius: 2px;
            animation: 250ms linear;
        }
    );

    let off = style! {
        div {
            background-color: none;
        }
    };

    let okay = style! {
        div {
            background-color: green;
        }
    };

    let warning = style! {
        div {
            background-color: yellow;
        }
    };

    let critical = style! {
        div {
            background-color: red;
        }
    };

    let class = match color {
        Color::Off => off,
        Color::Okay => okay,
        Color::Warning => warning,
        Color::Critical => critical,
    };

    html!(div.{(base)}.{(class)} {})
}

fn render_bars(percentage: usize) -> Markup {
    let bars = (percentage / 10) + 1;

    let loading_box = style!(div {
        display: flex;
        flex-direction: row;
        justify-content: start;
        align-items: center;
        gap: 4px;
        width: fit-content;
    });

    html!(
       div.{(loading_box)} sse-swap="bars" hx-swap="innerHTML" {
           @for value in 1..=10 {
               { (loading_bar(Color::from((value <= bars).then_some(value)))) }
           }
       }
    )
}

async fn page() -> Markup {
    let box_s = style!(div {
        border: 1px solid;
        border-radius: 4px;
        display: flex;
        flex-direction: row;
        justify-content: start;
        align-items: center;
        gap: 4px;
        padding: 4px;
        width: fit-content;
    });

    let percentage_s = style! {
        p {
            font-family: Andale Mono;
            font-size: 48px;
            margin: 0;
            min-width: 100px;
            text-align: right;
        }
    };

    let separator_s = style! {
        div {
            border-left: 1px solid black;
            height: 48px;
        }
    };

    html!(html {
        head {
            script src="https://unpkg.com/htmx.org@1.9.12" integrity="sha384-ujb1lZYygJmzgSwoxRggbCHcjc0rB2XoQrxeTUQyRjrOnlCoYta87iKBWq3EsdM2" crossorigin="anonymous" {}
            script src="https://unpkg.com/htmx.org@1.9.12/dist/ext/sse.js" {}
            link rel="stylesheet" href="/assets/main.css" {}
        }
        body {
            div.{(box_s)} hx-ext="sse" sse-connect="/cpu_load/events" {
                {(render_bars(100))}
                div.{(separator_s)} {}
                p.{(percentage_s)} sse-swap="percentage" hx-swap="innerHTML" {
                    {("100%")}
                }
            }
        }
    })
}

async fn events() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    struct State {
        next: Instant,
        last_value: usize,
        send_percentage: bool,
    }

    let stream = futures::stream::unfold(
        State {
            next: Instant::now(),
            last_value: 100,
            send_percentage: false,
        },
        |state| async move {
            if state.send_percentage {
                return Some((
                    Ok(Event::default()
                        .event("percentage")
                        .data(format!("{}%", state.last_value))),
                    State {
                        send_percentage: false,
                        ..state
                    },
                ));
            }

            let now = Instant::now();
            if state.next > now {
                sleep(state.next - now).await;
            }

            let percentage = rand::random::<usize>() % 100;

            let mut bars = String::new();
            render_bars(percentage).render_to(&mut bars);
            Some((
                Ok(Event::default().event("bars").data(bars)),
                State {
                    send_percentage: true,
                    last_value: percentage,
                    next: state.next + Duration::from_millis(500),
                },
            ))
        },
    );

    Sse::new(stream).keep_alive(KeepAlive::default())
}
