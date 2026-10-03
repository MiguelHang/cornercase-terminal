use std::time::Duration;

use serde_json::Value;
use ureq::Agent;
use ureq::http::Response;

use crate::error::{Error, Result};

const TIMEOUT: Duration = Duration::from_secs(20);

pub struct Service {
    pub name: &'static str,
    pub rejected: &'static str,
}

pub struct Answer {
    pub status: u16,
    pub json: Value,
}

impl Answer {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

fn agent() -> Agent {
    Agent::config_builder().timeout_global(Some(TIMEOUT)).http_status_as_error(false).build().into()
}

pub fn get(service: &Service, url: &str, query: &[(&str, &str)], headers: &[(&str, &str)]) -> Result<Answer> {
    let mut request = agent().get(url).header("Accept", "application/json");
    for (key, value) in headers {
        request = request.header(*key, *value);
    }
    finish(service, request.query_pairs(query.iter().copied()).call())
}

pub fn post(service: &Service, url: &str, headers: &[(&str, &str)], body: &Value) -> Result<Answer> {
    let mut request = agent().post(url).header("Accept", "application/json");
    for (key, value) in headers {
        request = request.header(*key, *value);
    }
    finish(service, request.content_type("application/json").send(body.to_string()))
}

fn finish(service: &Service, result: std::result::Result<Response<ureq::Body>, ureq::Error>) -> Result<Answer> {
    let mut response = result.map_err(|e| match e {
        ureq::Error::Timeout(_) => {
            Error::Api(format!("{} did not answer within {} s", service.name, TIMEOUT.as_secs()))
        }
        e => Error::Api(format!("could not reach {}: {e}", service.name)),
    })?;
    let status = response.status().as_u16();
    if status == 401 || status == 403 {
        return Err(Error::Api(service.rejected.into()));
    }
    if status == 429 {
        return Err(Error::Api(format!("{} rate limit reached, try again in a minute", service.name)));
    }
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|e| Error::Api(format!("could not read the answer of {}: {e}", service.name)))?;
    let json = serde_json::from_str(&text).unwrap_or(Value::Null);
    Ok(Answer { status, json })
}

pub fn origin(url: &str) -> &str {
    let after_scheme = url.find("://").map_or(0, |i| i + 3);
    url[after_scheme..].find('/').map_or(url, |i| &url[..after_scheme + i])
}

pub fn text(value: &Value, pointer: &str) -> String {
    value.pointer(pointer).and_then(Value::as_str).unwrap_or_default().to_string()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::test_util::FakeHttp;

    const SERVICE: Service = Service { name: "Tracker", rejected: "Tracker rejected the token" };

    #[rstest]
    #[case::with_path("https://api.app.shortcut.com/api/v3", "https://api.app.shortcut.com")]
    #[case::without_path("http://127.0.0.1:4000", "http://127.0.0.1:4000")]
    fn origin_is_scheme_and_host(#[case] url: &str, #[case] expected: &str) {
        assert_eq!(origin(url), expected);
    }

    #[test]
    fn get_sends_the_headers_and_the_query() {
        let server = FakeHttp::start(vec![("GET /things", 200, r#"{"a":1}"#)]);

        let answer =
            get(&SERVICE, &format!("{}/things", server.url()), &[("q", "a b")], &[("Token", "t0k")]).expect("get");

        let request = server.request(0);
        assert_eq!((answer.status, answer.json["a"].as_i64()), (200, Some(1)));
        assert!(request.starts_with("GET /things?q=a+b ") || request.starts_with("GET /things?q=a%20b "), "{request}");
        assert!(request.to_lowercase().contains("token: t0k"), "{request}");
    }

    #[test]
    fn post_sends_json() {
        let server = FakeHttp::start(vec![("POST /graphql", 200, r#"{"data":{}}"#)]);

        post(&SERVICE, &format!("{}/graphql", server.url()), &[], &serde_json::json!({"query": "q"})).expect("post");

        assert!(server.request(0).ends_with(r#"{"query":"q"}"#), "{}", server.request(0));
    }

    #[rstest]
    #[case::unauthorized(401, "Tracker rejected the token")]
    #[case::forbidden(403, "Tracker rejected the token")]
    #[case::rate_limited(429, "Tracker rate limit reached, try again in a minute")]
    fn some_answers_are_errors(#[case] status: u16, #[case] message: &str) {
        let server = FakeHttp::start(vec![("GET /x", status, "{}")]);
        let err = get(&SERVICE, &format!("{}/x", server.url()), &[], &[]).err().map(|e| e.to_string());
        assert_eq!(err.as_deref(), Some(message));
    }

    #[test]
    fn an_unreachable_server_says_so() {
        let err = get(&SERVICE, "http://127.0.0.1:1/x", &[], &[]).err().map(|e| e.to_string()).unwrap_or_default();
        assert!(err.starts_with("could not reach Tracker"), "{err}");
    }
}
