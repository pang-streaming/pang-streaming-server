use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, Mutex};
use gstreamer::glib::object::ObjectExt;
use gstreamer::prelude::{ElementExt, ElementExtManual, GstBinExt, GstBinExtManual, GstObjectExt, PadExt};
use gstreamer_app::{gst, AppSrc};
use gstreamer_app::prelude::Cast;
use crate::transform_layer::pads::dynamic_pads::setup_dynamic_pads;
use crate::transform_layer::pipelines::pipeline_elements::{create_audio, create_output, create_source, create_thumbnail_output, create_video};
use crate::utils::log_error::LogError;

pub struct HlsConvertor {
    pipelines: Arc<Mutex<HashMap<u32, Pipeline>>>,
    output_dir: String,
    segment_delay: u32,
}

pub struct Pipeline {
    pipeline: gst::Pipeline,
    app_src: AppSrc,
}

impl Pipeline {
    pub fn app_src(&self) -> &AppSrc {
        &self.app_src
    }
}

impl HlsConvertor {
    pub fn new(output_dir: String) -> Result<Self, Box<dyn Error>> {
        let config = crate::config::get_config();
        let segment_delay = config.server.segment_delay;
        std::fs::create_dir_all(&output_dir)
            .log_error("Failed to create output directory: ");

        Ok(Self {
            pipelines: Arc::new(Mutex::new(HashMap::new())),
            output_dir,
            segment_delay,
        })
    }

    pub fn get_pipelines(&self) -> Arc<Mutex<HashMap<u32, Pipeline>>> {
        self.pipelines.clone()
    }

    pub fn start_hls_conversion(
        &self,
        stream_id: u32,
        stream_name: &str,
        stream_host: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let output_path = format!("{}/{}", self.output_dir, stream_name);

        if !self.output_dir.starts_with("s3://") {
            std::fs::create_dir_all(&output_path)?;
        }

        let root_playlist = format!("{}/{}", stream_host, stream_id);
        let pipeline = self.create_hls_pipeline(stream_id, &root_playlist)?;
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
    ) -> Result<Pipeline, Box<dyn Error + Send + Sync>> {
        let pipeline = gst::Pipeline::new();
        let tee = gst::ElementFactory::make("tee")
            .name(&format!("tee-{}", stream_id))
            .build()?;

        let (app_src, flvdemux) = create_source(stream_id)?;
        let (video_queue, h264parse) = create_video(stream_id)?;
        let audio_elements = create_audio(stream_id)?;
        let aws_hls_sink = create_output(stream_id, root_playlist)?;
        let thumbnail_elements: (gstreamer::Element, gstreamer::Element, gstreamer::Element, gstreamer::Element, gstreamer::Element, gstreamer::Element, gstreamer::Element, gstreamer::Element, gstreamer::Element) = create_thumbnail_output(stream_id)?;

        pipeline.add_many(&[
            &app_src, &flvdemux, &video_queue, &h264parse, &audio_elements.0, &audio_elements.1, &aws_hls_sink, &tee,
            &thumbnail_elements.0, &thumbnail_elements.1, &thumbnail_elements.2, &thumbnail_elements.3, &thumbnail_elements.4,
            &thumbnail_elements.5, &thumbnail_elements.6, &thumbnail_elements.7, &thumbnail_elements.8,
        ])?;

        app_src.link(&flvdemux)?;

        setup_dynamic_pads(
            &flvdemux,
            &video_queue,
            &h264parse,
            audio_elements,
            &aws_hls_sink,
            &thumbnail_elements,
            &tee,
        );

        pipeline.set_state(gst::State::Playing)?;

        let app_src_element = app_src.downcast::<AppSrc>().unwrap();

        Ok(Pipeline {
            pipeline,
            app_src: app_src_element,
        })
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
