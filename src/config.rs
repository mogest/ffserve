use std::time::Duration;

pub enum VideoOrientation {
    Portrait,
    Landscape,
}

pub struct Config {
    pub ffmpeg_arguments_pass_1: &'static str,
    pub ffmpeg_arguments_pass_2: Option<&'static str>,
    pub maximum_video_length: Option<Duration>,
    pub require_orientation: Option<VideoOrientation>,
    pub expire_completed_jobs_after: Duration,
}

// FIXME : these should be options (environment variables?) instead of being hardcoded

pub static CONFIG: Config = Config {
    ffmpeg_arguments_pass_1: "-i $INPUT -vf scale=1280x720 -b:v 1024k -minrate 512k -maxrate 1485k -tile-columns 2 -g 240 -quality good -crf 32 -c:v libvpx-vp9 -speed 4 -map_metadata -1 -pass 1 -an -f null /dev/null",
    ffmpeg_arguments_pass_2: Some("-i $INPUT -vf scale=1280x720 -b:v 1024k -minrate 512k -maxrate 1485k -tile-columns 2 -g 240 -quality good -crf 32 -c:v libvpx-vp9 -speed 4 -map_metadata -1 -pass 2 -c:a libopus -y $OUTPUT"),
    maximum_video_length: Some(Duration::from_secs(2 * 60)),
    require_orientation: Some(VideoOrientation::Landscape),
    expire_completed_jobs_after: Duration::from_secs(60 * 60),
};
