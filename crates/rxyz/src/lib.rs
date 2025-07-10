//! Rxyz is the template portion of renderling.xyz.
use md::Node;
use mogwai::{prelude::*, ssr::Ssr};
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

    #[snafu(display("Rendering error"))]
    Render,
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
        let site_path = format!("{}", uri);
        log::debug!("site_path: {site_path}");
        Ok(site_path)
    }

    fn nav<V: View>(&self) -> Result<V::Element, Error> {
        rsx! {
            let nav = nav {
                h1() {
                    a(href = self.site_path("/")?, class="logo-link") {
                        img(src = self.site_path("/img/logo.png")?) {
                            "renderling"
                        }
                    }
                }
                div(class="nav-middle") {}
                div(class="nav-right") {
                    div(class="nav-top") {}
                    ul(class="nav-links") {
                        li() {
                            a(href = self.site_path("articles/index.html")?){
                                "articles"
                            }
                        }
                        li() {
                            a(href = self.site_path("devlog/index.html")?){
                                "devlog"
                            }
                        }
                        li() {
                            a(href = "https://github.com/schell/renderling") {
                                "github"
                            }
                        }
                        li() {
                            iframe(
                                src="https://github.com/sponsors/schell/button",
                                title="Sponsor schell",
                                height="32",
                                width="114",
                                style="border: 0; border-radius: 6px;"
                            ){}
                        }
                    }
                }
            }
        }

        Ok(nav)
    }

    fn page<V: View>(
        &self,
        title: impl AsRef<str>,
        content: Vec<Node<V>>,
        main_classes: &str,
    ) -> Result<V::Element, Error> {
        rsx! {
            let page = html(lang = "en") {
                head() {
                    meta(charset = "UTF-8"){}
                    link(rel="icon", href = self.site_path("favicon.ico")?){}
                    title() {
                        {title.as_ref().into_text::<V>()}
                    }
                    style() {
                        {self.css().into_text::<V>()}
                    }
                }
                body() {
                    {self.nav::<V>()?}
                    main(class=main_classes) {
                        {content}
                    }
                    footer() {
                        p(){
                            "This project is authored and maintained by Schell Scivally."
                            " "
                            "Please consider supporting this project by sponsoring it on GitHub 🙏"
                        }
                        iframe(
                            src="https://github.com/sponsors/schell/card",
                            title="Sponsor schell, author of Renderling",
                            height="225",
                            width="600",
                            style="border: 0;"
                        ){}
                    }
                }
            }
        }
        Ok(page)
    }

    fn render_page(&self, page: <mogwai::ssr::Ssr as View>::Element) -> Result<String, Error> {
        let s = page.html_string();
        Ok(prefix_doctype(s))
    }

    /// Interpolate the markdown bytes as its own page.
    pub fn render_markdown_page(
        &self,
        content: String,
        extra_classes: &str,
    ) -> Result<String, Error> {
        let imd = md::interpolate_markdown(content)?;
        let title = imd.title.unwrap_or("Untitled".to_string());
        let page = self.page::<Ssr>(title, imd.view, extra_classes)?;
        self.render_page(page)
    }
}
