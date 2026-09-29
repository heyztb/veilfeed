use std::collections::HashSet;

use quick_xml::{
    Writer,
    events::{BytesDecl, BytesEnd, BytesStart, Event},
};

use crate::{
    error::{AppError, AppResult},
    models::{OpmlEntry, OpmlPreview},
};

pub fn preview(xml: &str) -> AppResult<OpmlPreview> {
    let document =
        roxmltree::Document::parse(xml).map_err(|error| AppError::Xml(error.to_string()))?;
    let body = document
        .descendants()
        .find(|node| node.has_tag_name("body"))
        .ok_or_else(|| AppError::Xml("OPML body is missing".into()))?;
    let mut entries = Vec::new();
    let mut warnings = Vec::new();
    let mut seen = HashSet::new();
    walk(body, &mut vec![], &mut entries, &mut warnings, &mut seen);
    Ok(OpmlPreview { entries, warnings })
}

fn walk(
    node: roxmltree::Node<'_, '_>,
    path: &mut Vec<String>,
    entries: &mut Vec<OpmlEntry>,
    warnings: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    for outline in node
        .children()
        .filter(|child| child.has_tag_name("outline"))
    {
        let xml_url = outline
            .attribute("xmlUrl")
            .or_else(|| outline.attribute("xmlurl"));
        let title = outline
            .attribute("title")
            .or_else(|| outline.attribute("text"))
            .unwrap_or("Untitled")
            .trim();
        if let Some(feed_url) = xml_url {
            if url::Url::parse(feed_url).is_err() {
                warnings.push(format!("Skipped {title}: invalid feed URL"));
                continue;
            }
            let normalized = feed_url.trim().to_lowercase();
            if !seen.insert(normalized) {
                warnings.push(format!("Skipped duplicate feed: {title}"));
                continue;
            }
            entries.push(OpmlEntry {
                title: title.into(),
                feed_url: feed_url.trim().into(),
                site_url: outline.attribute("htmlUrl").map(str::to_owned),
                folder_path: path.clone(),
            });
        } else {
            let should_push = !title.is_empty() && title != "Untitled";
            if should_push {
                path.push(title.into());
            }
            walk(outline, path, entries, warnings, seen);
            if should_push {
                path.pop();
            }
        }
    }
}

pub fn export(entries: &[OpmlEntry]) -> AppResult<String> {
    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    writer.write_event(Event::Start(
        BytesStart::new("opml").with_attributes([("version", "2.0")]),
    ))?;
    writer.write_event(Event::Start(BytesStart::new("head")))?;
    writer.write_event(Event::Start(BytesStart::new("title")))?;
    writer.write_event(Event::Text(quick_xml::events::BytesText::new(
        "Veilfeed subscriptions",
    )))?;
    writer.write_event(Event::End(BytesEnd::new("title")))?;
    writer.write_event(Event::End(BytesEnd::new("head")))?;
    writer.write_event(Event::Start(BytesStart::new("body")))?;

    let mut current_path: Vec<&str> = Vec::new();
    for entry in entries {
        let next_path = entry
            .folder_path
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let common = current_path
            .iter()
            .zip(&next_path)
            .take_while(|(left, right)| left == right)
            .count();
        for _ in common..current_path.len() {
            writer.write_event(Event::End(BytesEnd::new("outline")))?;
        }
        for folder in &next_path[common..] {
            let mut element = BytesStart::new("outline");
            element.push_attribute(("text", *folder));
            element.push_attribute(("title", *folder));
            writer.write_event(Event::Start(element))?;
        }
        current_path = next_path;
        let mut outline = BytesStart::new("outline");
        outline.push_attribute(("type", "rss"));
        outline.push_attribute(("text", entry.title.as_str()));
        outline.push_attribute(("title", entry.title.as_str()));
        outline.push_attribute(("xmlUrl", entry.feed_url.as_str()));
        if let Some(site) = entry.site_url.as_deref() {
            outline.push_attribute(("htmlUrl", site));
        }
        writer.write_event(Event::Empty(outline))?;
    }
    for _ in 0..current_path.len() {
        writer.write_event(Event::End(BytesEnd::new("outline")))?;
    }
    writer.write_event(Event::End(BytesEnd::new("body")))?;
    writer.write_event(Event::End(BytesEnd::new("opml")))?;
    String::from_utf8(writer.into_inner()).map_err(|error| AppError::Xml(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_nested_and_deduplicates() {
        let source = r#"<?xml version="1.0"?><opml version="2.0"><body><outline text="News"><outline text="A" xmlUrl="https://a.test/feed"/><outline text="A again" xmlUrl="https://a.test/feed"/></outline></body></opml>"#;
        let value = preview(source).unwrap();
        assert_eq!(value.entries.len(), 1);
        assert_eq!(value.entries[0].folder_path, ["News"]);
        assert_eq!(value.warnings.len(), 1);
    }
    #[test]
    fn export_round_trips_nested_folders() {
        let entries = vec![OpmlEntry {
            title: "A".into(),
            feed_url: "https://a.test/feed".into(),
            site_url: None,
            folder_path: vec!["News".into(), "Technology".into()],
        }];
        let round_trip = preview(&export(&entries).unwrap()).unwrap();
        assert_eq!(round_trip.entries.len(), 1);
        assert_eq!(round_trip.entries[0].folder_path, ["News", "Technology"]);
    }
}
