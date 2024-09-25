use pusha::{Environment, ExternalPage, SiteConfig};

const CONFIG: SiteConfig = {
    const fn root_url(e: Environment) -> &'static str {
        match e {
            Environment::Local => "http://127.0.0.1:4000",
            Environment::Staging => "https://staging.renderling.xyz",
            Environment::Production => "https://renderling.xyz",
        }
    }

    const fn cloudfront_distro(e: Environment) -> Option<&'static str> {
        match e {
            Environment::Local => None,
            Environment::Staging => Some("E16FY1FTWBR11T"),
            Environment::Production => Some("E27AC3A8NB65G6"),
        }
    }

    const fn s3_bucket(e: Environment) -> Option<&'static str> {
        match e {
            Environment::Local => None,
            Environment::Staging => Some("staging.renderling.xyz"),
            Environment::Production => Some("renderling.xyz"),
        }
    }

    SiteConfig {
        root_url,
        cloudfront_distro,
        s3_bucket,
    }
};

struct Rxyz;
impl pusha::Renderer for Rxyz {
    type Error = rxyz::Error;

    fn render_content(
        cfg: &SiteConfig,
        environment: Environment,
        content: String,
        extra_classes: &str,
    ) -> Result<String, Self::Error> {
        let site = rxyz::Site::new((cfg.root_url)(environment)).unwrap();
        site.render_markdown_page(content, extra_classes)
    }
}

#[tokio::main]
async fn main() {
    pusha::run::<Rxyz>(&CONFIG, []).await;
}
