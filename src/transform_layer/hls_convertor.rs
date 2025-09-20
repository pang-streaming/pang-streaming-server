use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, Mutex};
use gstreamer::glib::BoolError;
use gstreamer::prelude::{ElementExt, ElementExtManual, GstBinExtManual};
use gstreamer_app::{gst, AppSrc};
use gstreamer_app::prelude::Cast;
use reqwest::Client;
use crate::transform_layer::pads::dynamic_pads::setup_dynamic_pads;
use crate::transform_layer::pipelines::pipeline_elements::{create_audio, create_output, create_source, create_video};
use crate::utils::log_error::LogError;

pub struct HlsConvertor {
    pipelines: Arc<Mutex<HashMap<u32, Pipeline>>>,
    output_dir: String,
    segment_delay: u32,
    http_client: Arc<Client>,
}

pub struct Pipeline {
    pipeline: gst::Pipeline,
    pub(crate) app_src: AppSrc,
}

impl HlsConvertor {
    pub fn new(client: Arc<Client>) -> Result<Self, Box<dyn Error>> {
        let config = crate::config::get_config();
        let segment_delay = config.server.segment_delay;
        let output_dir = "./hls_output".to_string();
        std::fs::create_dir_all(&output_dir)
            .log_error("Failed to create output directory: ");

        Ok(Self {
            pipelines: Arc::new(Mutex::new(HashMap::new())),
            output_dir,
            segment_delay,
            http_client: client,
        })
    }

    pub fn get_pipelines(&self) -> Arc<Mutex<HashMap<u32, Pipeline>>> {
        self.pipelines.clone()
    }

    pub fn start_hls_conversion(
        &self,
        stream_id: u32,
        stream_name: &str
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let output_path = format!("{}/{}", self.output_dir, stream_name);

        //로컬 테스트용 - daedyu
        if !self.output_dir.starts_with("s3://") {
            std::fs::create_dir_all(&output_path)?;
        }

        let root_playlist = format!("http://localhost:8080/{}/", stream_name);
        let pipeline = self.create_hls_pipeline(
            stream_id,
            &root_playlist,
            &output_path,
            self.segment_delay,
        )?;
        let mut pipelines = self.pipelines.lock().unwrap();
        pipelines.insert(stream_id, pipeline);
        println!("HLS conversion started for stream {} (key: {})", stream_id, stream_name);
        println!("Playlist available at: {}/playlist.m3u8", output_path);
        Ok(())
    }

    fn create_hls_pipeline(
        &self,
        stream_id: u32,
        root_playlist: &str,
        output_path: &str,
        segment_delay: u32,
    ) -> Result<Pipeline, Box<dyn Error + Send + Sync>> {
        let pipeline = gst::Pipeline::new();

        let (app_src, flvdemux) = create_source(stream_id)?;
        let video_elements = create_video(stream_id)?;
        let audio_elements = create_audio(stream_id)?;
        let (mpeg_ts_mux, hls_sink) = create_output(
            stream_id,
            root_playlist,
            output_path,
            segment_delay
        )?;

        pipeline.add_many(&[
            &app_src, &flvdemux,
            &video_elements.0, &video_elements.1,
            &audio_elements.0, &audio_elements.1,
            &mpeg_ts_mux, &hls_sink,
        ])?;

        app_src.link(&flvdemux)?;
        mpeg_ts_mux.link(&hls_sink)?;

        setup_dynamic_pads(&flvdemux, video_elements, audio_elements, &mpeg_ts_mux);
        pipeline.set_state(gst::State::Playing)?;

        let app_src_element = app_src.downcast::<AppSrc>().unwrap();
        Ok(Pipeline { pipeline, app_src: app_src_element })
    }

    pub fn stop_hls_conversion(&self, stream_id: u32) {
        let mut pipelines = self.pipelines.lock().unwrap();
        if let Some(pipeline_info) = pipelines.remove(&stream_id) {
            let _ = pipeline_info.app_src.end_of_stream();
            let _ = pipeline_info.pipeline.set_state(gst::State::Null);
            println!("GStreamer HLS conversion stopped for stream {}", stream_id);
        }
    }

    pub fn create_flv_header(&self) -> Vec<u8> {
        let mut header = Vec::new();
        header.extend_from_slice(b"FLV");
        header.push(1);
        header.push(0x05);
        header.extend_from_slice(&9u32.to_be_bytes());
        header.extend_from_slice(&0u32.to_be_bytes());
        header
    }

    pub fn create_flv_tag(&self, tag_type: u8, timestamp: u32, data: &[u8]) -> Vec<u8> {
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
}