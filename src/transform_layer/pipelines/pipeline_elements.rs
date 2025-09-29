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

pub fn create_video(stream_id: u32) -> Result<(gst::Element, gst::Element), BoolError>  {
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

pub fn create_output(stream_id: u32, root_playlist: &str, output_path: &str, segment_delay: u32) -> Result<( gst::Element), BoolError> {
    let awss3hlssink = gst::ElementFactory::make("awss3hlssink") 
        .property("name", &format!("awss3hlssink-{}", stream_id))
        .property("bucket", "") 
        .property("region", "ap-northeast-2") 
        .property("access-key", "") 
        .property("secret-access-key", "") 
        .property("endpoint-uri", "https://s3.ap-northeast-2.amazonaws.com") 
        .property("key-prefix", &format!("hls_output/{}", stream_id)) 
        .build()?;

    let hlssink: gst::Element = awss3hlssink.property("hlssink");
        hlssink.set_property("playlist-root", root_playlist.to_string());
        hlssink.set_property(
            "playlist-location",
            "playlist.m3u8",
        );
        hlssink.set_property(
            "location",
            "segment_%05d.ts",
        );
        hlssink.set_property("target-duration", segment_delay);
        hlssink.set_property("max-files", 0u32);
        hlssink.set_property("playlist-length", 0u32);

    Ok((awss3hlssink))
}
