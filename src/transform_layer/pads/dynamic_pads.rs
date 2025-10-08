use gstreamer::Element;
use gstreamer::prelude::{ElementExt, ElementExtManual, GstObjectExt, PadExt};

pub fn setup_dynamic_pads(
    flvdemux: &Element,
    video_queue: &Element,
    h264parse: &Element,
    audio_elements: (Element, Element),
    aws_hls_sink: &Element,
    thumbnail_elements: &(
        Element,
        Element,
        Element,
        Element,
        Element,
        Element,
        Element,
        Element,
        Element,
    ),
    tee: &Element,
) {
    let (audio_queue, aac_parse) = audio_elements;
    let sink_clone = aws_hls_sink.clone();
    let tee_clone = tee.clone();
    let video_queue = video_queue.clone();
    let h264parse = h264parse.clone();
    let thumbnail_elements = thumbnail_elements.clone();

    flvdemux.connect_pad_added(move |_, pad| {
        let pad_name = pad.name();
        println!("flvdemux pad added: {}", pad_name);

        match pad_name.as_str() {
            name if name.starts_with("video") => {
                let tee_sink_pad = tee_clone.static_pad("sink").unwrap();
                pad.link(&tee_sink_pad).unwrap();

                link_video_pipeline(&tee_clone, &video_queue, &h264parse, &sink_clone);
                link_thumbnail_pipeline(&tee_clone, &thumbnail_elements);
            }
            name if name.starts_with("audio") => {
                link_audio_pipeline(pad, &audio_queue, &aac_parse, &sink_clone);
            }
            _ => eprintln!("unknown pad: {}", pad_name),
        }
    });
}

fn link_video_pipeline(tee: &Element, queue: &Element, parser: &Element, sink: &Element) {
    let tee_src_pad = tee.request_pad_simple("src_%u").unwrap();
    let queue_sink_pad = queue.static_pad("sink").unwrap();
    if tee_src_pad.link(&queue_sink_pad).is_err() {
        eprintln!("Failed to link video tee to HLS queue");
        return;
    }

    if queue.link(parser).is_err() {
        eprintln!("Failed to link HLS queue to parser");
        return;
    }

    let mux_video_pad = sink.request_pad_simple("video").unwrap();
    if parser.static_pad("src").unwrap().link(&mux_video_pad).is_err() {
        eprintln!("Failed to link video parser to sink");
        return;
    }

    println!("HLS video pipeline connected");
}

fn link_thumbnail_pipeline(
    tee: &Element,
    thumb_elements: &(
        Element,
        Element,
        Element,
        Element,
        Element,
        Element,
        Element,
        Element,
        Element,
    ),
) {
    let (queue, h264parse, avdec_h264, videoconvert, videoscale, videorate, capsfilter, jpegenc, awss3putobjectsink) = thumb_elements;

    let tee_src_pad = tee.request_pad_simple("src_%u").unwrap();
    let queue_sink_pad = queue.static_pad("sink").unwrap();
    if tee_src_pad.link(&queue_sink_pad).is_err() {
        eprintln!("Failed to link tee to thumbnail queue");
        return;
    }

    let elements = vec![
        queue, h264parse, avdec_h264, videoconvert, videoscale, videorate, capsfilter, jpegenc, awss3putobjectsink
    ];

    if let Err(e) = gstreamer::Element::link_many(&elements) {
        eprintln!("Failed to link thumbnail pipeline elements: {}", e);
    }

    println!("Thumbnail pipeline connected");
}

fn link_audio_pipeline(pad: &gstreamer::Pad, queue: &Element, parser: &Element, sink: &Element) {
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
