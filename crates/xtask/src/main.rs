use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    io::Read,
    str::FromStr,
};

use clap::{Parser, ValueEnum};

#[derive(clap::Subcommand)]
enum Command {
    /// Deploy the site from the `site` directory.
    Deploy,
    /// Build the site locally, compiling templates and content into the `site` directory.
    Build,
    /// Clean the local site directory.
    Clean,
}

#[derive(Clone, Copy, Default, Debug, ValueEnum, serde::Serialize, serde::Deserialize)]
enum Environment {
    #[default]
    Local,
    Staging,
    Production,
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Environment::Local => "local",
            Environment::Staging => "staging",
            Environment::Production => "production",
        })
    }
}

impl FromStr for Environment {
    type Err = snafu::Whatever;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "local" => Ok(Self::Local),
            "staging" => Ok(Self::Staging),
            "production" => Ok(Self::Production),
            s => snafu::whatever!("unsupported environment '{s}'"),
        }
    }
}

impl Environment {
    const fn bucket(&self) -> Option<&'static str> {
        match self {
            Environment::Local => None,
            Environment::Staging => Some("staging.renderling.xyz"),
            Environment::Production => Some("renderling.xyz"),
        }
    }

    const fn cloudfront_distro(&self) -> Option<&'static str> {
        match self {
            Environment::Local => None,
            Environment::Staging => Some("E2TJQOQUYFZJ0U"),
            Environment::Production => Some("E1Q7V5BFHGQLIG"),
        }
    }

    const fn root_url(&self) -> &'static str {
        match self {
            Environment::Local => "http://127.0.0.1:8888",
            Environment::Staging => "https://staging.renderling.xyz",
            Environment::Production => "https://renderling.xyz",
        }
    }
}

#[derive(Parser)]
#[clap(author, version, about)]
struct Cli {
    /// The deployment environment. Should be "local", "staging" or "production".
    #[clap(long, short = 'e', default_value = "local")]
    environment: Environment,

    /// The local build directory.
    #[clap(long, short = 'b', default_value = "site")]
    build_directory: String,

    /// Subcommand
    #[clap(subcommand)]
    cmd: Command,
}

fn get_files(dir: impl AsRef<std::path::Path>) -> Vec<std::path::PathBuf> {
    log::trace!("reading {}", dir.as_ref().display());
    if !(dir.as_ref().exists() && dir.as_ref().is_dir()) {
        log::error!(
            "{} does not exist, or is not a directory",
            dir.as_ref().display()
        );
        panic!("not a dir");
    }

    let mut files = vec![];
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() {
            files.push(path);
        } else if path.is_dir() {
            files.extend(get_files(path));
        }
    }
    files
}

fn pop_parent_replace_ext(
    path: impl AsRef<std::path::Path>,
    maybe_ext: Option<&str>,
) -> std::path::PathBuf {
    let mut path = path.as_ref().to_path_buf();
    let mut components = path.components().collect::<VecDeque<_>>();
    if path.parent().is_some() {
        let parent = components.pop_front().unwrap();
        path = path.strip_prefix(parent).unwrap().to_path_buf();
    }
    if let Some(ext) = maybe_ext {
        path = path.with_extension(ext);
    }
    path
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ManifestFile {
    origin: String,
    origin_modified: chrono::DateTime<chrono::FixedOffset>,
    built_filepath: std::path::PathBuf,
    destination: std::path::PathBuf,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct SiteManifest {
    environment: Environment,
    build_directory: std::path::PathBuf,
    files: BTreeMap<String, ManifestFile>,
}

impl SiteManifest {
    fn new(environment: Environment, build_directory: std::path::PathBuf) -> Self {
        let manifest_path = format!("{}.yaml", environment);
        if let Ok(file) = std::fs::File::open(&manifest_path) {
            log::info!("reading site manifest from {manifest_path}");
            serde_yaml::from_reader(file).unwrap()
        } else {
            SiteManifest {
                environment,
                build_directory,
                files: Default::default(),
            }
        }
    }

    fn clean(&mut self) {
        log::info!("cleaning '{}'", self.build_directory.display());
        if self.build_directory.is_dir() {
            log::debug!("removing build dir '{}'", self.build_directory.display());
            std::fs::remove_dir_all(&self.build_directory).unwrap();
        }
        log::debug!("creating build dir '{}'", self.build_directory.display());
        std::fs::create_dir_all(&self.build_directory).unwrap();
        self.files = Default::default();
    }

    fn site(&self) -> rxyz::Site {
        log::trace!(
            "creating site with root url: '{}'",
            self.environment.root_url()
        );
        rxyz::Site::new(self.environment.root_url()).unwrap()
    }

    fn build_devlog(&mut self) {
        let origin =
            "https://raw.githubusercontent.com/schell/renderling/main/DEVLOG.md".to_owned();
        let destination = std::path::PathBuf::from("devlog/index.html");
        let built_filepath = self.build_directory.join(&destination);
        let content = String::from_utf8(
            std::process::Command::new("curl")
                .arg(&origin)
                .output()
                .expect("could not curl the devlog")
                .stdout,
        )
        .unwrap();

        log::trace!("rendering the devlog to {}", built_filepath.display());
        let page_string = self.site().render_markdown_page(content).unwrap();
        log::trace!("  writing");
        if let Some(parent) = built_filepath.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&built_filepath, page_string).unwrap();
        log::trace!("  done!");

        let head = String::from_utf8(
            std::process::Command::new("curl")
                .arg("--head")
                .arg(&origin)
                .output()
                .expect("could not curl the devlog")
                .stdout,
        )
        .unwrap();
        log::info!("devlog: {head}");

        let headers = head
            .lines()
            .filter_map(|line| line.split_once(':'))
            .collect::<HashMap<_, _>>();
        let origin_modified = match headers.get("date") {
            None => {
                log::warn!("headers did not contain 'date'");
                chrono::Utc::now().fixed_offset()
            }
            Some(d) => {
                log::debug!("date: {d}");
                match chrono::DateTime::parse_from_rfc2822(d) {
                    Err(e) => {
                        log::error!("could not parse date: {e}");
                        chrono::Utc::now().fixed_offset()
                    }
                    Ok(d) => d,
                }
            }
        };

        self.files.insert(
            origin.clone(),
            ManifestFile {
                origin,
                origin_modified,
                destination,
                built_filepath,
            },
        );
    }

    fn build(&mut self) {
        self.clean();

        let content_dir = std::path::PathBuf::from("content");
        log::trace!("downloading devlog");

        //self.build_devlog();

        let files = get_files(content_dir);
        let (markdown_files, other_files): (Vec<_>, Vec<_>) = files
            .into_iter()
            .partition(|path| path.extension().map(|ext| ext == "md").unwrap_or_default());

        for file in markdown_files {
            let destination = pop_parent_replace_ext(&file, Some("html"));
            let built_filepath = self.build_directory.join(&destination);
            log::trace!(
                "rendering {} to {}",
                file.display(),
                built_filepath.display()
            );
            let origin = format!("{}", file.display());

            let mut file = std::fs::File::open(file).unwrap();
            let meta = file.metadata().unwrap();
            let origin_modified =
                chrono::DateTime::<chrono::Utc>::from(meta.modified().unwrap()).fixed_offset();

            let mut content = String::new();
            let _ = file.read_to_string(&mut content).unwrap();
            let page_string = self.site().render_markdown_page(content).unwrap();
            log::trace!("  writing");
            if let Some(parent) = built_filepath.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&built_filepath, page_string).unwrap();
            log::trace!("  done!");

            self.files.insert(
                origin.clone(),
                ManifestFile {
                    origin,
                    origin_modified,
                    destination,
                    built_filepath,
                },
            );
        }

        for file in other_files {
            let destination = pop_parent_replace_ext(&file, None);
            let built_filepath = self.build_directory.join(&destination);
            if let Some(parent) = built_filepath.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            log::trace!("copying {} to {}", file.display(), built_filepath.display());
            if !file.exists() {
                log::error!("file {} does not exist", file.display());
            }

            let origin = format!("{}", file.display());
            let mut file = std::fs::File::open(file).unwrap();
            let meta = file.metadata().unwrap();
            let origin_modified =
                chrono::DateTime::<chrono::Utc>::from(meta.modified().unwrap()).fixed_offset();
            let mut bytes = vec![];
            let _ = file.read_to_end(&mut bytes).unwrap();

            std::fs::write(&built_filepath, bytes).unwrap();

            self.files.insert(
                origin.clone(),
                ManifestFile {
                    origin,
                    origin_modified,
                    built_filepath,
                    destination,
                },
            );
        }

        let manifest_string = serde_yaml::to_string(&self).unwrap();
        let manifest_path = format!("{}.yaml", self.environment);
        std::fs::write(&manifest_path, manifest_string).unwrap();
        log::info!("build manifest saved to '{manifest_path}'");
    }

    async fn deploy(&mut self) {
        let bucket = if let Some(b) = self.environment.bucket() {
            b
        } else {
            log::error!("local environment cannot be deployed");
            panic!("environment error");
        };

        log::info!(
            "deploying '{}' to bucket '{bucket}'",
            self.build_directory.display(),
        );

        self.build();

        let config = aws_config::load_from_env()
            .await
            .to_builder()
            .region(aws_config::Region::new("us-west-1"))
            .build();
        let s3 = aws_sdk_s3::Client::new(&config);
        for mfile in self.files.values() {
            let key = format!("{}", mfile.destination.display());
            let content_type =
                new_mime_guess::from_path(&mfile.destination).first_or_octet_stream();
            log::info!("uploading '{bucket}' '{key}' as {content_type}");
            let result = s3
                .put_object()
                .bucket(bucket)
                .key(key)
                .acl(aws_sdk_s3::types::ObjectCannedAcl::PublicRead)
                .content_type(content_type.essence_str())
                .body(
                    aws_sdk_s3::primitives::ByteStream::from_path(&mfile.built_filepath)
                        .await
                        .unwrap(),
                )
                .send()
                .await;
            if let Err(e) = result {
                log::error!("{e}");
                panic!("s3 upload failed: {e:#?}");
            }
        }

        log::info!("done uploading to s3, invalidating the cloudfront cache");
        let cf = aws_sdk_cloudfront::Client::new(&config);
        let paths = self
            .files
            .values()
            .map(|mf| format!("/{}", mf.destination.display()))
            .collect::<Vec<_>>();
        log::debug!("paths: {paths:#?}");
        let result = cf
            .create_invalidation()
            .distribution_id(self.environment.cloudfront_distro().unwrap())
            .invalidation_batch(
                aws_sdk_cloudfront::types::InvalidationBatch::builder()
                    .paths(
                        aws_sdk_cloudfront::types::Paths::builder()
                            .quantity(paths.len() as i32)
                            .set_items(Some(paths))
                            .build()
                            .unwrap(),
                    )
                    .caller_reference("xtask")
                    .build()
                    .unwrap(),
            )
            .send()
            .await;
        match result {
            Ok(invalidation) => {
                log::info!("created invalidation: {invalidation:#?}");
            }
            Err(e) => {
                log::error!("{e}");
                panic!("cloudfront error: {e:#?}");
            }
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::builder()
        .filter_module("rustls", log::LevelFilter::Warn)
        .filter_module("hyper_rustls", log::LevelFilter::Warn)
        .init();

    let cli = Cli::parse();

    let mut manifest = SiteManifest::new(cli.environment, cli.build_directory.into());
    log::info!("starting manifest: {manifest:#?}");

    match cli.cmd {
        Command::Deploy => manifest.deploy().await,
        Command::Build => manifest.build(),
        Command::Clean => manifest.clean(),
    }

    log::info!("ending manifest: {manifest:#?}");
}

#[cfg(test)]
mod test {
    use crate::pop_parent_replace_ext;

    #[test]
    fn path_sanity() {
        let path = std::path::PathBuf::from("parent/child/file.ext");
        let new_path = pop_parent_replace_ext(path, Some("xyz"));
        assert_eq!(std::path::PathBuf::from("child/file.xyz"), new_path);
    }
}
