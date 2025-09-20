use gst::Element;
use gstreamer::prelude::{ElementExt, ElementExtManual, GstObjectExt, PadExt};
use gstreamer_app::gst;

pub fn setup_dynamic_pads(
    flvdemux: &Element,
    video_elements: (Element, Element),
    audio_elements: (Element, Element),
    mpeg_ts_mux: &Element,
) {
    let (video_queue, h264_parse) = video_elements;
    let (audio_queue, aac_parse) = audio_elements;
    let mux_clone = mpeg_ts_mux.clone();

    flvdemux.connect_pad_added(move |_, pad| {
        let pad_name = pad.name();

        match pad_name.as_str() {
            name if name.starts_with("video") => {
                link_video_pipeline(pad, &video_queue, &h264_parse, &mux_clone);
            }
            name if name.starts_with("audio") => {
                link_audio_pipeline(pad, &audio_queue, &aac_parse, &mux_clone);
            }
            _ => eprintln!("unknown pad: {}", pad_name)
        }
    });
}

fn link_video_pipeline(pad: &gst::Pad, queue: &Element, parser: &Element, mux: &Element) {
    let sink_pad = queue.static_pad("sink").unwrap();
    if sink_pad.is_linked() { return; }

    if pad.link(&sink_pad).is_ok()
        && queue.link(parser).is_ok()
        && parser.link(mux).is_ok() {
        println!("Video pipeline connected");
    }
}

fn link_audio_pipeline(pad: &gst::Pad, queue: &Element, parser: &Element, mux: &Element) {
    let sink_pad = queue.static_pad("sink").unwrap();
    if sink_pad.is_linked() { return; }

    if pad.link(&sink_pad).is_ok()
        && queue.link(parser).is_ok()
        && parser.link(mux).is_ok() {
        println!("Audio pipeline connected");
    }
}