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

pub async fn problem_lookup(id_list: &[u32]) -> worker::Result<Vec<Map<String, Value>>> {
    let ids: String = id_list.iter().map(|id| format!(",{}", id)).collect();
    let url = format!(
        concat!(api_url!("/problem/lookup"), "?problemIds={}"),
        &ids[1..]
    );
    let request = Request::new(&url, Method::Get)?;
    let mut response = Fetch::Request(request).send().await?;
    if response.status_code() == 200 {
        response.json().await
    } else {
        Ok(vec![])
    }
}

pub async fn user_show(handle: &str) -> worker::Result<Option<Map<String, Value>>> {
    let handle = encode_uri(handle);
    let url = format!(concat!(api_url!("/user/show"), "?handle={}"), handle);
    let request = Request::new(&url, Method::Get)?;
    let mut response = Fetch::Request(request).send().await?;

    if response.status_code() == 200 {
        let value = response.json().await?;
        Ok(Some(value))
    } else {
        Ok(None)
    }
}
