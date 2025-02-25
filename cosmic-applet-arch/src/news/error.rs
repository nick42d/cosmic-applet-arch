#[derive(Debug)]
pub enum NewsError {
    Web(reqwest::Error),
    Rss(rss::Error),
}

impl From<rss::Error> for NewsError {
    fn from(e: rss::Error) -> NewsError {
        NewsError::Rss(e)
    }
}
impl From<reqwest::Error> for NewsError {
    fn from(e: reqwest::Error) -> NewsError {
        NewsError::Web(e)
    }
}
