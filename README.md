# ting-scraper-sdk

用于 Ting Reader 元数据插件，将源站解析结果转换为分页搜索结果，处理未知字段、标签去重、出版年份和结果校验。

## 添加依赖

推荐使用 Rust 1.93 或更高版本。在插件的 `Cargo.toml` 中添加：

```toml
[dependencies]
ting-plugin-sdk = { git = "https://github.com/dqsq2e2/ting-plugin-sdk.git", tag = "v2.0.3" }
ting-scraper-sdk = { git = "https://github.com/dqsq2e2/ting-scraper-sdk.git", tag = "v2.0.3" }
serde_json = "1"
```

## 返回搜索结果

在插件的 `search` 处理中，用 `publish_search_for_request` 转换解析结果：

```rust
use serde_json::{Value, json};
use ting_plugin_sdk::{Result, SdkError};
use ting_plugin_sdk::contract::scraper::SearchRequest;

fn search(input: Value) -> Result<Value> {
    let request: SearchRequest =
        serde_json::from_value(input.clone()).map_err(SdkError::parse)?;
    request.validate().map_err(SdkError::invalid)?;

    // 实际开发时，替换为源站请求和解析结果。
    let items = request.title.as_deref()
        .filter(|title| !title.trim().is_empty())
        .map(|title| vec![json!({ "title": title })])
        .unwrap_or_default();

    ting_scraper_sdk::publish_search_for_request(&input, json!({
        "items": items,
        "page": request.page,
        "page_size": request.page_size,
        "total": null,
        "has_more": null,
    }))
}
```

`page_size` 填源站实际分页大小，`page` 与请求一致。未知总数和下一页状态使用 `null`。

每条记录至少提供非空 `title`。其他支持字段包括 `id`、`source_url`、`author`、`narrator`、`cover_url`、`intro`、出版信息、`tags`、`duration`、`score` 和章节标题。可空字段会补为 `null`，未知数组补为 `[]`；平台内部字段不会对外发布。

`publish_search` 适用于已处理分页请求的结果；通常优先使用 `publish_search_for_request`，校验源站页码与当前请求一致。

完整业务入口、构建、打包和安装步骤见 [插件开发指南](https://github.com/dqsq2e2/ting-reader/blob/main/docs/plugins/plugin-dev.md)，字段和限额见 [能力声明](https://github.com/dqsq2e2/ting-reader/blob/main/docs/plugins/capabilities.md)。

## 验证本仓库

```sh
cargo check --locked
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo check --locked --target wasm32-wasip1
```
