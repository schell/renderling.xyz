//! Rxyz is the template portion of renderling.xyz.
use mogwai_dom::{core::view::ViewBuilder, rsx, view::SsrDom};
use snafu::prelude::*;

mod md;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Interpolation md error: {message}"))]
    Md { message: markdown::message::Message },

    #[snafu(display("Interpolation html error: {source} "))]
    Html { source: html_parser::Error },

    #[snafu(display("Yaml: {source}"))]
    Yaml { source: serde_yaml::Error },

    #[snafu(display("{source}"))]
    InvalidUri { source: http::uri::InvalidUri },

    #[snafu(display("{source}"))]
    Http { source: http::Error },

    #[snafu(display("Rendering error: {source}"))]
    Render {
        source: mogwai_dom::core::view::Error,
    },
}

fn uri_builder_from(uri: &http::uri::Uri) -> http::uri::Builder {
    let b = http::uri::Builder::new();
    let b = if let Some(scheme) = uri.scheme() {
        b.scheme(scheme.as_str())
    } else {
        b
    };
    let b = if let Some(auth) = uri.authority() {
        b.authority(auth.as_str())
    } else {
        b
    };
    let b = if let Some(pq) = uri.path_and_query() {
        b.path_and_query(pq.as_str())
    } else {
        b
    };
    b
}

fn prefix_doctype(html: String) -> String {
    format!("<!doctype html>{html}")
}

pub struct Site {
    url_root: http::Uri,
}

impl Site {
    /// Create a new site object that will do the rendering.
    pub fn new(url_root: impl AsRef<str>) -> Result<Self, Error> {
        Ok(Self {
            url_root: http::Uri::try_from(url_root.as_ref()).context(InvalidUriSnafu)?,
        })
    }

    pub fn css(&self) -> String {
        const CSS: &str = include_str!("../style.css");
        // root will never have the trailing slash!
        let root = format!("{}", self.url_root);
        let root = root.trim_end_matches('/');
        CSS.replace("#URL_ROOT#", root)
    }

    /// Convert a link relative to the site root to an absolute link.
    fn site_path(&self, path: impl AsRef<str>) -> Result<String, Error> {
        let uri = uri_builder_from(&self.url_root);
        let sep = if self.url_root.path().ends_with('/') {
            ""
        } else {
            "/"
        };
        let path = format!(
            "{}{sep}{}",
            self.url_root.path(),
            path.as_ref().trim_matches('/')
        );
        let uri = uri.path_and_query(path).build().context(HttpSnafu)?;
        Ok(format!("{}", uri))
    }

    fn nav(&self) -> Result<ViewBuilder, Error> {
        Ok(rsx! {
            nav {
                ul(class="nav-links") {
                    li() {
                        a(href = self.site_path("devlog")?){{"devlog.html"}}
                    }
                    li() {
                        a(href = "https://github.com/schell/renderling") {{"github"}}
                    }
                }
                h1() {
                    a(href = self.site_path("/")?, class="logo-link") {
                        {"🍖 renderling"}
                    }
                }
            }
        })
    }

    fn render_page(
        &self,
        title: impl AsRef<str>,
        content: ViewBuilder,
        main_classes: &str,
    ) -> Result<String, Error> {
        let page = rsx! {
            html(lang = "en") {
                head {
                    meta(charset = "UTF-8"){}
                    link(rel="icon", href = self.site_path("favicon.ico")?){}
                    title {{title.as_ref()}}
                    style {{self.css()}}
                }
                body {
                    {self.nav()?}
                    main(class=main_classes) {{content}}
                    footer() {
                        p(){{":)"}}
                    }
                }
            }
        };
        let dom = SsrDom::try_from(page).context(RenderSnafu)?;
        let s = futures_lite::future::block_on(dom.html_string());
        Ok(prefix_doctype(s))
    }

    /// Interpolate the markdown bytes as its own page.
    pub fn render_markdown_page(
        &self,
        content: String,
        extra_classes: &str,
    ) -> Result<String, Error> {
        let imd = md::interpolate_markdown(content)?;
        self.render_page(
            imd.title.unwrap_or("Untitled".to_string()),
            imd.view,
            extra_classes,
        )
    }
}
