//! RSS 2.0 and Atom feed generation for renderling.xyz.
//!
//! Parses news entries from `content/news/index.md` and articles from
//! `content/articles/*.md`, then generates static XML feed files.

use std::path::Path;

use chrono::NaiveDate;

use crate::md;

/// A single item in the feed (either a news entry or an article).
#[derive(Debug, Clone)]
pub struct FeedItem {
    pub title: String,
    pub link: String,
    pub date: NaiveDate,
    pub content_html: String,
}

/// Render a markdown string to an HTML fragment (no page wrapper).
///
/// Uses the `markdown` crate directly to produce HTML, since we don't
/// need the full mogwai SSR page template for feed content.
fn markdown_to_html(md_content: &str) -> String {
    let opts = markdown::Options {
        parse: {
            let mut p = markdown::ParseOptions::gfm();
            p.constructs.frontmatter = true;
            p
        },
        compile: markdown::CompileOptions {
            allow_dangerous_html: true,
            ..markdown::CompileOptions::gfm()
        },
    };
    markdown::to_html_with_options(md_content, &opts).unwrap_or_default()
}

/// Generate an anchor id from a heading text, matching the logic in `md.rs`.
fn heading_to_anchor(heading_text: &str) -> String {
    let mut last_char_is_hyphen = false;
    heading_text
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() {
                last_char_is_hyphen = false;
                Some(c.to_lowercase().to_string())
            } else if c.is_whitespace() && last_char_is_hyphen {
                None
            } else if c.is_whitespace() {
                last_char_is_hyphen = true;
                Some("-".to_owned())
            } else if c == '-' && last_char_is_hyphen {
                None
            } else if c == '-' {
                last_char_is_hyphen = true;
                Some("-".to_owned())
            } else if c == ',' {
                // commas are stripped, matching the as_link_text behavior
                None
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .concat()
}

/// Parse news date strings like "Wed 11 Feb, 2026" or "Sun 30 Nov, 2025".
fn parse_news_date(date_str: &str) -> Option<NaiveDate> {
    // Try the primary format: "Wed 11 Feb, 2026"
    if let Ok(d) = NaiveDate::parse_from_str(date_str.trim(), "%a %d %b, %Y") {
        return Some(d);
    }
    // Try without comma: "Wed 24 September 2025"
    if let Ok(d) = NaiveDate::parse_from_str(date_str.trim(), "%a %d %B, %Y") {
        return Some(d);
    }
    if let Ok(d) = NaiveDate::parse_from_str(date_str.trim(), "%a %d %B %Y") {
        return Some(d);
    }
    log::warn!("Could not parse news date: '{date_str}'");
    None
}

/// Strip YAML frontmatter from markdown content (the `---` delimited block).
fn strip_frontmatter(content: &str) -> &str {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return content;
    }
    // Find the closing ---
    if let Some(end) = trimmed[3..].find("---") {
        let after = &trimmed[3 + end + 3..];
        after.trim_start_matches('\n').trim_start_matches('\r')
    } else {
        content
    }
}

/// Parse news entries from the news/index.md content.
///
/// Splits on `## <date>` headings. Each date section (with all its ### sub-topics)
/// becomes a single feed item.
pub fn parse_news_entries(content: &str, root_url: &str) -> Vec<FeedItem> {
    let body = strip_frontmatter(content);

    let mut items = Vec::new();
    let mut current_date: Option<NaiveDate> = None;
    let mut current_title: Option<String> = None;
    let mut current_anchor: Option<String> = None;
    let mut current_lines: Vec<&str> = Vec::new();

    // Helper to flush the accumulated entry
    let flush = |date: &Option<NaiveDate>,
                 title: &Option<String>,
                 anchor: &Option<String>,
                 lines: &[&str],
                 items: &mut Vec<FeedItem>,
                 root_url: &str| {
        if let (Some(date), Some(title), Some(anchor)) = (date, title, anchor) {
            let md_content = lines.join("\n");
            let html = markdown_to_html(&md_content);
            if !html.trim().is_empty() {
                items.push(FeedItem {
                    title: title.clone(),
                    link: format!("{}/news/index.html#{}", root_url, anchor),
                    date: *date,
                    content_html: html,
                });
            }
        }
    };

    for line in body.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            let heading = heading.trim();
            // Flush the previous entry
            flush(
                &current_date,
                &current_title,
                &current_anchor,
                &current_lines,
                &mut items,
                root_url,
            );

            // Start new entry
            current_date = parse_news_date(heading);
            current_title = Some(format!("Renderling News - {}", heading));
            current_anchor = Some(heading_to_anchor(heading));
            current_lines.clear();
        } else {
            current_lines.push(line);
        }
    }

    // Flush the last entry
    flush(
        &current_date,
        &current_title,
        &current_anchor,
        &current_lines,
        &mut items,
        root_url,
    );

    items
}

/// Parse articles from the articles directory.
///
/// Reads each `.md` file, extracts frontmatter title and date,
/// and renders the full content to HTML.
/// Skips:
/// * index.md
/// * adages.md
/// * files under live/
/// * any article with `exclude_from_rss: true` in its frontmatter.
pub fn parse_articles(articles_dir: &Path, root_url: &str) -> Vec<FeedItem> {
    let mut items = Vec::new();

    let entries = match std::fs::read_dir(articles_dir) {
        Ok(entries) => entries,
        Err(e) => {
            log::warn!("Could not read articles directory: {e}");
            return items;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();

        // Skip directories (like live/)
        if path.is_dir() {
            continue;
        }

        // Only process .md files
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Skip index.md and adages.md
        if filename == "index.md" || filename == "adages.md" {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Could not read {}: {e}", path.display());
                continue;
            }
        };

        // Parse frontmatter
        let mut opts = markdown::ParseOptions::gfm();
        opts.constructs.frontmatter = true;
        let mut node = match markdown::to_mdast(&content, &opts) {
            Ok(n) => n,
            Err(e) => {
                log::warn!("Could not parse markdown for {}: {e}", path.display());
                continue;
            }
        };

        let meta: Option<md::ContentMeta> =
            md::get_frontmatter(&mut node).and_then(|fm| serde_yaml::from_str(&fm.value).ok());

        let title = meta
            .as_ref()
            .map(|m| m.title.clone())
            .unwrap_or_else(|| filename.trim_end_matches(".md").replace('_', " "));

        let should_exclude = meta
            .as_ref()
            .map(|m| m.exclude_from_rss)
            .unwrap_or_default();
        if should_exclude {
            log::warn!("Skipping article '{title}': exclude_from_rss is true");
            continue;
        }

        let date = meta
            .as_ref()
            .and_then(|m| m.date.as_deref())
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

        let date = match date {
            Some(d) => d,
            None => {
                log::warn!(
                    "Skipping article '{}': no valid date in frontmatter",
                    path.display()
                );
                continue;
            }
        };

        // Build the link path: /articles/<stem>.html
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let link = format!("{}/articles/{}.html", root_url, stem);

        // Render content to HTML (without frontmatter)
        let body = strip_frontmatter(&content);
        let html = markdown_to_html(body);

        items.push(FeedItem {
            title,
            link,
            date,
            content_html: html,
        });
    }

    items
}

/// Escape text for use in XML (outside of CDATA sections).
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Escape content for CDATA sections. CDATA cannot contain "]]>".
fn cdata_escape(s: &str) -> String {
    s.replace("]]>", "]]]]><![CDATA[>")
}

/// Generate an RSS 2.0 feed XML string.
pub fn generate_rss(root_url: &str, items: &[FeedItem]) -> String {
    let last_build_date = items
        .iter()
        .map(|i| i.date)
        .max()
        .unwrap_or_else(|| chrono::Utc::now().date_naive());

    let last_build_rfc2822 = last_build_date
        .and_hms_opt(12, 0, 0)
        .unwrap()
        .and_utc()
        .format("%a, %d %b %Y %H:%M:%S +0000")
        .to_string();

    let mut xml = String::new();
    xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push('\n');
    xml.push_str(r#"<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">"#);
    xml.push('\n');
    xml.push_str("  <channel>\n");
    xml.push_str("    <title>Renderling</title>\n");
    xml.push_str(&format!("    <link>{}</link>\n", xml_escape(root_url)));
    xml.push_str("    <description>News and articles from renderling.xyz</description>\n");
    xml.push_str("    <language>en</language>\n");
    xml.push_str(&format!(
        "    <lastBuildDate>{}</lastBuildDate>\n",
        last_build_rfc2822
    ));
    xml.push_str(&format!(
        "    <atom:link href=\"{}/feed.xml\" rel=\"self\" type=\"application/rss+xml\" />\n",
        xml_escape(root_url)
    ));

    for item in items {
        let pub_date = item
            .date
            .and_hms_opt(12, 0, 0)
            .unwrap()
            .and_utc()
            .format("%a, %d %b %Y %H:%M:%S +0000")
            .to_string();

        xml.push_str("    <item>\n");
        xml.push_str(&format!(
            "      <title>{}</title>\n",
            xml_escape(&item.title)
        ));
        xml.push_str(&format!("      <link>{}</link>\n", xml_escape(&item.link)));
        xml.push_str(&format!("      <guid>{}</guid>\n", xml_escape(&item.link)));
        xml.push_str(&format!("      <pubDate>{}</pubDate>\n", pub_date));
        xml.push_str(&format!(
            "      <description><![CDATA[{}]]></description>\n",
            cdata_escape(&item.content_html)
        ));
        xml.push_str("    </item>\n");
    }

    xml.push_str("  </channel>\n");
    xml.push_str("</rss>\n");
    xml
}

/// Generate an Atom feed XML string.
pub fn generate_atom(root_url: &str, items: &[FeedItem]) -> String {
    let updated = items
        .iter()
        .map(|i| i.date)
        .max()
        .unwrap_or_else(|| chrono::Utc::now().date_naive());

    let updated_str = updated
        .and_hms_opt(12, 0, 0)
        .unwrap()
        .and_utc()
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();

    let mut xml = String::new();
    xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push('\n');
    xml.push_str(r#"<feed xmlns="http://www.w3.org/2005/Atom">"#);
    xml.push('\n');
    xml.push_str("  <title>Renderling</title>\n");
    xml.push_str(&format!(
        "  <link href=\"{}\" rel=\"alternate\" />\n",
        xml_escape(root_url)
    ));
    xml.push_str(&format!(
        "  <link href=\"{}/atom.xml\" rel=\"self\" type=\"application/atom+xml\" />\n",
        xml_escape(root_url)
    ));
    xml.push_str(&format!("  <id>{}</id>\n", xml_escape(root_url)));
    xml.push_str(&format!("  <updated>{}</updated>\n", updated_str));
    xml.push_str("  <author>\n");
    xml.push_str("    <name>Schell Scivally</name>\n");
    xml.push_str("  </author>\n");
    xml.push_str("  <subtitle>News and articles from renderling.xyz</subtitle>\n");

    for item in items {
        let item_updated = item
            .date
            .and_hms_opt(12, 0, 0)
            .unwrap()
            .and_utc()
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();

        xml.push_str("  <entry>\n");
        xml.push_str(&format!("    <title>{}</title>\n", xml_escape(&item.title)));
        xml.push_str(&format!(
            "    <link href=\"{}\" />\n",
            xml_escape(&item.link)
        ));
        xml.push_str(&format!("    <id>{}</id>\n", xml_escape(&item.link)));
        xml.push_str(&format!("    <updated>{}</updated>\n", item_updated));
        xml.push_str(&format!(
            "    <content type=\"html\"><![CDATA[{}]]></content>\n",
            cdata_escape(&item.content_html)
        ));
        xml.push_str("  </entry>\n");
    }

    xml.push_str("</feed>\n");
    xml
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_news_date() {
        assert_eq!(
            parse_news_date("Wed 11 Feb, 2026"),
            Some(NaiveDate::from_ymd_opt(2026, 2, 11).unwrap())
        );
        assert_eq!(
            parse_news_date("Sun 30 Nov, 2025"),
            Some(NaiveDate::from_ymd_opt(2025, 11, 30).unwrap())
        );
        assert_eq!(
            parse_news_date("Wed 24 September, 2025"),
            Some(NaiveDate::from_ymd_opt(2025, 9, 24).unwrap())
        );
        assert_eq!(
            parse_news_date("Sun 21 September, 2025"),
            Some(NaiveDate::from_ymd_opt(2025, 9, 21).unwrap())
        );
    }

    #[test]
    fn test_heading_to_anchor() {
        assert_eq!(heading_to_anchor("Wed 11 Feb, 2026"), "wed-11-feb-2026");
        assert_eq!(
            heading_to_anchor("Sun 21 September, 2025"),
            "sun-21-september-2025"
        );
    }

    #[test]
    fn test_strip_frontmatter() {
        let content = "---\ntitle: test\n---\n\nHello world";
        assert_eq!(strip_frontmatter(content), "Hello world");
    }

    #[test]
    fn test_parse_news_entries() {
        let content = r#"---
title: devlog
---
_The latest happenings_

## Wed 11 Feb, 2026

### Big News

Some content here.

## Sun 30 Nov, 2025

More content.
"#;
        let items = parse_news_entries(content, "https://renderling.xyz");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Renderling News - Wed 11 Feb, 2026");
        assert_eq!(
            items[0].link,
            "https://renderling.xyz/news/index.html#wed-11-feb-2026"
        );
        assert_eq!(items[0].date, NaiveDate::from_ymd_opt(2026, 2, 11).unwrap());
        assert!(items[0].content_html.contains("Big News"));
        assert_eq!(items[1].title, "Renderling News - Sun 30 Nov, 2025");
    }

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("a < b & c"), "a &lt; b &amp; c");
    }

    #[test]
    fn test_cdata_escape() {
        assert_eq!(cdata_escape("foo]]>bar"), "foo]]]]><![CDATA[>bar");
    }

    #[test]
    fn test_generate_rss() {
        let items = vec![FeedItem {
            title: "Test Item".to_string(),
            link: "https://example.com/test".to_string(),
            date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            content_html: "<p>Hello</p>".to_string(),
        }];
        let rss = generate_rss("https://example.com", &items);
        assert!(rss.contains("<title>Renderling</title>"));
        assert!(rss.contains("<title>Test Item</title>"));
        assert!(rss.contains("<![CDATA[<p>Hello</p>]]>"));
        assert!(rss.contains("rss version=\"2.0\""));
    }

    #[test]
    fn test_generate_atom() {
        let items = vec![FeedItem {
            title: "Test Item".to_string(),
            link: "https://example.com/test".to_string(),
            date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            content_html: "<p>Hello</p>".to_string(),
        }];
        let atom = generate_atom("https://example.com", &items);
        assert!(atom.contains("<title>Renderling</title>"));
        assert!(atom.contains("<title>Test Item</title>"));
        assert!(atom.contains("xmlns=\"http://www.w3.org/2005/Atom\""));
        assert!(atom.contains("Schell Scivally"));
    }
}
