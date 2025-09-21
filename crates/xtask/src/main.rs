use clap::Parser;
use pusha::{Environment, SiteConfig};

const fn renderling_docs_url(e: Environment) -> &'static str {
    match e {
        Environment::Local => "http://127.0.0.1:4000/docs",
        Environment::Staging => "https://staging.renderling.xyz/docs",
        Environment::Production => "https://renderling.xyz/docs",
    }
}

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

/// A cli args struct that is a superset of pusha::Cli.
#[derive(Parser)]
struct Cli {
    /// The branch to checkout the renderling repo from.
    ///
    /// This is used to generate the manual and the documentation.
    #[clap(long, default_value = "main")]
    renderling_branch: String,

    /// Whether or not to refresh the renderling checkout
    #[clap(long)]
    renderling_refresh: bool,

    #[command(flatten)]
    pusha_args: pusha::PushaCli,
}

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
    env_logger::builder().init();

    let cli = Cli::parse();

    let renderling_checkout_dir =
        std::path::PathBuf::from(std::env!("CARGO_WORKSPACE_DIR")).join("renderling");
    let renderling_docs_dir =
        std::path::PathBuf::from(std::env!("CARGO_WORKSPACE_DIR")).join("content/docs");
    let renderling_manual_dir =
        std::path::PathBuf::from(std::env!("CARGO_WORKSPACE_DIR")).join("content/manual");

    let docs_and_manual_rebuild = if renderling_checkout_dir.is_dir() {
        if cli.renderling_refresh {
            // refresh
            log::info!(
                "refreshing the renderling checkout at branch '{}'",
                cli.renderling_branch
            );
            let child = tokio::process::Command::new("git")
                .args(["checkout", &cli.renderling_branch])
                .current_dir(&renderling_checkout_dir)
                .spawn()
                .unwrap();
            match child.wait_with_output().await {
                Ok(_) => {
                    log::info!("...checkout successful");
                }
                Err(e) => {
                    log::error!("could not checkout branch: {e}");
                    panic!("{e}");
                }
            }

            let child = tokio::process::Command::new("git")
                .args(["pull", "origin", &cli.renderling_branch])
                .current_dir(&renderling_checkout_dir)
                .spawn()
                .unwrap();
            match child.wait_with_output().await {
                Ok(_) => {
                    log::info!("...pull successful");
                }
                Err(e) => {
                    log::error!("could not pull branch: {e}");
                    panic!("{e}");
                }
            }
        } else {
            log::warn!("not refreshing the renderling checkout");
        }

        cli.renderling_refresh || !renderling_docs_dir.exists() || !renderling_manual_dir.exists()
    } else {
        // clone
        log::info!(
            "cloning the renderling repo at branch '{}'",
            cli.renderling_branch
        );
        let child = tokio::process::Command::new("git")
            .args([
                "clone",
                "https://github.com/schell/renderling.git",
                "--branch",
            ])
            .arg(cli.renderling_branch)
            .spawn()
            .unwrap();
        let result = child.wait_with_output().await;
        match result {
            Ok(_) => {
                log::info!("...clone successful");
            }
            Err(e) => {
                log::error!("...could not clone the renderling repo: {e}");
                panic!("{e}");
            }
        }
        true
    };

    if docs_and_manual_rebuild {
        log::info!("rebuilding docs and manual");
        tokio::fs::create_dir_all(&renderling_docs_dir)
            .await
            .unwrap();
        tokio::fs::create_dir_all(&renderling_manual_dir)
            .await
            .unwrap();
        let renderling_cargo_workspace = renderling_checkout_dir.canonicalize().unwrap();
        let child = tokio::process::Command::new("cargo")
            .args([
                "xtask",
                "manual",
                "--no-test",
                "--docs-url",
                renderling_docs_url(cli.pusha_args.environment),
            ])
            .current_dir(&renderling_cargo_workspace)
            .env("CARGO_WORKSPACE_DIR", &renderling_cargo_workspace)
            .spawn()
            .unwrap();
        match child.wait_with_output().await {
            Ok(output) => {
                if output.status.success() {
                    log::info!("...built docs and manual");
                } else {
                    log::error!("...doc and manual building was unsuccessful");
                    panic!("could not build docs and manual");
                }
            }
            Err(e) => {
                log::error!("could not build docs and manual: {e}");
                panic!("{e}");
            }
        }

        log::info!("moving docs into content directory");
        let built_renderling_docs = renderling_cargo_workspace
            .join("target/doc")
            .canonicalize()
            .unwrap();
        tokio::fs::remove_dir_all(&renderling_docs_dir)
            .await
            .unwrap_or_else(|e| {
                log::error!("could not remove existing docs dir: {e}");
                panic!("{e}");
            });
        tokio::fs::rename(&built_renderling_docs, &renderling_docs_dir)
            .await
            .unwrap_or_else(|e| {
                log::error!("could not move built docs dir into the content directory: {e}");
                panic!("{e}");
            });

        log::info!("moving manual into content directory");
        let built_manual = renderling_cargo_workspace
            .join("manual/book")
            .canonicalize()
            .unwrap();
        tokio::fs::remove_dir_all(&renderling_manual_dir)
            .await
            .unwrap_or_else(|e| {
                log::error!("could not remove existing manual dir: {e}");
                panic!("{e}");
            });
        tokio::fs::rename(&built_manual, &renderling_manual_dir)
            .await
            .unwrap_or_else(|e| {
                log::error!("could not move built manual dir into the content directory: {e}");
                panic!("{e}");
            });
    }

    cli.pusha_args.run::<Rxyz>(&CONFIG, []).await;
}
