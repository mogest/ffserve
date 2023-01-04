use std::time::Instant;
use uuid::Uuid;

use crate::command::run_command;
use crate::config::CONFIG;
use crate::models::{build_path, FileType, Job, MutexedJobs, State};

fn error_to_io_error<T, U>(result: Result<T, U>) -> Result<T, std::io::Error>
where
    U: ToString,
{
    result.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
}

fn upload(id: Uuid, url: &str) -> std::io::Result<()> {
    let file = std::fs::File::open(build_path(id, FileType::Output))?;

    let client = reqwest::blocking::Client::new();

    let response = client.put(url).body(file).send();

    error_to_io_error(error_to_io_error(response)?.error_for_status())?;

    Ok(())
}

fn build_arguments(id: Uuid, arguments: &str) -> Vec<String> {
    arguments
        .split(" ")
        .map(|arg| match arg {
            "$INPUT" => build_path(id, FileType::Input),
            "$OUTPUT" => build_path(id, FileType::Output),
            other => other.to_owned(),
        })
        .collect()
}

fn update_state(
    guarded_jobs: MutexedJobs,
    id: Uuid,
    new_state: State,
    error_message: Option<String>,
) -> Option<Job> {
    let mut jobs = guarded_jobs.lock().unwrap();

    if let Some(mut job) = jobs.get_mut(&id) {
        match new_state {
            State::Done | State::Error => job.completed_at = Some(Instant::now()),
            _ => (),
        }

        job.state = new_state;
        job.error_message = error_message;

        return Some(job.clone());
    }

    return None;
}

fn transcode(job: Job) -> std::io::Result<()> {
    println!("[{}] processor: starting ffmpeg pass 1", job.id);
    run_command(
        "ffmpeg",
        build_arguments(job.id, CONFIG.ffmpeg_arguments_pass_1),
        &"ffmpeg pass 1",
    )?;

    if let Some(arguments) = CONFIG.ffmpeg_arguments_pass_2 {
        println!("[{}] processor: starting ffmpeg pass 2", job.id);
        run_command(
            "ffmpeg",
            build_arguments(job.id, arguments),
            &"ffmpeg pass 2",
        )?;
    }

    println!(
        "[{}] processor: starting upload to URL {}",
        job.id, job.dest_url
    );

    upload(job.id, &job.dest_url)?;

    return Ok(());
}

fn process_job(id: Uuid, jobs: MutexedJobs) {
    if let Some(job) = update_state(jobs.clone(), id, State::Processing, None) {
        match transcode(job) {
            Ok(_) => {
                update_state(jobs.clone(), id, State::Done, None);
                println!("[{}] processor: complete", id);
            }
            Err(err) => {
                update_state(jobs.clone(), id, State::Error, Some(err.to_string()));
                println!("[{}] processor: ended with error: {}", id, err.to_string());
            }
        };
    } else {
        println!("[{}] processor: no such job registered, ignoring", id);
    }
}

pub fn processor(rx: std::sync::mpsc::Receiver<Uuid>, jobs: MutexedJobs) {
    println!("Starting transcoding processor...");

    loop {
        match rx.recv() {
            Ok(id) => process_job(id, jobs.clone()),
            Err(_) => {
                println!("Transcoding processor shutdown");
                return;
            }
        }
    }
}
