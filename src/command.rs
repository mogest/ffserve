use regex::Regex;
use serde::Serialize;
use std::process::{Command, Output};

fn io_err(message: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, message.to_string())
}

pub fn run_command(
    executable: &str,
    arguments: Vec<String>,
    descriptor: &str,
) -> std::io::Result<Output> {
    let output = Command::new(executable).args(arguments).output()?;

    if !output.status.success() {
        match String::from_utf8(output.stderr) {
            Ok(text) => return Err(io_err(&format!("{descriptor} failed\n\n{text}"))),
            Err(_) => {
                return Err(io_err(&format!(
                    "{descriptor} failed and the output was not UTF-8"
                )))
            }
        }
    }

    return Ok(output);
}

#[derive(Serialize, Debug)]
pub struct Metadata {
    pub width: u32,
    pub height: u32,
    pub duration: u64,
}

fn cap_u32(cap: &regex::Captures, i: usize) -> u32 {
    cap.get(i).unwrap().as_str().parse::<u32>().unwrap()
}

fn cap_u64(cap: &regex::Captures, i: usize) -> u64 {
    cap.get(i).unwrap().as_str().parse::<u64>().unwrap()
}

pub fn probe(path: &str) -> std::io::Result<Metadata> {
    let output = run_command("ffprobe", vec![path.to_string()], "ffprobe")?;

    let text = String::from_utf8(output.stderr).map_err(|_| io_err("invalid encoding"))?;

    println!("probe output: {}", text);

    let duration_re = Regex::new(r"(?m)^  Duration: (\d\d):(\d\d):(\d\d)\.\d\d,").unwrap();
    let duration_cap = duration_re
        .captures(&text)
        .ok_or(io_err("no duration found"))?;

    let duration = cap_u64(&duration_cap, 1) * 3600
        + cap_u64(&duration_cap, 2) * 60
        + cap_u64(&duration_cap, 3);

    let resolution_re = Regex::new(r"(?m)^  Stream [^ ]+: Video: .*, (\d\d\d+)x(\d\d\d+)").unwrap();
    let resolution_cap = resolution_re
        .captures(&text)
        .ok_or(io_err("no resolution found"))?;

    let width = cap_u32(&resolution_cap, 1);
    let height = cap_u32(&resolution_cap, 2);

    Ok(Metadata {
        width,
        height,
        duration,
    })
}
