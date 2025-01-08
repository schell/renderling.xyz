//! Interpolation of markdown templates.
use std::collections::HashMap;

use futures_lite::StreamExt;
use html_parser::Element;
use markdown::mdast::{
    self, Code, Definition, Delete, Emphasis, Heading, Html, Image, InlineCode, Link, List, ListItem, Paragraph, Strong, Table, TableCell, TableRow, Text, ThematicBreak
};
use mogwai_dom::prelude::*;
use snafu::ResultExt;

const SECTION_LINK: &str = "🔗";

pub fn make_html_view(node: html_parser::Node) -> Option<ViewBuilder> {
    log::trace!("html node:");
    match node {
        html_parser::Node::Text(s) => {
            log::trace!("  text: '{s}'");
            Some(ViewBuilder::text(s))
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
            log::trace!("  element: {name}");
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
            if !classes.is_empty() {
                let classes = classes.join(" ");
                attributes.insert("class".into(), Some(classes));
            }
            for (k, v) in attributes.into_iter() {
                log::trace!("  attribute: ({k}, {v:?})");
                if let Some(v) = v {
                    view = view.with_single_attrib_stream(k, v);
                } else {
                    view = view.with_single_bool_attrib_stream(k, true);
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


fn to_text(node: &mdast::Node) -> String {
    let mut s = String::new();
        let nodes: &[mdast::Node] = match node {
            mdast::Node::Root(_) => todo!(),
            mdast::Node::Blockquote(bq) => &bq.children,
            mdast::Node::FootnoteDefinition(fnd) => &fnd.children,
            mdast::Node::List(list) => &list.children,
            mdast::Node::Emphasis(e) => &e.children,
            mdast::Node::MdxTextExpression(mdte) => {
                s = mdte.value.clone();
                &[]
            }
            mdast::Node::Link(Link { title, .. }) => if let Some(t) = title.as_ref() {
                s = t.clone();
                &[]
            } else {
                &[]
            },
            mdast::Node::Strong(s) => &s.children,
            mdast::Node::Text(t) => {
                s = t.value.clone();
                &[]
            }
            mdast::Node::Code(c) => {
                s = c.value.clone();
                &[]
            }
            mdast::Node::Heading(h) => &h.children,
            mdast::Node::Table(t) => &t.children,
            _ => &[]
        };
        let s2 = nodes.iter().map(to_text).collect::<Vec<_>>().concat();
        [s, s2].concat().chars().filter_map(|c| if c.is_alphanumeric() {
            Some(c.to_lowercase().to_string())
        } else if c == ' ' {
            Some("_".to_owned())
        } else {
            None
        }).collect::<Vec<_>>().concat()
    }

#[derive(Default)]
pub struct AstRenderer {
    link_refs: HashMap<String, FanInput<Definition>>,
}



impl AstRenderer {
        pub fn make_md_view(&mut self, node: mdast::Node) -> ViewBuilder {
        match node {
            mdast::Node::Root(root) => {
                let children: Vec<_> = root.children.into_iter().map(|c| self.make_md_view(c)).collect();
                rsx! { slot {
                    {children}
                } }
            }
            mdast::Node::Blockquote(bq) => {
                let children: Vec<_> = bq.children.into_iter().map(|c| self.make_md_view(c)).collect();
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
                let children: Vec<_> = children.into_iter().map(|c| self.make_md_view(c)).collect();
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
                let children: Vec<_> = children.into_iter().map(|c| self.make_md_view(c)).collect();
                rsx! {
                    del{ {children} }
                }
            }
            mdast::Node::Emphasis(Emphasis {
                children,
                position: _,
            }) => {
                let children: Vec<_> = children.into_iter().map(|c| self.make_md_view(c)).collect();
                rsx! {
                    em{ {children} }
                }
            }
            mdast::Node::MdxTextExpression(_) => todo!("support for MdxTextExpression"),
            mdast::Node::FootnoteReference(_) => todo!("support for footnotes"),
            mdast::Node::Html(Html { value, position: _ }) => {
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
                let children: Vec<_> = children.into_iter().map(|c| self.make_md_view(c)).collect();
                rsx! {
                    a(href = url, title = title.unwrap_or_default()) {
                        {children}
                    }
                }
            }
            mdast::Node::LinkReference(link_ref) => {
                log::trace!("{link_ref:#?}");
                let children: Vec<_> = link_ref.children.into_iter().map(|c| self.make_md_view(c)).collect();
                // We'll get the definition later in the parsing process
                let input = FanInput::<Definition>::default();
                self.link_refs.insert(link_ref.identifier, input.clone());
                rsx! {
                    a(
                        href = input.stream().map(|def| {
                            log::trace!("got def: {}", def.url);
                            def.url
                        }),
                        title = input.stream().map(|def| def.title.unwrap_or_default())
                    ) { {children} }
                }
            }
            mdast::Node::Strong(Strong {
                children,
                position: _,
            }) => {
                let children: Vec<_> = children.into_iter().map(|c| self.make_md_view(c)).collect();
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
                log::trace!("code_lang: {lang:#?}");
                log::trace!("code_meta: {meta:#?}");
                log::trace!("code_value:\n{value}");
                rsx! {
                    pre(class = "code-snippet") {{value}}
                }
                // if let Some(lang) = lang.as_deref() {
                //     use syntect::{highlighting::ThemeSet, parsing::SyntaxSet};

                //     const DRACULA_BYTES: &[u8] = include_bytes!("../Dracula.tmTheme");

                //     let ss = SyntaxSet::load_defaults_newlines();
                //     let mut cursor = std::io::Cursor::new(DRACULA_BYTES);
                //     let theme = ThemeSet::load_from_reader(&mut cursor).unwrap();
                //     let syntax = ss
                //         .syntaxes()
                //         .iter()
                //         .find(|s| s.name.to_lowercase() == lang)
                //         .unwrap_or(&ss.syntaxes()[0]);
                //     //let c = theme.settings.background.unwrap_or(Color::WHITE);
                //     let html = syntect::html::highlighted_html_for_string(&value, &ss, syntax, &theme)
                //         .unwrap();
                //     log::info!("html_value: {html}");
                //     let dom = html_parser::Dom::parse(&html).unwrap();
                //     let children: Vec<_> = dom.children.into_iter().flat_map(make_html_view).collect();
                //     let mut code = ViewBuilder::element("code");
                //     for child in children.into_iter() {
                //         code = code.append(child);
                //     }
                //     code
                // } else {
                //     rsx! {
                //         code(class=lang.unwrap_or_default()) {
                //             pre(){{ value}}
                //         }
                //     }
                // }
            }
            mdast::Node::Math(_) => todo!("support for Math"),
            mdast::Node::MdxFlowExpression(_) => todo!("support for MdxFlowExpression"),
            mdast::Node::Heading(Heading {
                children,
                position: _,
                depth,
            }) => {
                let id = children.iter().map(to_text).collect::<Vec<_>>().concat();
                let href = format!("#{}", id);
                let children: Vec<_> = children.into_iter().map(|c| self.make_md_view(c)).collect();
                match depth {
                    1 => rsx! { 
                        h1(id = id){
                            {children}
                            a(class = "heading-link", href = href) { {SECTION_LINK} }
                        }
                    },
                    2 => rsx! { 
                        h2(id = id){
                            {children}
                            a(class = "heading-link", href = href) { {SECTION_LINK} }
                        }
                    },
                    3 => rsx! { 
                        h3(id = id){
                            {children}
                            a(class = "heading-link", href = href) { {SECTION_LINK} }
                        }
                    },
                    4 => rsx! { h4{{children}}},
                    5 => rsx! { h5{{children}}},
                    _ => rsx! { h6{{children}}},
                }
            }
            mdast::Node::Table(Table {
                mut children,
                position: _,
                align: _,
            }) => {
                let ast_head = children.remove(0);
                let table_head = match ast_head {
                    mdast::Node::TableRow(TableRow{ children, ..}) => {
                        let children = children.into_iter().map(|node| match node {
                            mdast::Node::TableCell(TableCell { children, .. }) => rsx! {
                                th() {{children.into_iter().map(|c| self.make_md_view(c)).collect::<Vec<_>>()}} 
                            },
                            n => self.make_md_view(n),
                        }).collect::<Vec<_>>();  
                        rsx! {
                            thead() {
                                {{children}}
                            }
                        }
                    }
                    node => self.make_md_view(node),
                };
                let children: Vec<_> = children.into_iter().map(|c| self.make_md_view(c)).collect();
                rsx! {
                    table() {
                        {table_head}
                        tbody() {
                            {children}
                        }
                    }
                }
            }
            mdast::Node::ThematicBreak(ThematicBreak { position: _ }) => rsx! { br{} },
            mdast::Node::TableRow(TableRow {
                children,
                position: _,
            }) => {
                let children: Vec<_> = children.into_iter().map(|c| self.make_md_view(c)).collect();
                rsx! {
                    tr() {{children}}
                }
            }
            mdast::Node::TableCell(TableCell { children, position: _ }) => {
                let children: Vec<_> = children.into_iter().map(|c| self.make_md_view(c)).collect();
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
                let children: Vec<_> = children.into_iter().map(|c| self.make_md_view(c)).collect();
                rsx! {
                    li(class=checked.map(|c| if c { "todo-checked" } else {"todo-unchecked"}).unwrap_or_default()) {
                        {children}
                    }
                }
            }
            mdast::Node::Definition(def) => {
                if let Some(input) = self.link_refs.get(&def.identifier) {
                    log::trace!("sending {} {}", def.identifier, def.url);
                    input.try_send(def).unwrap();
                }
                rsx! { slot(){} }
            }
            mdast::Node::Paragraph(Paragraph {
                children,
                position: _,
            }) => {
                let children: Vec<_> = children.into_iter().map(|c| self.make_md_view(c)).collect();
                rsx! {
                    p(){{children}}
                }
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
    let mut opts = markdown::ParseOptions::gfm();
    opts.constructs.frontmatter = true;
    let mut node = markdown::to_mdast(content.as_ref(), &opts)
        .map_err(|message| crate::Error::Md { message })?;
    let meta: Option<ContentMeta> = if let Some(frontmatter) = get_frontmatter(&mut node) {
        Some(serde_yaml::from_str(&frontmatter.value).context(crate::YamlSnafu)?)
    } else {
        log::trace!("no meta");
        None
    };
    let mut renderer = AstRenderer::default();
    let view = renderer.make_md_view(node.clone());
    let mut md = InterpolatedMarkdown::new(view);
    md.title = meta.map(|m| m.title);
    Ok(md)
}
