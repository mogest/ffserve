use std::process::{Command, Stdio};
use std::time::Instant;
use uuid::Uuid;

use crate::models::{Job, FileType, MutexedJobs, State, build_path};

const ARGUMENTS_COMMON: &'static str = "-i $INPUT -vf scale=1280x720 -b:v 1024k -minrate 512k -maxrate 1485k -tile-columns 2 -g 240 -quality good -crf 32 -c:v libvpx-vp9 -speed 4 -map_metadata -1";
const ARGUMENTS_PASS_1: &'static str = "-pass 1 -an -f null /dev/null";
const ARGUMENTS_PASS_2: &'static str = "-pass 2 -c:a libopus -y $OUTPUT";

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

enum Pass { One, Two }

fn build_arguments(id: Uuid, pass: Pass) -> Vec<String> {
    let arguments = match pass {
        Pass::One => format!("{ARGUMENTS_COMMON} {ARGUMENTS_PASS_1}"),
        Pass::Two => format!("{ARGUMENTS_COMMON} {ARGUMENTS_PASS_2}"),
    };

    arguments.split(" ").map(|arg|
        match arg {
            "$INPUT" => build_path(id, FileType::Input),
            "$OUTPUT" => build_path(id, FileType::Output),
            other => other.to_owned()
        }
    ).collect()
}

fn run_ffmpeg(arguments: Vec<String>, descriptor: &str) -> std::io::Result<()> {
    let mut process = Command::new("ffmpeg")
        .args(arguments)
        .stdout(Stdio::piped())
        .spawn()?;

    let exit_status = process.wait()?;

    if !exit_status.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("{descriptor} failed"),
        ));
    }

    return Ok(());
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
    run_ffmpeg(build_arguments(job.id, Pass::One), &"ffmpeg pass 1")?;
    run_ffmpeg(build_arguments(job.id, Pass::Two), &"ffmpeg pass 2")?;

    println!("[{}] processor: starting upload to URL {}", job.id, job.dest_url);

    upload(job.id, &job.dest_url)?;

    return Ok(());
}

fn process_job(id: Uuid, jobs: MutexedJobs) {
    if let Some(job) = update_state(jobs.clone(), id, State::Processing, None) {
        println!("[{}] processor: starting ffmpeg", id);

        match transcode(job) {
            Ok(_) => {
                update_state(jobs.clone(), id, State::Done, None);
                println!("[{}] processor: complete", id);
            },
            Err(err) => {
                update_state(jobs.clone(), id, State::Error, Some(err.to_string()));
                println!("[{}] processor: ended with error: {}", id, err.to_string());
            },
        };
    }
    else {
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
