//! Interpolation of markdown templates.
use std::collections::{BTreeMap, HashMap};

use html_parser::Element;
use markdown::mdast::{
    self, Code, Definition, Delete, Emphasis, Heading, Html, Image, InlineCode, Link, List,
    ListItem, Paragraph, Strong, Table, TableCell, TableRow, Text, ThematicBreak,
};
use mogwai::prelude::*;
use snafu::ResultExt;

const SECTION_LINK: &str = "🔗";

pub enum Node<V: View> {
    Text(V::Text),
    Element(V::Element),
}

impl<V: View> ViewChild<V> for Node<V> {
    fn as_append_arg(&self) -> AppendArg<V, impl Iterator<Item = std::borrow::Cow<'_, V::Node>>> {
        match self {
            Node::Text(t) => t.as_boxed_append_arg(),
            Node::Element(e) => e.as_boxed_append_arg(),
        }
    }
}

impl<V: View> Node<V> {
    fn text(t: V::Text) -> Self {
        Node::Text(t)
    }

    fn el(e: V::Element) -> Self {
        Node::Element(e)
    }
}

pub fn make_html_view<V: View>(node: html_parser::Node) -> Option<Node<V>> {
    log::trace!("html node:");
    match node {
        html_parser::Node::Text(s) => Some(Node::Text(V::Text::new(s))),
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
            let view = V::Element::new(name);
            children.into_iter().for_each(|child| {
                if let Some(child) = make_html_view(child) {
                    view.append_child(&child);
                }
            });
            if let Some(id) = id {
                attributes.insert("id".into(), Some(id));
            }
            if !classes.is_empty() {
                let classes = classes.join(" ");
                attributes.insert("class".into(), Some(classes));
            }
            for (k, v) in BTreeMap::from_iter(attributes.into_iter()).into_iter() {
                log::trace!("  attribute: ({k}, {v:?})");
                let value = v.unwrap_or_default();
                view.set_property(k, value);
            }
            Some(Node::Element(view))
        }
        html_parser::Node::Comment(_) => None,
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ContentMeta {
    pub title: String,

    #[serde(default)]
    pub date: Option<String>,

    /// Whether the article should be excluded from RSS feeds
    #[serde(default)]
    pub exclude_from_rss: bool,
}

/// Converts a markdown node into a heading link.
pub fn as_link_text(node: &mdast::Node) -> String {
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
        mdast::Node::Link(Link { title, .. }) => {
            if let Some(t) = title.as_ref() {
                s = t.clone();
                &[]
            } else {
                &[]
            }
        }
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
        _ => &[],
    };
    let s2 = nodes.iter().map(as_link_text).collect::<Vec<_>>().concat();
    let mut last_char_is_hyphen = false;
    [s, s2]
        .concat()
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() {
                last_char_is_hyphen = false;
                Some(c.to_lowercase().to_string())
            } else if c.is_whitespace() && last_char_is_hyphen {
                // last_char_is_hyphen is aready `true`
                None
            } else if c.is_whitespace() {
                last_char_is_hyphen = true;
                Some("-".to_owned())
            } else if c == '-' && last_char_is_hyphen {
                None
            } else if c == '-' {
                last_char_is_hyphen = true;
                Some("-".to_owned())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .concat()
}

#[derive(Default)]
pub struct AstRenderer {
    link_defs: HashMap<String, Definition>,
    link_refs: HashMap<String, Vec<Proxy<Option<Definition>>>>,
}

impl AstRenderer {
    pub fn insert_proxy(&mut self, key: impl AsRef<str>, mut p: Proxy<Option<Definition>>) {
        if let Some(def) = self.link_defs.get(key.as_ref()) {
            p.set(Some(def.clone()));
        } else {
            // store it for later when we get the def
            let entry = self.link_refs.entry(key.as_ref().to_string()).or_default();
            entry.push(p);
        }
    }

    pub fn insert_def(&mut self, def: Definition) {
        if let Some(fns) = self.link_refs.remove(&def.identifier) {
            for mut p in fns.into_iter() {
                p.set(Some(def.clone()));
            }
        }
        self.link_defs.insert(def.identifier.clone(), def);
    }
}

impl AstRenderer {
    pub fn make_md_view<V: View>(&mut self, node: mdast::Node) -> Vec<Node<V>> {
        match node {
            mdast::Node::Root(root) => {
                let children: Vec<_> = root
                    .children
                    .into_iter()
                    .flat_map(|c| self.make_md_view(c))
                    .collect();
                children
            }
            mdast::Node::Blockquote(bq) => {
                let children: Vec<_> = bq
                    .children
                    .into_iter()
                    .map(|c| self.make_md_view(c))
                    .collect();
                rsx! {
                    let bq = blockquote {
                        {children}
                    }
                }
                vec![Node::el(bq)]
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
                    rsx! {
                        let ol = ol(start = start) {
                            {children}
                        }
                    }
                    vec![Node::el(ol)]
                } else {
                    rsx! {
                        let ul = ul{
                            {children}
                        }
                    }
                    vec![Node::el(ul)]
                }
            }
            mdast::Node::MdxjsEsm(_) => todo!("support for mdxjsEsm"),
            mdast::Node::Toml(_) => todo!("support for Toml"),
            mdast::Node::Yaml(_) => unreachable!(),
            mdast::Node::Break(_) => vec![Node::el(V::Element::new("br"))],
            mdast::Node::InlineCode(InlineCode { value, position: _ }) => {
                rsx! {
                    let code = code(){ {value} }
                }
                vec![Node::el(code)]
            }
            mdast::Node::InlineMath(_) => todo!("support for inline math"),
            mdast::Node::Delete(Delete {
                children,
                position: _,
            }) => {
                let children: Vec<_> = children.into_iter().map(|c| self.make_md_view(c)).collect();
                rsx! {
                    let del = del{ {children} }
                }
                vec![Node::el(del)]
            }
            mdast::Node::Emphasis(Emphasis {
                children,
                position: _,
            }) => {
                let children: Vec<_> = children.into_iter().map(|c| self.make_md_view(c)).collect();
                rsx! {
                    let em = em{ {children} }
                }
                vec![Node::el(em)]
            }
            mdast::Node::MdxTextExpression(_) => todo!("support for MdxTextExpression"),
            mdast::Node::FootnoteReference(_) => todo!("support for footnotes"),
            mdast::Node::Html(Html { value, position: _ }) => {
                let dom = html_parser::Dom::parse(&value).unwrap();
                let children: Vec<_> = dom.children.into_iter().flat_map(make_html_view).collect();
                children
            }
            mdast::Node::Image(Image {
                position: _,
                alt,
                url,
                title,
            }) => {
                rsx! {
                    let i = img(alt = alt, src = url, title = title.unwrap_or_default()){}
                }
                vec![Node::el(i)]
            }
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
                    let a = a(href = url, title = title.unwrap_or_default()) {
                        {children}
                    }
                }
                vec![Node::el(a)]
            }
            mdast::Node::LinkReference(link_ref) => {
                log::trace!("{link_ref:#?}");
                let children: Vec<_> = link_ref
                    .children
                    .into_iter()
                    .map(|c| self.make_md_view(c))
                    .collect();
                // We'll get the definition later in the parsing process
                let mut proxy = Proxy::<Option<Definition>>::default();
                rsx! {
                    let a = a(
                        href = proxy(maybe_def => {
                            log::info!("got def {maybe_def:#?}");
                            maybe_def.as_ref().map(|def| def.url.clone()).unwrap_or_default()
                        }),
                        title = proxy(maybe_def => maybe_def.as_ref().and_then(|def| def.title.clone()).unwrap_or_default())
                    ) {
                        {children}
                    }
                }
                self.insert_proxy(link_ref.identifier, proxy);
                vec![Node::el(a)]
            }
            mdast::Node::Strong(Strong {
                children,
                position: _,
            }) => {
                let children: Vec<_> = children.into_iter().map(|c| self.make_md_view(c)).collect();
                rsx! {
                    let s = strong() { {children} }
                }
                vec![Node::el(s)]
            }
            mdast::Node::Text(Text { value, position: _ }) => vec![Node::text(V::Text::new(value))],
            mdast::Node::Code(Code {
                value,
                position,
                lang,
                meta,
            }) => {
                log::trace!("code_lang: {lang:#?}");
                log::trace!("code_meta: {meta:#?}");
                log::trace!("code_value:\n{value}");
                log::trace!("position:\n{position:#?}");

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
                    let mut h = syntect::easy::HighlightLines::new(syntax, &theme);
                    rsx! {
                        let wrapper = div(class = "code-wrapper") {
                            let code = pre(class = "code-snippet"){}
                        }
                    }
                    for line in syntect::util::LinesWithEndings::from(&value) {
                        let ranges: Vec<(syntect::highlighting::Style, &str)> =
                            h.highlight_line(line, &ss).unwrap();
                        for (style, text) in ranges.iter() {
                            let color = format!(
                                "#{:02x}{:02x}{:02x}",
                                style.foreground.r, style.foreground.g, style.foreground.b
                            );
                            let bg = format!(
                                "#{:02x}{:02x}{:02x}",
                                style.background.r, style.background.g, style.background.b
                            );
                            let span: V::Element = V::Element::new("span");
                            span.set_style("color", color);
                            span.set_style("background", bg);
                            let text = V::Text::new(text);
                            span.append_child(text);
                            code.append_child(&span);
                        }
                    }
                    vec![Node::Element(wrapper)]
                } else {
                    rsx! {
                        let code = code(class=lang.unwrap_or_default()) {
                            pre(){{value.into_text::<V>()}}
                        }
                    }
                    vec![Node::el(code)]
                }
            }
            mdast::Node::Math(_) => todo!("support for Math"),
            mdast::Node::MdxFlowExpression(_) => todo!("support for MdxFlowExpression"),
            mdast::Node::Heading(Heading {
                children,
                position: _,
                depth,
            }) => {
                let id = children
                    .iter()
                    .map(as_link_text)
                    .collect::<Vec<_>>()
                    .concat();
                let href = format!("#{id}");
                let children: Vec<_> = children.into_iter().map(|c| self.make_md_view(c)).collect();
                let section_link = V::Text::new(SECTION_LINK);
                match depth {
                    1 => {
                        rsx! {
                            let h = h1(id = id){
                                {children}
                                a(class = "heading-link", href = href) { {section_link} }
                            }
                        }
                        vec![Node::el(h)]
                    }
                    2 => {
                        rsx! {
                            let h = h2(id = id){
                                {children}
                                a(class = "heading-link", href = href) { {section_link} }
                            }
                        }
                        vec![Node::el(h)]
                    }
                    3 => {
                        rsx! {
                            let h = h3(id = id){
                                {children}
                                a(class = "heading-link", href = href) { {section_link} }
                            }
                        }
                        vec![Node::el(h)]
                    }
                    4 => {
                        rsx! { let h = h4{{children}}}
                        vec![Node::el(h)]
                    }
                    5 => {
                        rsx! { let h = h5{{children}}}
                        vec![Node::el(h)]
                    }
                    _ => {
                        rsx! { let h = h6{{children}} }
                        vec![Node::el(h)]
                    }
                }
            }
            mdast::Node::Table(Table {
                mut children,
                position: _,
                align: _,
            }) => {
                let ast_head = children.remove(0);
                let table_head = match ast_head {
                    mdast::Node::TableRow(TableRow { children, .. }) => {
                        let children = children.into_iter().map(|node| match node {
                            mdast::Node::TableCell(TableCell { children, .. }) => {
                                rsx! {
                                    let t = th() {{children.into_iter().map(|c| self.make_md_view(c)).collect::<Vec<_>>()}} 
                                }
                                vec![Node::el(t)]
                            },
                            n => self.make_md_view(n),
                        }).collect::<Vec<_>>();
                        rsx! {
                            let t = thead() {
                                {{children}}
                            }
                        }
                        vec![Node::el(t)]
                    }
                    node => self.make_md_view(node),
                };
                let children: Vec<_> = children.into_iter().map(|c| self.make_md_view(c)).collect();
                rsx! {
                    let t = table() {
                        {table_head}
                        tbody() {
                            {children}
                        }
                    }
                }
                vec![Node::el(t)]
            }
            mdast::Node::ThematicBreak(ThematicBreak { position: _ }) => {
                vec![Node::el(V::Element::new("br"))]
            }
            mdast::Node::TableRow(TableRow {
                children,
                position: _,
            }) => {
                let children: Vec<_> = children.into_iter().map(|c| self.make_md_view(c)).collect();
                rsx! {
                    let t = tr() {{children}}
                }
                vec![Node::el(t)]
            }
            mdast::Node::TableCell(TableCell {
                children,
                position: _,
            }) => {
                let children: Vec<_> = children.into_iter().map(|c| self.make_md_view(c)).collect();
                rsx! {
                    let t = td() {{children}}
                }
                vec![Node::el(t)]
            }
            mdast::Node::ListItem(ListItem {
                children,
                position: _,
                spread: _,
                checked,
            }) => {
                let children: Vec<_> = children.into_iter().map(|c| self.make_md_view(c)).collect();
                rsx! {
                    let l = li(class=checked.map(|c| if c { "todo-checked" } else {"todo-unchecked"}).unwrap_or_default()) {
                        {children}
                    }
                }
                vec![Node::el(l)]
            }
            mdast::Node::Definition(def) => {
                self.insert_def(def.clone());
                vec![]
            }
            mdast::Node::Paragraph(Paragraph {
                children,
                position: _,
            }) => {
                let children: Vec<_> = children.into_iter().map(|c| self.make_md_view(c)).collect();
                rsx! {
                    let p = p(){{children}}
                }
                vec![Node::el(p)]
            }
        }
    }
}

pub struct InterpolatedMarkdown<V: View> {
    pub title: Option<String>,
    pub view: Vec<Node<V>>,
}

impl<V: View> InterpolatedMarkdown<V> {
    pub fn new(view: Vec<Node<V>>) -> Self {
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

pub fn interpolate_markdown<V: View>(
    content: impl AsRef<str>,
) -> Result<InterpolatedMarkdown<V>, crate::Error> {
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
