use gstreamer_app::glib::BoolError;
use gstreamer_app::gst;
use gst::prelude::*;

pub fn create_source(stream_id: u32) -> Result<(gst::Element, gst::Element), BoolError> {
    let app_src = gst::ElementFactory::make("appsrc")
        .property("name", &format!("appsrc-{}", stream_id))
        .property("format", gst::Format::Time)
        .build()?;
    let flvdemux = gst::ElementFactory::make("flvdemux")
        .property("name", &format!("flvdemux-{}", stream_id))
        .build()?;
    Ok((app_src, flvdemux))
}

pub fn create_video(stream_id: u32) -> Result<(gst::Element, gst::Element), BoolError> {
    let video_queue = gst::ElementFactory::make("queue")
        .property("name", &format!("videoqueue-{}", stream_id))
        .build()?;

    let h264parse = gst::ElementFactory::make("h264parse")
        .property("name", &format!("h264parse-{}", stream_id))
        .property("config-interval", -1i32)
        .build()?;

    Ok((video_queue, h264parse))
}

pub fn create_audio(stream_id: u32) -> Result<(gst::Element, gst::Element), BoolError> {
    let audio_queue = gst::ElementFactory::make("queue")
        .property("name", &format!("audioqueue-{}", stream_id))
        .build()?;

    let aac_parse = gst::ElementFactory::make("aacparse")
        .property("name", &format!("aacparse-{}", stream_id))
        .build()?;

    Ok((audio_queue, aac_parse))
}

pub fn create_output(stream_id: u32, root_playlist: &str) -> Result<gst::Element, BoolError> {
    let s3_config = crate::config::get_config().s3.clone();
    let awss3hlssink = gst::ElementFactory::make("awss3hlssink")
        .property("name", &format!("awss3hlssink-{}", stream_id))
        .property("bucket", &s3_config.bucket)
        .property("region", &s3_config.region)
        .property("access-key", &s3_config.access_key)
        .property("secret-access-key", &s3_config.secret_access_key)
        .property("endpoint-uri", &s3_config.endpoint_uri)
        .property("key-prefix", &format!("hls_output/{}", stream_id))
        .build()?;

    let hlssink: gst::Element = awss3hlssink.property("hlssink");
    hlssink.set_property("playlist-root", root_playlist.to_string());
    hlssink.set_property("playlist-location", "playlist.m3u8");
    hlssink.set_property("location", "segment_%05d.ts");
    hlssink.set_property("target-duration", 2u32);
    hlssink.set_property("max-files", 5u32);

    Ok(awss3hlssink)
}

pub fn create_thumbnail_output(
    stream_id: u32,
) -> Result<
    (
        gst::Element,
        gst::Element,
        gst::Element,
        gst::Element,
        gst::Element,
        gst::Element,
        gst::Element,
        gst::Element,
        gst::Element,
    ),
    BoolError,
> {
    let s3_config = crate::config::get_config().s3.clone();

    let queue = gst::ElementFactory::make("queue")
        .name(&format!("queue-thumb-{}", stream_id))
        .property_from_str("leaky", "downstream") 
        .build()?;

    let h264parse = gst::ElementFactory::make("h264parse")
        .name(&format!("h264parse-thumb-{}", stream_id))
        .build()?;

    let avdec_h264 = gst::ElementFactory::make("avdec_h264")
        .name(&format!("avdec-thumb-{}", stream_id))
        .build()?;

    let videoconvert = gst::ElementFactory::make("videoconvert")
        .name(&format!("videoconvert-thumb-{}", stream_id))
        .build()?;

    let videoscale = gst::ElementFactory::make("videoscale")
        .name(&format!("videoscale-thumb-{}", stream_id))
        .build()?;

    let videorate = gst::ElementFactory::make("videorate")
        .name(&format!("videorate-thumb-{}", stream_id))
        .build()?;

    let caps = gst::Caps::builder("video/x-raw")
        .field("width", 640)
        .field("height", 360)
        .field("framerate", gst::Fraction::new(1, 30))
        .build();

    let capsfilter = gst::ElementFactory::make("capsfilter")
        .name(&format!("capsfilter-thumb-{}", stream_id))
        .property("caps", &caps)
        .build()?;

    let jpegenc = gst::ElementFactory::make("jpegenc")
        .name(&format!("jpegenc-thumb-{}", stream_id))
        .build()?;

    let awss3putobjectsink = gst::ElementFactory::make("awss3putobjectsink")
        .name(&format!("thumbsink-{}", stream_id))
        .property("bucket", &s3_config.bucket)
        .property("region", &s3_config.region)
        .property("access-key", &s3_config.access_key)
        .property("secret-access-key", &s3_config.secret_access_key)
        .property("endpoint-uri", &s3_config.endpoint_uri)
        .property("key", &format!("hls_output/{}/thumb.jpg", stream_id))
        .property("sync", &false)
        .property("async", &true)
        .build()?;

    Ok((
        queue,
        h264parse,
        avdec_h264,
        videoconvert,
        videoscale,
        videorate,
        capsfilter,
        jpegenc,
        awss3putobjectsink,
    ))
}