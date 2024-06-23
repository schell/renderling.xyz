//! Interpolation of markdown templates.
use html_parser::Element;
use markdown::mdast::{
    self, Code, Definition, Delete, Emphasis, Heading, Html, Image, InlineCode, Link, List,
    ListItem, Paragraph, Strong, Table, TableCell, TableRow, Text, ThematicBreak,
};
use mogwai_dom::prelude::*;
use snafu::ResultExt;

pub fn make_html_view(node: html_parser::Node) -> Option<ViewBuilder> {
    match node {
        html_parser::Node::Text(s) => {
            if s.is_empty() {
                None
            } else {
                Some(ViewBuilder::text(s))
            }
        }
        html_parser::Node::Element(Element {
            id,
            name,
            variant: _,
            mut attributes,
            classes,
            children,
            source_span: _,
        }) => {
            let mut view = children
                .into_iter()
                .fold(ViewBuilder::element(name), |view, child| {
                    if let Some(child) = make_html_view(child) {
                        view.append(child)
                    } else {
                        view
                    }
                });
            if let Some(id) = id {
                attributes.insert("id".into(), Some(id));
            }
            let classes = classes.join(" ");
            attributes.insert("class".into(), Some(classes));
            for (k, v) in attributes.into_iter() {
                if let Some(v) = v {
                    view = view.with_single_attrib_stream(k, v);
                }
            }
            Some(view)
        }
        html_parser::Node::Comment(_) => None,
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ContentMeta {
    pub title: String,
}

pub fn make_md_view(node: mdast::Node) -> ViewBuilder {
    match node {
        mdast::Node::Root(root) => {
            let children: Vec<_> = root.children.into_iter().map(make_md_view).collect();
            rsx! { slot {
                {children}
            } }
        }
        mdast::Node::BlockQuote(bq) => {
            let children: Vec<_> = bq.children.into_iter().map(make_md_view).collect();
            rsx! {
                blockquote {
                    {children}
                }
            }
        }
        mdast::Node::FootnoteDefinition(_) => {
            // * https://docs.rs/markdown/1.0.0-alpha.14/markdown/mdast/struct.FootnoteDefinition.html
            // * https://github.blog/changelog/2021-09-30-footnotes-now-supported-in-markdown-fields/
            // * https://gist.github.com/schell/0e0e4aa5ea5c229d4843f6f1faa70264
            todo!("support footnotes")
        }
        mdast::Node::MdxJsxFlowElement(_) => todo!("support mdxjsxflowelement"),
        mdast::Node::List(List {
            children,
            position: _,
            ordered,
            start,
            spread: _,
        }) => {
            let children: Vec<_> = children.into_iter().map(make_md_view).collect();
            if ordered {
                let start = start.map(|n| n.to_string()).unwrap_or("1".to_string());
                rsx! { ol(start = start) {
                    {children}
                }}
            } else {
                rsx! { ul{
                    {children}
                }}
            }
        }
        mdast::Node::MdxjsEsm(_) => todo!("support for mdxjsEsm"),
        mdast::Node::Toml(_) => todo!("support for Toml"),
        mdast::Node::Yaml(_) => unreachable!(),
        mdast::Node::Break(_) => rsx! { br{} },
        mdast::Node::InlineCode(InlineCode { value, position: _ }) => rsx! {
            code(){ {value} }
        },
        mdast::Node::InlineMath(_) => todo!("support for inline math"),
        mdast::Node::Delete(Delete {
            children,
            position: _,
        }) => {
            let children: Vec<_> = children.into_iter().map(make_md_view).collect();
            rsx! {
                del{ {children} }
            }
        }
        mdast::Node::Emphasis(Emphasis {
            children,
            position: _,
        }) => {
            let children: Vec<_> = children.into_iter().map(make_md_view).collect();
            rsx! {
                em{ {children} }
            }
        }
        mdast::Node::MdxTextExpression(_) => todo!("support for MdxTextExpression"),
        mdast::Node::FootnoteReference(_) => todo!("support for footnotes"),
        mdast::Node::Html(Html { value, position: _ }) => {
            // TODO UNWRAP: todo, make this safe
            let dom = html_parser::Dom::parse(&value).unwrap();
            let children: Vec<_> = dom.children.into_iter().flat_map(make_html_view).collect();
            rsx! {
                slot{ {children} }
            }
        }
        mdast::Node::Image(Image {
            position: _,
            alt,
            url,
            title,
        }) => rsx! {
            img(alt = alt, src = url, title = title.unwrap_or_default()){}
        },
        mdast::Node::ImageReference(_) => todo!("support for ImageReference"),
        mdast::Node::MdxJsxTextElement(_) => todo!("support for MdxJsxTextElement"),
        mdast::Node::Link(Link {
            children,
            position: _,
            url,
            title,
        }) => {
            let children: Vec<_> = children.into_iter().map(make_md_view).collect();
            rsx! {
                a(href = url, title = title.unwrap_or_default()) {
                    {children}
                }
            }
        }
        mdast::Node::LinkReference(_) => todo!("support for LinkReference"),
        mdast::Node::Strong(Strong {
            children,
            position: _,
        }) => {
            let children: Vec<_> = children.into_iter().map(make_md_view).collect();
            rsx! {
                strong() { {children} }
            }
        }
        mdast::Node::Text(Text { value, position: _ }) => ViewBuilder::text(value),
        mdast::Node::Code(Code {
            value,
            position: _,
            lang,
            meta,
        }) => {
            log::info!("code_lang: {lang:#?}");
            log::info!("code_meta: {meta:#?}");
            log::info!("code_value:\n{value}");

            if let Some(lang) = lang.as_deref() {
                use syntect::{highlighting::ThemeSet, parsing::SyntaxSet};

                const DRACULA_BYTES: &[u8] = include_bytes!("../Dracula.tmTheme");

                let ss = SyntaxSet::load_defaults_newlines();
                let mut cursor = std::io::Cursor::new(DRACULA_BYTES);
                let theme = ThemeSet::load_from_reader(&mut cursor).unwrap();
                let syntax = ss
                    .syntaxes()
                    .iter()
                    .find(|s| s.name.to_lowercase() == lang)
                    .unwrap_or(&ss.syntaxes()[0]);
                //let c = theme.settings.background.unwrap_or(Color::WHITE);
                let html = syntect::html::highlighted_html_for_string(&value, &ss, syntax, &theme)
                    .unwrap();
                let dom = html_parser::Dom::parse(&html).unwrap();
                let children: Vec<_> = dom.children.into_iter().flat_map(make_html_view).collect();

                rsx! {
                    code(class="rust") {{children}}
                }
            } else {
                rsx! {
                    code(class=lang.unwrap_or_default()) {
                        pre(){{value}}
                    }
                }
            }
        }
        mdast::Node::Math(_) => todo!("support for Math"),
        mdast::Node::MdxFlowExpression(_) => todo!("support for MdxFlowExpression"),
        mdast::Node::Heading(Heading {
            children,
            position: _,
            depth,
        }) => {
            let children: Vec<_> = children.into_iter().map(make_md_view).collect();
            match depth {
                1 => rsx! { h1{{children}}},
                2 => rsx! { h2{{children}}},
                3 => rsx! { h3{{children}}},
                4 => rsx! { h4{{children}}},
                5 => rsx! { h5{{children}}},
                _ => rsx! { h6{{children}}},
            }
        }
        mdast::Node::Table(Table {
            children,
            position: _,
            align: _,
        }) => {
            let children: Vec<_> = children.into_iter().map(make_md_view).collect();
            rsx! {
                table() {
                    {children}
                }
            }
        }
        mdast::Node::ThematicBreak(ThematicBreak { position: _ }) => rsx! { br{} },
        mdast::Node::TableRow(TableRow {
            children,
            position: _,
        }) => {
            let children: Vec<_> = children.into_iter().map(make_md_view).collect();
            rsx! {
                tr() {{children}}
            }
        }
        mdast::Node::TableCell(TableCell {
            children,
            position: _,
        }) => {
            let children: Vec<_> = children.into_iter().map(make_md_view).collect();
            rsx! {
                td() {{children}}
            }
        }
        mdast::Node::ListItem(ListItem {
            children,
            position: _,
            spread: _,
            checked,
        }) => {
            let children: Vec<_> = children.into_iter().map(make_md_view).collect();
            rsx! {
                li(class=checked.map(|c| if c { "todo-checked" } else {"todo-unchecked"}).unwrap_or_default()) {
                    {children}
                }
            }
        }
        mdast::Node::Definition(Definition {
            position: _,
            url: _,
            title: _,
            identifier: _,
            label: _,
        }) => todo!("support for Definition"),
        mdast::Node::Paragraph(Paragraph {
            children,
            position: _,
        }) => {
            let children: Vec<_> = children.into_iter().map(make_md_view).collect();
            rsx! {
                p(){{children}}
            }
        }
    }
}

pub struct InterpolatedMarkdown {
    pub title: Option<String>,
    pub view: ViewBuilder,
}

impl InterpolatedMarkdown {
    pub fn new(view: ViewBuilder) -> Self {
        Self { title: None, view }
    }
}

pub fn get_frontmatter(node: &mut mdast::Node) -> Option<mdast::Yaml> {
    if let mdast::Node::Yaml(yaml) = node {
        Some(yaml.clone())
    } else if let Some(children) = node.children_mut() {
        let mut frontmatter = None;
        children.retain_mut(|child| {
            if let Some(yaml) = get_frontmatter(child) {
                frontmatter = Some(yaml.clone());
                return false;
            }
            true
        });
        frontmatter
    } else {
        None
    }
}

pub fn interpolate_markdown(
    content: impl AsRef<str>,
) -> Result<InterpolatedMarkdown, crate::Error> {
    let mut opts = markdown::ParseOptions::default();
    opts.constructs.frontmatter = true;
    let mut node = markdown::to_mdast(content.as_ref(), &opts)
        .map_err(|message| crate::Error::Md { message })?;
    let meta: Option<ContentMeta> = if let Some(frontmatter) = get_frontmatter(&mut node) {
        Some(serde_yaml::from_str(&frontmatter.value).context(crate::YamlSnafu)?)
    } else {
        log::debug!("no meta");
        None
    };
    let view = make_md_view(node.clone());
    let mut md = InterpolatedMarkdown::new(view);
    md.title = meta.map(|m| m.title);
    Ok(md)
}
