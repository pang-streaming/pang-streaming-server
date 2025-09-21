use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use gstreamer_app::gst;
use crate::transform_layer::hls_convertor::Pipeline;
use gstreamer;

pub fn push_to_gstreamer(
    pipelines: Arc<Mutex<HashMap<u32, Pipeline>>>,
    stream_id: u32,
    flv_data: Vec<u8>,
    timestamp: u32,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut pipelines = pipelines.lock().unwrap();
    if let Some(pipeline_info) = pipelines.get_mut(&stream_id) {
        let mut buffer = gst::Buffer::with_size(flv_data.len()).unwrap();
        {
            let buffer_ref = buffer.get_mut().unwrap();
            buffer_ref.set_pts(gstreamer::ClockTime::from_mseconds(timestamp as u64));
            buffer_ref.set_dts(gstreamer::ClockTime::from_mseconds(timestamp as u64));
            let mut map = buffer_ref.map_writable().unwrap();
            map.copy_from_slice(&flv_data);
        }

        match pipeline_info.app_src().push_buffer(buffer) {
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