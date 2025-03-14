use anyhow::{Context, Result};
use clap::Parser;
use futures_util::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::{Client, header, redirect};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Semaphore;
use url::Url;

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    #[clap(required = true)]
    url: String,

    // Output file name: default to last part of URL
    #[clap(short, long)]
    output: Option<PathBuf>,

    // Show verbose output
    #[clap(short, long)]
    verbose: bool,

    #[clap(short = 'r', long, default_value = "10")]
    max_redirects: usize,

    // Disable following redirects
    #[clap(long)]
    no_follow: bool,

    #[clap(short = 'c', long)]
    continue_download: bool,

    // Force overwrite existing files
    #[clap(short = 'f', long)]
    force: bool,

    #[clap(short = 'R', long)]
    recursive: bool,

    // Maximum recursion depth
    #[clap(short = 'l', long, default_value = "5")]
    max_depth: usize,

    #[clap(short = 'j', long, default_value = "5")]
    max_concurrent: usize,

    #[clap(short = 'd', long)]
    same_domain: bool,
}

async fn download_file(client: &Client, url: &Url, output_path: &Path, args: &Args) -> Result<()> {
    let file_exists = output_path.exists();
    let mut downloaded_size = 0;

    if file_exists {
        if args.force {
            if args.verbose {
                println!(
                    "File exists, overwriting due to --force (-f) flag: {}",
                    output_path.display()
                );
            }
        } else if args.continue_download {
            downloaded_size = std::fs::metadata(output_path)
                .context("Failed to get file metadata")?
                .len();

            if args.verbose {
                println!(
                    "Resuming download from byte pos {} for {}",
                    downloaded_size,
                    output_path.display()
                );
            }
        } else {
            if args.verbose {
                println!("Skipping existing file: {}", output_path.display());
            }

            return Ok(());
        }
    }

    // Prepare request with range header if resuming
    let mut request = client.get(url.clone());

    if downloaded_size > 0 {
        request = request.header(header::RANGE, format!("bytes={}-", downloaded_size));
    }

    let response = request.send().await.context("Failed to send request")?;

    if args.verbose && response.url().to_string().ne(&url.to_string()) {
        println!("Request was redirected to: {}", response.url());
    }

    let status = response.status();

    // Handle 206 - Partial Content
    let is_partial = status.as_u16() == 206;

    if !status.is_success() && !is_partial {
        anyhow::bail!("Server return error status: {} for {}", status, url);
    }

    if downloaded_size > 0 && !is_partial {
        if args.verbose {
            println!(
                "Server doesnt support resume, download from the beginning for {}",
                url
            );
        }

        downloaded_size = 0;
    }

    let content_length = response.content_length().unwrap_or(0);
    let total_size = if is_partial {
        downloaded_size + content_length
    } else {
        content_length
    };

    let pb = if total_size > 0 {
        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta}) {msg}")
            .unwrap()
            .progress_chars("#>-"));

        pb.set_message(format!("{}", output_path.display()));

        // Set init pos if resuming
        if downloaded_size > 0 {
            pb.set_position(downloaded_size);
        }

        Some(pb)
    } else {
        None
    };

    let mut file = if downloaded_size > 0 {
        OpenOptions::new()
            .append(true)
            .open(output_path)
            .with_context(|| {
                format!(
                    "Failed to open file for appending: {}",
                    output_path.display()
                )
            })?
    } else {
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).context("Failed to create parent directories")?;
        }

        File::create(output_path)
            .with_context(|| format!("Failed to create file: {}", output_path.display()))?
    };

    let is_html = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.contains("text/html"))
        .unwrap_or(false);

    let mut stream = response.bytes_stream();
    let mut content = Vec::new();

    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.context("Error while downloading file")?;
        file.write_all(&chunk)
            .with_context(|| format!("Failed to write to file: {}", output_path.display()))?;

        if is_html && args.recursive {
            content.extend_from_slice(&chunk);
        }

        if let Some(pb) = &pb {
            pb.inc(chunk.len() as u64);
        }
    }

    if let Some(pb) = pb {
        pb.finish_with_message(format!("Downloaded: {}", output_path.display()));
    }

    if args.verbose {
        println!("Download complete: {}", output_path.display());
    }

    // Return the content if it's HTML and we're doing recursive download
    Ok(())
}

async fn download_recursively(
    client: &Client,
    url: Url,
    base_dir: &Path,
    depth: usize,
    max_depth: usize,
    visited: &mut HashSet<String>,
    _multi_progress: Arc<MultiProgress>,
    semaphore: Arc<Semaphore>,
    args: &Args,
) -> Result<()> {
    let url_str = url.to_string();

    if visited.contains(&url_str) {
        return Ok(());
    }

    // Mark as visited
    visited.insert(url_str);

    if depth > max_depth {
        return Ok(());
    }

    let path = url.path();
    let path = if path.ends_with("/") || path.is_empty() {
        "index.html"
    } else {
        path.trim_start_matches("/")
    };

    let output_path = base_dir.join(path);

    // Using semaphore to limit concurrent downloads
    let _permit = semaphore.acquire().await?;
    download_file(client, &url, &output_path, args).await?;

    let response = client
        .get(url.clone())
        .send()
        .await
        .context("Failed to send request")?;

    if !response.status().is_success() {
        if args.verbose {
            println!("Skipping {} due to status code {}", url, response.status());
        }

        return Ok(());
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let url = Url::parse(&args.url).context("Failed to parse URL")?;

    let base_dir = if args.recursive {
        // For recursive download, createe a directory based on the domain name
        let domain = url.host_str().unwrap_or("download");
        let base_dir = PathBuf::from(domain);
        fs::create_dir_all(&base_dir).context("Failed to create base dir")?;

        base_dir
    } else {
        // For single file
        PathBuf::from(".")
    };

    let output_path = if args.recursive {
        let path = url.path();
        let path = if path.ends_with("/") || path.is_empty() {
            "index.html"
        } else {
            path.trim_start_matches("/")
        };

        base_dir.join(path)
    } else {
        match &args.output {
            Some(path) => path.clone(),
            None => {
                let filename = url
                    .path_segments()
                    .and_then(|segments| segments.last())
                    .unwrap_or("index.html");

                PathBuf::from(filename)
            }
        }
    };

    if let Some(parent) = output_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).context("Failed to create parent dir")?;
        }
    }

    if args.verbose {
        println!("Downloading {} to {}", args.url, output_path.display());
    }

    let redirect_policy = if args.no_follow {
        redirect::Policy::none()
    } else {
        redirect::Policy::limited(args.max_redirects)
    };

    let client = Client::builder()
        .redirect(redirect_policy)
        .build()
        .context("Failed to build HTTP client")?;

    if args.recursive {
        // For recursive downloads, use a different approach
        let multi_progress = Arc::new(MultiProgress::new());
        let semaphore = Arc::new(Semaphore::new(args.max_concurrent));
        let mut visited = HashSet::new();

        download_recursively(
            &client,
            url,
            &base_dir,
            0,
            args.max_depth,
            &mut visited,
            Arc::clone(&multi_progress),
            Arc::clone(&semaphore),
            &args,
        )
        .await?;

        println!(
            "Recursive download complete! Downloaded {} files.",
            visited.len()
        );
    } else {
        download_file(&client, &url, &output_path, &args).await?;
    }

    Ok(())
}
