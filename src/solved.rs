use serde::Deserialize;
use serde_json::{Map, Value};
use worker::js_sys::encode_uri;
use worker::{Fetch, Method, Request};

macro_rules! api_url {
    ($endpoint:literal) => {
        concat!("https://solved.ac/api/v3", $endpoint)
    };
}

pub async fn search_problem(query: &str, page: u32) -> worker::Result<Vec<Map<String, Value>>> {
    let query = encode_uri(query);
    let url = format!(
        concat!(api_url!("/search/problem"), "?query={}&page={}"),
        query, page
    );
    let request = Request::new(&url, Method::Get)?;
    let mut response = Fetch::Request(request).send().await?;

    #[derive(Deserialize)]
    struct RawResponse {
        items: Vec<Map<String, Value>>,
    }

    if response.status_code() == 200 {
        let raw: RawResponse = response.json().await?;
        Ok(raw.items)
    } else {
        Ok(vec![])
    }
}
