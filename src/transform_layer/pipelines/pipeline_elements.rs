use gstreamer_app::glib::BoolError;
use gstreamer_app::gst;

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
pub fn create_output(
    stream_id: u32,
    root_playlist: &str,
    output_path: &str,
    segment_delay: u32,
) -> Result<(gst::Element, gst::Element), BoolError> {
    let mpegtsmux = gst::ElementFactory::make("mpegtsmux")
        .property("name", &format!("mpegtsmux-{}", stream_id))
        .build()?;
    let hlssink = gst::ElementFactory::make("hlssink3")
        .property("playlist-root", root_playlist) 
        .property(
            "playlist-location",
            &format!("{}/playlist.m3u8", output_path),
        )
    
        .property("location", &format!("{}/segment_%05d.m4s", output_path)) 
        .property("target-duration", segment_delay) 
        .property("max-files", 5u32)                
        .build()?;

    Ok((mpegtsmux, hlssink))
}

pub fn create_thumbnail(
    stream_id: u32,
    output_path: &str,
) -> Result<
    (
        gst::Element,
        gst::Element,
        gst::Element,
        gst::Element,
        gst::Element,
        gst::Element,
    ),
    BoolError,
> {
    let avdec_h264 = gst::ElementFactory::make("avdec_h264")
        .name(&format!("avdec-thumb-{}", stream_id))
        .build()?;

    let videoconvert = gst::ElementFactory::make("videoconvert")
        .name(&format!("videoconvert-thumb-{}", stream_id))
        .build()?;

    let videorate = gst::ElementFactory::make("videorate")
        .name(&format!("videorate-thumb-{}", stream_id))
        .build()?;

    let caps = gst::Caps::builder("video/x-raw")
        .field("framerate", gst::Fraction::new(1, 60))
        .build();

    let capsfilter = gst::ElementFactory::make("capsfilter")
        .name(&format!("capsfilter-thumb-{}", stream_id))
        .property("caps", &caps)
        .build()?;

    let jpegenc = gst::ElementFactory::make("jpegenc")
        .name(&format!("jpegenc-thumb-{}", stream_id))
        .build()?;

    let multifilesink = gst::ElementFactory::make("multifilesink")
        .name(&format!("thumbsink-{}", stream_id))
        .property("location", &format!("{}/thumb.jpg", output_path))
        .property("post-messages", &true)
        .property("async", &true)
        .property("sync", &false)
        .build()?;

    Ok((
        avdec_h264,
        videoconvert,
        videorate,
        capsfilter,
        jpegenc,
        multifilesink,
    ))
}

pub fn create_thumbnail_queue(
    stream_id: u32,
) -> Result<(gst::Element, gst::Element, gst::Element), BoolError> {
    let tee = gst::ElementFactory::make("tee")
        .name(&format!("tee-{}", stream_id))
        .build()?;
    let queue_hls = gst::ElementFactory::make("queue")
        .name(&format!("queue-hls-{}", stream_id))
        .build()?;
    let queue_thumb = gst::ElementFactory::make("queue")
        .name(&format!("queue-thumb-{}", stream_id))
        .property("max-size-buffers", 1u32)
        .property("max-size-time", 0u64)
        .property_from_str("leaky", "downstream") 
        .build()?;

    Ok((tee, queue_hls, queue_thumb))
}
