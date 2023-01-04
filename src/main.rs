use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

use crate::web::start_web_server;
use crate::processor::processor;

mod web;
mod models;
mod processor;
mod command;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    std::env::set_current_dir("data").expect("Failed trying to change to a directory called `data`.  Make a `data` directory inside the current working directory so ffserve has somewhere to store files and processing logs.");

    let (tx, rx) = mpsc::sync_channel(1024 * 50);

    let jobs = Arc::new(Mutex::new(HashMap::new()));
    let jobs_clone = jobs.clone();

    thread::spawn(move || processor(rx, jobs_clone));

    start_web_server(jobs, tx).await
}
