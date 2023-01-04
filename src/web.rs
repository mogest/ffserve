use actix_web::{
    error, get, http::StatusCode, post, web, App, HttpMessage, HttpResponse, HttpServer, Responder,
    Result,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::mpsc::SyncSender;
use std::time::Duration;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::command::{probe, Metadata};
use crate::config::{VideoOrientation, CONFIG};
use crate::models::{build_path, FileType, Job, MutexedJobs, State};

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
    metadata: Metadata,
}

#[derive(Serialize)]
enum ErrorType {
    InvalidVideo,
    VideoTooLong,
    IncorrectOrientation,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: ErrorType,
    description: String,
}

fn validate_video_metadata(id: Uuid) -> std::result::Result<Metadata, (ErrorType, String)> {
    let path = build_path(id, FileType::Input);

    let metadata = match probe(&path) {
        Ok(data) => data,
        Err(err) => {
            println!("[{}] Probe failed: {:?}", id, err);
            return Err((
                ErrorType::InvalidVideo,
                "Supplied file is not in a recognised video format".to_owned(),
            ));
        }
    };

    println!("[{}] Video metadata: {:?}", id, metadata);

    if let Some(max_length) = CONFIG.maximum_video_length {
        if Duration::from_secs(metadata.duration) > max_length {
            println!("[{}] Failed, duration too long", id);
            return Err((
                ErrorType::VideoTooLong,
                format!(
                    "Video duration is greater than {} seconds",
                    max_length.as_secs()
                ),
            ));
        }
    }

    match CONFIG.require_orientation {
        Some(VideoOrientation::Landscape) => {
            if metadata.height > metadata.width {
                println!("[{}] Failed, incorrect orientation", id);
                return Err((
                    ErrorType::IncorrectOrientation,
                    "Video is in portrait orientation, only landscape videos are accepted"
                        .to_owned(),
                ));
            }
        }

        Some(VideoOrientation::Portrait) => {
            if metadata.height < metadata.width {
                println!("[{}] Failed, incorrect orientation", id);
                return Err((
                    ErrorType::IncorrectOrientation,
                    "Video is in landscape orientation, only portrait videos are accepted"
                        .to_owned(),
                ));
            }
        }

        None => {}
    }

    Ok(metadata)
}

#[get("/")]
async fn status(data: web::Data<AppState>) -> Result<impl Responder> {
    let mut jobs = data.jobs.lock().unwrap();

    let mut response = StatusResponse { jobs: vec![] };

    let mut expired_jobs = vec![];

    for (_, job) in jobs.iter() {
        if let Some(time) = job.completed_at {
            if time.elapsed() > CONFIG.expire_completed_jobs_after {
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

fn build_error_response(error_type: ErrorType, description: &str) -> HttpResponse {
    HttpResponse::build(StatusCode::BAD_REQUEST).json(ErrorResponse {
        error: error_type,
        description: description.to_owned(),
    })
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

    println!("[{}] downloading source URL {}", job.id, job.source_url);

    let mut stream = download(&job.source_url).await?;

    let path = build_path(job.id, FileType::Input);

    {
        let mut file = fs::File::create(&path).await?;

        while let Some(item) = stream.next().await {
            tokio::io::copy(&mut item?.as_ref(), &mut file).await?;
        }

        file.flush().await?;
    }

    let metadata = match validate_video_metadata(job.id) {
        Ok(metadata) => metadata,
        Err((error_type, description)) => {
            let _ = tokio::fs::remove_file(path).await;
            return Ok(build_error_response(error_type, &description));
        }
    };

    let id = job.id;

    let response = SubmitResponse {
        id: id.to_string(),
        metadata,
    };

    let mut jobs = data.jobs.lock().unwrap();
    jobs.insert(job.id, job);

    data.channel
        .send(id)
        .map_err(|_| error::ErrorInternalServerError("Failed to internally queue"))?;

    Ok(HttpResponse::build(StatusCode::OK).json(response))
}

async fn download(
    url: &str,
) -> Result<impl futures::stream::Stream<Item = Result<web::Bytes, error::PayloadError>>> {
    let mut res =
        awc::Client::default().get(url).send().await.map_err(|e| {
            error::ErrorBadRequest(format!("GET request failed: {}", e.to_string()))
        })?;

    return Ok(res.take_payload());
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
