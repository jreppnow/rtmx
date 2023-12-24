use super::templates::generated::index_html;
use axum::response::Html;

pub async fn index() -> Html<String> {
    let mut body: Vec<u8> = Vec::new();
    index_html(&mut body).expect("Template instantiation should never fail!");
    Html(String::from_utf8(body).unwrap())
}
