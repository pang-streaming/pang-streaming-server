use gst::Element;
use gstreamer::prelude::{ElementExt, ElementExtManual, GstObjectExt, PadExt};
use gstreamer_app::gst;

pub fn setup_dynamic_pads(
    flvdemux: &Element,
    video_elements: (Element, Element),
    audio_elements: (Element, Element),
    aws_hls_sink: &Element,
) {
    let (video_queue, h264_parse) = video_elements;
    let (audio_queue, aac_parse) = audio_elements;
    let sink_clone = aws_hls_sink.clone();

    flvdemux.connect_pad_added(move |_, pad| {
        let pad_name = pad.name();
        println!("flvdemux pad added: {}", pad_name);

        match pad_name.as_str() {
            name if name.starts_with("video") => {
                link_video_pipeline(pad, &video_queue, &h264_parse, &sink_clone);
            }
            name if name.starts_with("audio") => {
                link_audio_pipeline(pad, &audio_queue, &aac_parse, &sink_clone);
            }
            _ => eprintln!("unknown pad: {}", pad_name),
        }
    });
}

fn link_video_pipeline(pad: &gst::Pad, queue: &Element, parser: &Element, sink: &Element) {
    let sink_pad = queue.static_pad("sink").unwrap();
    if sink_pad.is_linked() {
        return;
    }

    if pad.link(&sink_pad).is_err() {
        eprintln!("Failed to link video pad to queue");
        return;
    }

    if queue.link(parser).is_err() {
        eprintln!("Failed to link video queue to parser");
        return;
    }

    let mux_video_pad = sink.request_pad_simple("video").unwrap();
    if parser.static_pad("src").unwrap().link(&mux_video_pad).is_err() {
        eprintln!("Failed to link video parser to sink");
        return;
    }

    println!("Video pipeline connected");
}

fn link_audio_pipeline(pad: &gst::Pad, queue: &Element, parser: &Element, sink: &Element) {
    let sink_pad = queue.static_pad("sink").unwrap();
    if sink_pad.is_linked() {
        return;
    }

    if pad.link(&sink_pad).is_err() {
        eprintln!("Failed to link audio pad to queue");
        return;
    }

    if queue.link(parser).is_err() {
        eprintln!("Failed to link audio queue to parser");
        return;
    }

    let mux_audio_pad = sink.request_pad_simple("audio").unwrap();
    if parser.static_pad("src").unwrap().link(&mux_audio_pad).is_err() {
        eprintln!("Failed to link audio parser to sink");
        return;
    }

    println!("Audio pipeline connected");
}