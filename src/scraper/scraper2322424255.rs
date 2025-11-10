use anyhow::{Context, Result, anyhow};
use futures::future::join_all;
use log::{error, info};
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Client;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::{fmt::Write, path::Path, sync::Arc, time::Duration};
use tokio::{
    fs::{self, metadata},
    sync::Semaphore,
    time::sleep,
};

use crate::scraper::{SCRAPE_DATA, ScrapingTarget};

static SEL: Lazy<Selector> =
    Lazy::new(|| Selector::parse("div#content.content").expect("Invalid content selector"));

static RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\(第(\d+)/(\d+)页\)").expect("Invalid part regex"));

const CONCURRENT_REQUEST_LIMIT: usize = 2;
const POST_JOB_DELAY_MS: u64 = 500;
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36";
const RETRY_LIMIT: u32 = 3;
const RETRY_DELAY_MS: u64 = 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicJob {
    pub id: String,
    pub position: u32,
}

#[derive(Debug)]
struct TopicPage {
    index: u32,
    content: String,
}

#[derive(Debug)]
struct TopicOutput {
    position: u32,
    content: String,
}

pub struct Scraper {
    base_url: String,
    jobs: Vec<TopicJob>,
}

impl Scraper {
    pub async fn new(target: ScrapingTarget) -> Result<Self> {
        let target_id = match target {
            ScrapingTarget::TalentInDemonicSect => "107391",
        };

        let jobs = read_scrape_data(target_id).await?;

        Ok(Self {
            base_url: format!("http://23.224.242.55/book/{}", target_id),
            jobs,
        })
    }

    pub async fn scrape(&self, save_path: impl AsRef<Path>) -> Result<()> {
        if metadata(&save_path).await.is_ok() {
            fs::remove_dir_all(&save_path).await?;
        }

        fs::create_dir_all(&save_path).await?;

        let client = Arc::new(
            Client::builder()
                .user_agent(USER_AGENT)
                .timeout(Duration::from_secs(15))
                .build()
                .context("Failed to build HTTP client.")?,
        );

        let semaphore = Arc::new(Semaphore::new(CONCURRENT_REQUEST_LIMIT));

        let mut tasks = Vec::new();

        for job in &self.jobs {
            let client_clone = Arc::clone(&client);
            let semaphore_clone = Arc::clone(&semaphore);
            let base_url = self.base_url.clone();
            let save_dir = save_path.as_ref().to_path_buf();
            let job = job.clone();

            tasks.push(tokio::spawn(async move {
                let permit = semaphore_clone
                    .acquire_owned()
                    .await
                    .expect("Semaphore closed");

                info!("Processing Chapter No: {}", job.position);

                let topic = process_single_job(&base_url, job, &client_clone).await?;

                let position = save_topic(&save_dir, topic).await?;

                sleep(Duration::from_millis(POST_JOB_DELAY_MS)).await;

                drop(permit);
                Ok::<_, anyhow::Error>(position)
            }));
        }

        let results = join_all(tasks).await;
        let mut success_count = 0;
        let mut failure_count = 0;

        for task_result in results {
            match task_result {
                Ok(Ok(_position)) => {
                    success_count += 1;
                }
                Ok(Err(app_error)) => {
                    failure_count += 1;
                    eprintln!("Scraping failed: {:?}", app_error);
                }
                Err(join_error) => {
                    eprintln!("A task panicked: {:?}", join_error);
                    failure_count += 1;
                }
            }
        }

        println!("\n--- Scraping Summary ---");
        println!("Total Jobs: {}", success_count + failure_count);
        println!("✅ Successes: {}", success_count);
        println!("❌ Failures: {}", failure_count);

        Ok(())
    }
}

async fn process_single_job(base_url: &str, job: TopicJob, client: &Client) -> Result<TopicOutput> {
    let page_1 = fetch_page_with_retries(base_url, &job.id, 1, client).await?;

    let parsed_page_1 = parse_content(&page_1);

    let total_pages: u32 = RE
        .captures(&page_1.content)
        .and_then(|caps| caps.get(2))
        .and_then(|m| m.as_str().parse().ok())
        .unwrap_or(1);

    let mut all_parsed_content = vec![parsed_page_1];

    if total_pages > 1 {
        for index in 2..=total_pages {
            let page = fetch_page_with_retries(base_url, &job.id, index, client).await?;
            let parsed_page = parse_content(&page);
            all_parsed_content.push(parsed_page);
        }
    }

    let combined_content = all_parsed_content.join("\n\n");

    Ok(TopicOutput {
        position: job.position,
        content: combined_content,
    })
}

/// Fetches a single page with retries.
///
/// Returns `Ok(Some(TopicPage))` on success.
/// Returns `Ok(None)` on a 404 Not Found.
/// Returns `Err(anyhow::Error)` on a persistent error (e.g., network, 5xx) after all retries.
async fn fetch_page_with_retries(
    base_url: &str,
    job_id: &str,
    index: u32,
    client: &Client,
) -> Result<TopicPage> {
    let url = build_url(base_url, job_id, index);

    let mut attempts = 0;
    loop {
        attempts += 1;

        match client.get(&url).send().await {
            Ok(response) => {
                // Check for other non-success statuses (e.g., 500, 403)
                match response.error_for_status() {
                    Ok(success_response) => {
                        // HTTP 200 OK
                        let content = success_response
                            .text()
                            .await
                            .context("Failed to read response text")?;
                        return Ok(TopicPage { index, content });
                    }
                    Err(status_error) => {
                        // 4xx or 5xx error
                        if attempts >= RETRY_LIMIT {
                            return Err(anyhow!(
                                "Request for {} failed with status {} after {} attempts",
                                url,
                                status_error.status().unwrap_or_default(),
                                RETRY_LIMIT
                            ));
                        } else {
                            error!(
                                "Request for {} failed with status {}",
                                url,
                                status_error.status().unwrap_or_default()
                            );
                        }
                    }
                }
            }
            Err(network_error) => {
                // Network error, timeout, etc.
                if attempts >= RETRY_LIMIT {
                    return Err(anyhow!(
                        "Request for {} failed ({}) after {} attempts",
                        url,
                        network_error,
                        RETRY_LIMIT
                    ));
                }
            }
        }

        // If we're here, the request failed, and we need to retry.
        let delay_ms = RETRY_DELAY_MS * 2u64.pow(attempts - 1);

        println!(
            "   -> Retrying {} (Attempt {}/{}). Waiting {}ms...",
            url,
            attempts + 1,
            RETRY_LIMIT,
            delay_ms
        );

        sleep(Duration::from_millis(delay_ms)).await;
    }
}

fn build_url(base_url: &str, id: &str, index: u32) -> String {
    if index == 1 {
        format!("{}/{}.html", base_url, id)
    } else {
        format!("{}/{}_{}.html", base_url, id, index)
    }
}

fn parse_content(page: &TopicPage) -> String {
    let doc = Html::parse_document(&page.content);

    doc.select(&SEL)
        .next()
        .map(|root_el| {
            let mut peekable_lines = root_el
                .text()
                .filter_map(|text| {
                    let trimmed = text.trim();
                    if trimmed.is_empty()
                        || matches!(
                            trimmed,
                            "read2();" | "read3();" | "（本章未完，请点击下一页继续阅读）"
                        )
                    {
                        None
                    } else {
                        Some(trimmed)
                    }
                })
                .peekable();

            let skip_title = page.index != 1
                && peekable_lines
                    .peek()
                    .map_or(false, |line| line.starts_with("第"));

            if skip_title {
                peekable_lines.next();
            }

            let mut text_content = String::new();

            if let Some(first_line) = peekable_lines.next() {
                let _ = write!(text_content, "{}", first_line);

                for line in peekable_lines {
                    let _ = write!(text_content, "\n\n{}", line);
                }
            }

            RE.replace_all(&text_content, "").into_owned()
        })
        .unwrap_or_default()
}

async fn save_topic(save_dir: &Path, topic: TopicOutput) -> Result<u32> {
    let file_name = format!("{}.md", topic.position);
    let file_path = save_dir.join(&file_name);

    fs::write(&file_path, &topic.content)
        .await
        .with_context(|| {
            format!(
                "Failed to write Chapter {} to file {}",
                topic.position, &file_name
            )
        })?;

    info!("Successfully saved: {}", &file_name);

    Ok(topic.position)
}

async fn read_scrape_data(target_id: &str) -> Result<Vec<TopicJob>> {
    info!("JSON file: {}", format!("{}.json", target_id));

    // We can safely unwrap because we this exists.
    let content = SCRAPE_DATA
        .get_file(format!("{}/{}.json", "2322424255", target_id))
        .unwrap() // This should now succeed
        .contents_utf8()
        .unwrap();

    let data = serde_json::from_str(content).with_context(|| {
        format!(
            "Failed to parse scrape data for {} of 2322424255.",
            target_id
        )
    })?;

    Ok(data)
}
