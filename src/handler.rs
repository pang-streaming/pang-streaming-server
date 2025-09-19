use scuffle_rtmp::session::server::{ServerSessionError, SessionData, SessionHandler};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use reqwest::Client;

pub struct Handler {
    pipelines: Arc<Mutex<HashMap<u32, Pipeline>>>,
    output_dir: String,
    segment_delay: u64,
    authenticated_stream_id: Option<String>,
    http_client: Arc<Client>,
}

struct Pipeline {
    pipeline: gst::Pipeline,
    appsrc: gst_app::AppSrc,
    stop_tx: tokio::sync::oneshot::Sender<()>,
}

impl Handler {
    pub fn new(client: Arc<Client>) -> Result<Self, Box<dyn std::error::Error>> {
        gst::init().map_err(|e| format!("Failed to initialize GStreamer: {}", e))?;

        let config = crate::config::get_config();
        let segment_delay = config.server.segment_delay;

        let output_dir = "./hls_output".to_string();
        std::fs::create_dir_all(&output_dir).unwrap_or_else(|e| {
            eprintln!("Failed to create output directory: {}", e);
        });

        Ok(Self {
            pipelines: Arc::new(Mutex::new(HashMap::new())),
            output_dir,
            segment_delay,
            authenticated_stream_id: None,
            http_client: client,
        })
    }

    fn start_hls_conversion(
        &self,
        stream_id: u32,
        stream_key: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let output_path = format!("{}/{}", self.output_dir, stream_key);
        std::fs::create_dir_all(&output_path)?;

        let segment_path = format!("{}/stream.ts", output_path);

        let pipeline = gst::Pipeline::new();

        let appsrc = gst::ElementFactory::make("appsrc")
            .property("name", &format!("appsrc-{}", stream_id))
            .build()?;

        let flvdemux = gst::ElementFactory::make("flvdemux")
            .property("name", &format!("flvdemux-{}", stream_id))
            .build()?;

        let videoqueue = gst::ElementFactory::make("queue")
            .property("name", &format!("videoqueue-{}", stream_id))
            .build()?;

        let h264parse = gst::ElementFactory::make("h264parse")
            .property("name", &format!("h264parse-{}", stream_id))
            .property("config-interval", -1i32)
            .build()?;

        let audioqueue = gst::ElementFactory::make("queue")
            .property("name", &format!("audioqueue-{}", stream_id))
            .build()?;

        let aacparse = gst::ElementFactory::make("aacparse")
            .property("name", &format!("aacparse-{}", stream_id))
            .build()?;

        let mpegtsmux = gst::ElementFactory::make("mpegtsmux")
            .property("name", &format!("mpegtsmux-{}", stream_id))
            .build()?;

        let filesink = gst::ElementFactory::make("filesink")
            .property("name", &format!("filesink-{}", stream_id))
            .build()?;

        let appsrc_element = appsrc.clone().downcast::<gst_app::AppSrc>().unwrap();
        appsrc_element.set_caps(Some(&gst::Caps::builder("video/x-flv").build()));
        appsrc_element.set_property("format", gst::Format::Time);
        appsrc_element.set_property("is-live", true);
        appsrc_element.set_property("do-timestamp", true);

        filesink.set_property("location", &segment_path);

        pipeline.add_many(&[
            &appsrc,
            &flvdemux,
            &videoqueue,
            &h264parse,
            &audioqueue,
            &aacparse,
            &mpegtsmux,
            &filesink,
        ])?;

        appsrc.link(&flvdemux)?;
        mpegtsmux.link(&filesink)?;

        let videoqueue_clone = videoqueue.clone();
        let audioqueue_clone = audioqueue.clone();
        let h264parse_clone = h264parse.clone();
        let aacparse_clone = aacparse.clone();
        let mpegtsmux_clone = mpegtsmux.clone();

        flvdemux.connect_pad_added(move |_, pad| {
            let pad_name = pad.name();
            println!("New pad added: {}", pad_name);

            if pad_name.starts_with("video") {
                let sink_pad = videoqueue_clone.static_pad("sink").unwrap();
                if sink_pad.is_linked() {
                    return;
                }

                if pad.link(&sink_pad).is_ok() {
                    println!("Video pad linked to queue");
                    if videoqueue_clone.link(&h264parse_clone).is_ok() {
                        println!("Video queue linked to h264parse");
                        if h264parse_clone.link(&mpegtsmux_clone).is_ok() {
                            println!("h264parse linked to mux - video pipeline complete");
                        }
                    }
                }
            } else if pad_name.starts_with("audio") {
                let sink_pad = audioqueue_clone.static_pad("sink").unwrap();
                if sink_pad.is_linked() {
                    return;
                }

                if pad.link(&sink_pad).is_ok() {
                    println!("Audio pad linked to queue");
                    if audioqueue_clone.link(&aacparse_clone).is_ok() {
                        println!("Audio queue linked to aacparse");
                        if aacparse_clone.link(&mpegtsmux_clone).is_ok() {
                            println!("aacparse linked to mux - audio pipeline complete");
                        }
                    }
                }
            }
        });

        pipeline.set_state(gst::State::Playing)?;

        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel();
        let output_path_clone = output_path.clone();
        eprintln!("{}", format!("플리 보여주기{}", output_path_clone));
        let playlist_path = format!("{}/playlist.m3u8", output_path);
        let segment_path_clone = segment_path.clone();
        let segment_delay = self.segment_delay;

        tokio::spawn(async move {
            use std::time::Duration;
            use tokio::time::sleep;

            let mut segment_counter = 0u32;

            loop {
                tokio::select! {
                    _ = &mut stop_rx => {
                        println!("Stopping segment loop for stream {}", stream_id);
                        break;
                    }
                    _ = sleep(Duration::from_secs(segment_delay)) => {
                        if std::path::Path::new(&segment_path_clone).exists() {
                            let segment_name = format!("segment{:05}.ts", segment_counter);
                            let new_segment_path = format!("{}/{}", output_path_clone, segment_name);

                            if let Ok(_) = std::fs::copy(&segment_path_clone, &new_segment_path) {
                                println!("Created segment: {}", segment_name);

                                let mut playlist = String::new();
                                playlist.push_str("#EXTM3U\n");
                                playlist.push_str("#EXT-X-VERSION:3\n");
                                playlist.push_str("#EXT-X-TARGETDURATION:3\n");
                                playlist.push_str(&format!("#EXT-X-MEDIA-SEQUENCE:{}\n", segment_counter));

                                let window_size = 5;

                                let start = if segment_counter >= window_size {
                                    segment_counter - window_size + 1
                                } else {
                                    0
                                };

                                for i in start..=segment_counter {
                                    let seg_name = format!("segment{:05}.ts", i);
                                    playlist.push_str("#EXTINF:3.0,\n");
                                    playlist.push_str(&format!("{}\n", seg_name));
                                }

                                if let Err(e) = std::fs::write(&playlist_path, playlist) {
                                    eprintln!("Failed to write playlist: {}", e);
                                } else {
                                    println!("Updated playlist with segment {}", segment_counter);
                                }

                                segment_counter += 1;
                            }
                        }
                    }
                }
            }
        });

        let pipeline_info: Pipeline = Pipeline {
            pipeline,
            appsrc: appsrc_element,
            stop_tx,
        };

        let mut pipelines = self.pipelines.lock().unwrap();
        pipelines.insert(stream_id, pipeline_info);

        println!(
            "GStreamer HLS conversion started for stream {} (key: {}) using basic filesink",
            stream_id, stream_key
        );
        println!("Stream will be saved to: {}", segment_path);
        Ok(())
    }

    fn create_flv_header() -> Vec<u8> {
        let mut header = Vec::new();
        header.extend_from_slice(b"FLV");
        header.push(1);
        header.push(0x05);
        header.extend_from_slice(&9u32.to_be_bytes());
        header.extend_from_slice(&0u32.to_be_bytes());
        header
    }

    fn create_flv_tag(tag_type: u8, timestamp: u32, data: &[u8]) -> Vec<u8> {
        let mut tag = Vec::new();
        tag.push(tag_type);
        let data_size = data.len() as u32;
        tag.push((data_size >> 16) as u8);
        tag.push((data_size >> 8) as u8);
        tag.push(data_size as u8);
        tag.push((timestamp >> 16) as u8);
        tag.push((timestamp >> 8) as u8);
        tag.push(timestamp as u8);
        tag.push((timestamp >> 24) as u8);
        tag.extend_from_slice(&[0, 0, 0]);
        tag.extend_from_slice(data);
        let tag_size = (11 + data.len()) as u32;
        tag.extend_from_slice(&tag_size.to_be_bytes());
        tag
    }

    fn push_to_gstreamer(
        &self,
        stream_id: u32,
        flv_data: Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut pipelines = self.pipelines.lock().unwrap();
        if let Some(pipeline_info) = pipelines.get_mut(&stream_id) {
            let mut buffer = gst::Buffer::with_size(flv_data.len()).unwrap();
            {
                let buffer_ref = buffer.get_mut().unwrap();
                let mut map = buffer_ref.map_writable().unwrap();
                map.copy_from_slice(&flv_data);
            }

            match pipeline_info.appsrc.push_buffer(buffer) {
                Ok(_) => {}
                Err(gst::FlowError::Flushing) => {
                    println!("Pipeline is flushing for stream {}", stream_id);
                }
                Err(e) => {
                    eprintln!("Failed to push buffer to AppSrc: {:?}", e);
                    return Err(format!("GStreamer push error: {:?}", e).into());
                }
            }
        } else {
            eprintln!("No pipeline found for stream {}", stream_id);
        }
        Ok(())
    }

    fn stop_hls_conversion(&self, stream_id: u32) {
        let mut pipelines = self.pipelines.lock().unwrap();
        if let Some(pipeline_info) = pipelines.remove(&stream_id) {
            let _ = pipeline_info.stop_tx.send(());
            let _ = pipeline_info.appsrc.end_of_stream();
            let _ = pipeline_info.pipeline.set_state(gst::State::Null);
            println!("GStreamer HLS conversion stopped for stream {}", stream_id);
        }
    }
}

impl SessionHandler for Handler {
    async fn on_data(
        &mut self,
        stream_id: u32,
        data: SessionData,
    ) -> Result<(), ServerSessionError> {
        match data {
            SessionData::Video { timestamp, data } => {
                let flv_tag = Self::create_flv_tag(9, timestamp, &data);
                let _ = self.push_to_gstreamer(stream_id, flv_tag);
            }
            SessionData::Audio { timestamp, data } => {
                let flv_tag = Self::create_flv_tag(8, timestamp, &data);
                let _ = self.push_to_gstreamer(stream_id, flv_tag);
            }
            SessionData::Amf0 { timestamp, data } => {
                let flv_tag = Self::create_flv_tag(18, timestamp, &data);
                let _ = self.push_to_gstreamer(stream_id, flv_tag);
            }
        }
        Ok(())
    }

    async fn on_publish(
        &mut self,
        stream_id: u32,
        app_name: &str,
        stream_key: &str,
    ) -> Result<(), ServerSessionError> {
        if stream_key.is_empty() {
            return Err(ServerSessionError::InvalidChunkSize(0));
        }

        if let Err(e) = self.start_hls_conversion(stream_id, stream_key) {
            eprintln!("Failed to start HLS conversion: {}", e);
            return Err(ServerSessionError::InvalidChunkSize(0));
        }

        let flv_header = Self::create_flv_header();
        let _ = self.push_to_gstreamer(stream_id, flv_header);
        Ok(())
    }

    async fn on_unpublish(&mut self, stream_id: u32) -> Result<(), ServerSessionError> {
        self.stop_hls_conversion(stream_id);
        Ok(())
    }
}
