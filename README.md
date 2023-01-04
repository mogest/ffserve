# ffserve

A Dockerised, ffmpeg-based transcoding service with an HTTP API.

## Using it

Start it up:

    docker run --rm -it -p 3600:3600 ghcr.io/mogest/ffserve:latest

Next, issue a transcoding request.  Pass in a source URL, from where the source video will be downloaded, and
a destination URL, where the transcoded video will be uploaded with a PUT request post-processing:

    curl -d '{"source_url": "https://source.files/some.mov", "dest_url": "https://dest.files/some.webm"}' -H "content-type: application/json" localhost:3600

    {"id":"bd6e3823-eb2b-4f00-9a77-88dbeeeeec8e","metadata":{"width":1920,"height":1080,"duration":35}}

If the source URL is not found, the file is not a video, or it doesn't meet validation criteria (see below), a 400
error will be returned.  The `error` value will be one of `InvalidVideo`, `VideoTooLong`, `IncorrectOrientation`.

    {"error":"InvalidVideo","description":"Supplied file is not in a recognised video format"}

Once successfully submitted, you can check on the transcoding progress:

    curl localhost:3600

    {"jobs":[{"id":"bd6e3823-eb2b-4f00-9a77-88dbeeeeec8e","state":"Processing","error_message":null}]}

States are `Waiting` (queued up waiting for the transcoder), `Processing` (currently being transcoded), `Done`
(transcoding successfully completed) and `Error` (transcoding failed).

If the state is `Error`, the `error_message` value will be set.

Completed transcoding requests are removed from this list after an hour.

## Configuration

At the moment the configuration is hardcoded in `config.rs` to:

  * ensure the source video is less than two minutes long
  * ensure the source video is in landscape orientation
  * do a two-pass transcode to VP9/webm at 1280x720 at average 1Mbps

I'll make this configurable at some stage.

## License

MIT license.  Copyright 2023 Mog Nesbitt.
