use actix_web::{error, get, post, web, App, HttpMessage, HttpServer, Responder, Result};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::mpsc::SyncSender;
use std::time::Duration;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::models::{build_path, FileType, Job, MutexedJobs, State};

const COMPLETED_EXPIRY_DURATION: Duration = Duration::from_secs(60 * 60);

struct AppState {
    jobs: MutexedJobs,
    channel: SyncSender<Uuid>,
}

#[derive(Serialize, Debug, Clone)]
struct JobState {
    id: String,
    state: String,
    error_message: Option<String>,
}

#[derive(Serialize)]
struct StatusResponse {
    jobs: Vec<JobState>,
}

#[derive(Deserialize)]
struct SubmitParams {
    source_url: String,
    dest_url: String,
}

#[derive(Serialize)]
struct SubmitResponse {
    id: String,
}

#[get("/")]
async fn status(data: web::Data<AppState>) -> Result<impl Responder> {
    let mut jobs = data.jobs.lock().unwrap();

    let mut response = StatusResponse { jobs: vec![] };

    let mut expired_jobs = vec![];

    for (_, job) in jobs.iter() {
        if let Some(time) = job.completed_at {
            if time.elapsed() > COMPLETED_EXPIRY_DURATION {
                expired_jobs.push(job.id);
                continue;
            }
        }

        response.jobs.push(JobState {
            id: job.id.to_string(),
            state: format!("{:?}", job.state),
            error_message: job.error_message.clone(),
        });
    }

    for id in expired_jobs {
        jobs.remove(&id);

        // It doesn't matter if these files are here or not.
        let _ = fs::remove_file(build_path(id, FileType::Input)).await;
        let _ = fs::remove_file(build_path(id, FileType::Output)).await;
    }

    Ok(web::Json(response))
}

#[post("/")]
async fn submit(
    data: web::Data<AppState>,
    submit_params: web::Json<SubmitParams>,
) -> Result<impl Responder> {
    let job = Job {
        id: Uuid::new_v4(),
        source_url: submit_params.source_url.to_owned(),
        dest_url: submit_params.dest_url.to_owned(),
        state: State::Waiting,
        error_message: None,
        completed_at: None,
    };

    let mut res = awc::Client::default()
        .get(&job.source_url)
        .send()
        .await
        .map_err(|e| error::ErrorBadRequest(format!("GET request failed: {}", e.to_string())))?;

    let mut stream = res.take_payload();

    let mut file = fs::File::create(build_path(job.id, FileType::Input)).await?;

    while let Some(item) = stream.next().await {
        tokio::io::copy(&mut item?.as_ref(), &mut file).await?;
    }

    file.flush().await?;

    let id = job.id;

    let response = SubmitResponse {
        id: id.to_string(),
    };

    let mut jobs = data.jobs.lock().unwrap();
    jobs.insert(job.id, job);

    data.channel
        .send(id)
        .map_err(|_| error::ErrorInternalServerError("Failed to internally queue"))?;

    Ok(web::Json(response))
}

fn get_port() -> u16 {
    env::var("PORT")
        .map_err(|_| ())
        .and_then(|string| string.parse::<u16>().map_err(|_| ()))
        .unwrap_or(3600)
}

pub async fn start_web_server(jobs: MutexedJobs, tx: SyncSender<Uuid>) -> std::io::Result<()> {
    let app_state = web::Data::new(AppState { jobs, channel: tx });
    let port = get_port();

    println!("Starting web server at 0.0.0.0:{port}...");

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .service(status)
            .service(submit)
    })
    .bind(("0.0.0.0", get_port()))?
    .run()
    .await
}
