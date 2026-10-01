//! Helpers to publish scraper-specific parser output in the shared metadata
//! contract. Source parsing and fallback remain private to each plugin.

use serde::Deserialize;
use serde_json::Value;
use ting_plugin_sdk::contract::scraper::{ScraperResult, SearchPage, SearchRequest};
use ting_plugin_sdk::{Result, SdkError};

#[derive(Deserialize)]
struct ParsedPage {
    items: Vec<ParsedItem>,
    page: u32,
    page_size: u32,
    // Some websites only expose the current page; zero from the parser
    // means no total was observed, not that the result set is empty.
    total: Option<u64>,
    has_more: Option<bool>,
}

#[derive(Deserialize)]
struct ParsedItem {
    id: Option<String>,
    source_url: Option<String>,
    title: String,
    author: Option<String>,
    narrator: Option<String>,
    cover_url: Option<String>,
    intro: Option<String>,
    subtitle: Option<String>,
    publisher: Option<String>,
    language: Option<String>,
    genre: Option<String>,
    published_year: Option<Value>,
    published_date: Option<String>,
    isbn: Option<String>,
    asin: Option<String>,
    explicit: Option<bool>,
    abridged: Option<bool>,
    #[serde(default)]
    tags: Vec<String>,
    duration: Option<u64>,
    score: Option<f64>,
    chapter_title_template: Option<String>,
    #[serde(default)]
    chapter_titles: Vec<String>,
}

fn optional_string(input: Option<String>) -> Option<String> {
    input
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn publish_item(item: ParsedItem) -> Result<ScraperResult> {
    let mut seen_tags = std::collections::HashSet::new();
    let mut tags = Vec::new();
    for tag in item.tags {
        let tag = tag.trim();
        if !tag.is_empty() && seen_tags.insert(tag.to_owned()) {
            tags.push(tag.to_owned());
        }
    }
    let published_year = match item.published_year {
        Some(Value::Number(year)) => year.as_u64().and_then(|year| u16::try_from(year).ok()),
        Some(Value::String(year)) => year.parse::<u16>().ok(),
        None | Some(Value::Null) => None,
        _ => return Err(SdkError::invalid("Invalid published year")),
    };
    let published = ScraperResult {
        id: optional_string(item.id),
        source_url: optional_string(item.source_url),
        title: item.title.trim().to_owned(),
        author: optional_string(item.author),
        narrator: optional_string(item.narrator),
        cover_url: optional_string(item.cover_url),
        intro: optional_string(item.intro),
        subtitle: optional_string(item.subtitle),
        publisher: optional_string(item.publisher),
        language: optional_string(item.language),
        genre: optional_string(item.genre),
        published_year,
        published_date: optional_string(item.published_date),
        isbn: optional_string(item.isbn),
        asin: optional_string(item.asin),
        explicit: item.explicit,
        abridged: item.abridged,
        tags,
        duration: item.duration,
        score: item.score,
        chapter_title_template: optional_string(item.chapter_title_template),
        chapter_titles: item.chapter_titles,
    };
    published.validate().map_err(SdkError::invalid)?;
    Ok(published)
}

/// Convert a platform parser's strongly typed result into the public result.
/// Undeclared parser-only fields (such as chapter counts) are not exported.
pub fn publish_search<T: serde::Serialize>(parsed: T) -> Result<Value> {
    let parsed: ParsedPage =
        serde_json::from_value(serde_json::to_value(parsed).map_err(SdkError::parse)?)
            .map_err(SdkError::parse)?;
    let page = SearchPage {
        items: parsed
            .items
            .into_iter()
            .map(publish_item)
            .collect::<Result<_>>()?,
        page: parsed.page,
        page_size: parsed.page_size,
        total: parsed.total.filter(|total| *total != 0),
        has_more: parsed.has_more,
    };
    page.validate().map_err(SdkError::invalid)?;
    serde_json::to_value(page).map_err(SdkError::parse)
}

/// Publish a platform search page without lying about a fixed upstream page
/// size. Truncating a 20-item upstream page to a requested size of 10 would
/// silently skip items when the client requests page 2.
pub fn publish_search_for_request<T: serde::Serialize>(
    request: &Value,
    parsed: T,
) -> Result<Value> {
    let request: SearchRequest =
        serde_json::from_value(request.clone()).map_err(SdkError::parse)?;
    request.validate().map_err(SdkError::invalid)?;
    let parsed: ParsedPage =
        serde_json::from_value(serde_json::to_value(parsed).map_err(SdkError::parse)?)
            .map_err(SdkError::parse)?;
    if parsed.page != request.page {
        return Err(SdkError::invalid("Source page differs from the request"));
    }
    let page = SearchPage {
        items: parsed
            .items
            .into_iter()
            .map(publish_item)
            .collect::<Result<_>>()?,
        page: request.page,
        page_size: parsed.page_size,
        total: parsed.total.filter(|total| *total != 0),
        has_more: parsed.has_more,
    };
    page.validate().map_err(SdkError::invalid)?;
    serde_json::to_value(page).map_err(SdkError::parse)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn publishes_null_unknowns_and_preserves_known_match_fields() {
        let parsed = json!({
            "items": [{"id": "", "title": " Book ", "author": "", "tags": [" A ", "A"],
                "published_year": "2020", "score": 0.8, "chapter_count": 3}],
            "page": 1, "page_size": 20, "total": 0
        });
        let result = publish_search(parsed).unwrap();
        assert_eq!(result["total"], Value::Null);
        assert_eq!(result["has_more"], Value::Null);
        assert_eq!(result["items"][0]["id"], Value::Null);
        assert_eq!(result["items"][0]["author"], Value::Null);
        assert_eq!(result["items"][0]["tags"], json!(["A"]));
        assert_eq!(result["items"][0]["published_year"], 2020);
        assert!(result["items"][0].get("chapter_count").is_none());
    }

    #[test]
    fn fixed_upstream_page_size_is_reported_truthfully() {
        let request = json!({
            "title": "Book", "author": null, "narrator": null,
            "page": 2, "page_size": 10, "filters": {},
            "chapter_candidates": [], "context": null
        });
        let source = json!({
            "items": [{"id": "a", "title": "Book"}],
            "page": 2, "page_size": 20, "total": 0
        });
        let page = publish_search_for_request(&request, source.clone()).unwrap();
        assert_eq!(page["page_size"], 20);
        assert_eq!(page["total"], Value::Null);
        let mut different_page = source;
        different_page["page"] = json!(1);
        assert!(publish_search_for_request(&request, different_page).is_err());
    }
}
